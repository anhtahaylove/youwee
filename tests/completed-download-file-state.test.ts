import { describe, expect, test } from 'bun:test';
import { resetMissingCompletedQueueItems } from '@/lib/persisted-download-queue';
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
    expect(result[0]?.completedHistoryId).toBeUndefined();
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
});
