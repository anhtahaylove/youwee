import { describe, expect, test } from 'bun:test';
import { mergeRestoredItems } from '@/hooks/usePersistedDownloadQueue';
import type { DownloadItem } from '@/lib/types';

function queueItem(id: string, url: string, mediaId?: string): DownloadItem {
  return {
    id,
    url,
    title: id,
    status: 'pending',
    progress: 0,
    speed: '',
    eta: '',
    mediaId,
  };
}

describe('persisted download queue merge', () => {
  test('keeps current items authoritative across canonical URL and ID collisions', () => {
    const currentItems = [
      queueItem('current-youtube', 'https://www.youtube.com/watch?v=abc123'),
      queueItem('shared-id', 'https://example.com/current'),
    ];
    const savedItems = [
      queueItem('saved-youtube', 'https://youtu.be/abc123?t=30'),
      queueItem('shared-id', 'https://example.com/stale'),
      queueItem('saved-unique', 'https://example.com/unique'),
    ];

    expect(mergeRestoredItems(savedItems, currentItems).map((item) => item.id)).toEqual([
      'saved-unique',
      'current-youtube',
      'shared-id',
    ]);
  });

  test('does not restore duplicate IDs or canonical identities from persisted data', () => {
    const savedItems = [
      queueItem('first', 'https://example.com/first'),
      queueItem('first', 'https://example.com/duplicate-id'),
      queueItem('youtube-long', 'https://www.youtube.com/watch?v=xyz789'),
      queueItem('youtube-short', 'https://youtu.be/xyz789'),
    ];

    expect(mergeRestoredItems(savedItems, []).map((item) => item.id)).toEqual([
      'first',
      'youtube-long',
    ]);
  });
});
