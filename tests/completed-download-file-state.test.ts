import { describe, expect, test } from 'bun:test';
import {
  isDownloadProgressForItem,
  reconcileQueueItemsWithHistoryStates,
  resetMissingCompletedQueueItems,
} from '@/lib/persisted-download-queue';
import type { DownloadItem } from '@/lib/types';

describe('resetMissingCompletedQueueItems', () => {
  test('returns a completed item to pending when its downloaded file was removed', () => {
    const item = {
      id: 'download-1',
      url: 'https://example.com/video',
      title: 'Video',
      thumbnail: '',
      duration: '',
      extractor: 'direct',
      status: 'completed',
      progress: 100,
      completedFilepath: 'C:\\Downloads\\video.mp4',
      completedHistoryId: 'history-1',
      completedFilesize: 1024,
      completedFormat: 'mp4',
    } satisfies DownloadItem;

    const result = resetMissingCompletedQueueItems([item], new Set(['C:\\Downloads\\video.mp4']));

    expect(result[0]).toMatchObject({
      status: 'pending',
      progress: 0,
      errorCode: 'OUTPUT_FILE_MISSING',
    });
    expect(result[0]?.completedFilepath).toBeUndefined();
    expect(result[0]?.completedHistoryId).toBe('history-1');
    expect(result[0]?.outputCollisionPolicy).toBe('overwrite');
  });

  test('keeps completed items unchanged when their files still exist', () => {
    const item = {
      id: 'download-1',
      url: 'https://example.com/video',
      title: 'Video',
      thumbnail: '',
      duration: '',
      extractor: 'direct',
      status: 'completed',
      progress: 100,
      completedFilepath: 'C:\\Downloads\\video.mp4',
    } satisfies DownloadItem;
    const items = [item];

    expect(resetMissingCompletedQueueItems(items, new Set())).toBe(items);
  });

  test('restores the exact queue item after its history row is relinked', () => {
    const item = {
      id: 'download-1',
      url: 'https://example.com/video',
      title: 'Video',
      status: 'pending',
      progress: 0,
      speed: '',
      eta: '',
      errorCode: 'OUTPUT_FILE_MISSING',
      completedHistoryId: 'history-1',
    } satisfies DownloadItem;

    const result = reconcileQueueItemsWithHistoryStates(
      [item],
      [
        {
          historyId: 'history-1',
          filepath: 'D:\\Moved\\video.mp4',
          fileExists: true,
        },
      ],
    );

    expect(result[0]).toMatchObject({
      status: 'completed',
      progress: 100,
      completedFilepath: 'D:\\Moved\\video.mp4',
      completedHistoryId: 'history-1',
    });
    expect(result[0]?.errorCode).toBeUndefined();
  });

  test('matches progress by job id or preserved history id', () => {
    const item = { id: 'queue-1', completedHistoryId: 'history-1' };

    expect(isDownloadProgressForItem(item, { id: 'queue-1' })).toBe(true);
    expect(
      isDownloadProgressForItem(item, { id: 'history-redownload-job', history_id: 'history-1' }),
    ).toBe(true);
    expect(isDownloadProgressForItem(item, { id: 'other', history_id: 'history-2' })).toBe(false);
  });
});
