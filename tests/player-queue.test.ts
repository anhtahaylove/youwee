import { describe, expect, test } from 'bun:test';
import {
  buildPlayableAudioQueue,
  isPlayableAudioEntry,
  type MinimalHistoryEntry,
  reconcilePlayableAudioQueue,
} from '../src/lib/player-queue';

interface TestEntry extends MinimalHistoryEntry {
  title?: string;
}

function makeEntry(overrides: Partial<TestEntry> = {}): TestEntry {
  return {
    id: '1',
    filepath: 'C:\\Music\\track.mp3',
    file_exists: true,
    format: 'mp3',
    quality: 'audio',
    ...overrides,
  };
}

describe('isPlayableAudioEntry', () => {
  test('accepts explicit audio downloads with existing files', () => {
    expect(isPlayableAudioEntry(makeEntry())).toBe(true);
  });

  test('rejects missing files or empty paths', () => {
    expect(isPlayableAudioEntry(makeEntry({ file_exists: false }))).toBe(false);
    expect(isPlayableAudioEntry(makeEntry({ filepath: '' }))).toBe(false);
  });

  test('rejects video entries even when the container can hold audio', () => {
    expect(isPlayableAudioEntry(makeEntry({ format: 'mp4', quality: '1080' }))).toBe(false);
    expect(isPlayableAudioEntry(makeEntry({ format: 'webm', quality: '720' }))).toBe(false);
  });

  test('accepts ambiguous containers only when they are explicit audio downloads', () => {
    expect(isPlayableAudioEntry(makeEntry({ format: 'webm', quality: 'audio' }))).toBe(true);
  });
});

describe('buildPlayableAudioQueue', () => {
  test('keeps source order while filtering out non-playable entries', () => {
    const queue = buildPlayableAudioQueue([
      makeEntry({ id: 'a', format: 'mp3', quality: 'audio' }),
      makeEntry({ id: 'b', format: 'mp4', quality: '1080' }),
      makeEntry({ id: 'c', format: 'm4a', quality: 'audio' }),
    ]);

    expect(queue.map((entry) => entry.id)).toEqual(['a', 'c']);
  });
});

describe('reconcilePlayableAudioQueue', () => {
  test('keeps current track when it still exists', () => {
    const queue = [
      makeEntry({ id: 'a' }),
      makeEntry({ id: 'b', filepath: 'C:\\Music\\b.mp3' }),
      makeEntry({ id: 'c', filepath: 'C:\\Music\\c.mp3' }),
    ];

    const result = reconcilePlayableAudioQueue(queue, 1, queue);

    expect(result.queue.map((entry) => entry.id)).toEqual(['a', 'b', 'c']);
    expect(result.currentIndex).toBe(1);
    expect(result.removedCurrent).toBe(false);
  });

  test('refreshes queued metadata when the same track ids are returned with new fields', () => {
    const queue = [makeEntry({ id: 'a', title: 'Old title', filepath: 'C:\\Music\\old.mp3' })];
    const refreshed = [makeEntry({ id: 'a', title: 'New title', filepath: 'C:\\Music\\new.mp3' })];

    const result = reconcilePlayableAudioQueue(queue, 0, refreshed);

    expect(result.queue[0]?.title).toBe('New title');
    expect(result.queue[0]?.filepath).toBe('C:\\Music\\new.mp3');
    expect(result.currentIndex).toBe(0);
    expect(result.removedCurrent).toBe(false);
  });

  test('moves to the next available track when the current one is removed', () => {
    const queue = [
      makeEntry({ id: 'a' }),
      makeEntry({ id: 'b', filepath: 'C:\\Music\\b.mp3' }),
      makeEntry({ id: 'c', filepath: 'C:\\Music\\c.mp3' }),
    ];
    const remaining = [queue[0], queue[2]];

    const result = reconcilePlayableAudioQueue(queue, 1, remaining);

    expect(result.queue.map((entry) => entry.id)).toEqual(['a', 'c']);
    expect(result.currentIndex).toBe(1);
    expect(result.removedCurrent).toBe(true);
  });

  test('falls back to the previous track when the removed current track was the last one', () => {
    const queue = [makeEntry({ id: 'a' }), makeEntry({ id: 'b', filepath: 'C:\\Music\\b.mp3' })];
    const remaining = [queue[0]];

    const result = reconcilePlayableAudioQueue(queue, 1, remaining);

    expect(result.queue.map((entry) => entry.id)).toEqual(['a']);
    expect(result.currentIndex).toBe(0);
    expect(result.removedCurrent).toBe(true);
  });

  test('returns an empty queue when nothing playable remains', () => {
    const queue = [makeEntry({ id: 'a' })];

    const result = reconcilePlayableAudioQueue(queue, 0, []);

    expect(result.queue).toEqual([]);
    expect(result.currentIndex).toBe(0);
    expect(result.removedCurrent).toBe(true);
  });
});
