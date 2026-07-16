import { describe, expect, test } from 'bun:test';
import { partitionDownloadQueueUrls } from '@/lib/download-duplicates';

describe('partitionDownloadQueueUrls', () => {
  test('recognizes a completed Facebook Reel already present in the queue', () => {
    const reelUrl = 'https://www.facebook.com/reel/2058460874874165';
    const completed = { id: 'completed-reel', url: reelUrl, status: 'completed' };

    const result = partitionDownloadQueueUrls([reelUrl], [completed]);

    expect(result.alreadyQueuedItems).toEqual([completed]);
    expect(result.newUrls).toEqual([]);
  });

  test('recognizes tracked and clean Facebook Reel URLs as the same queue item', () => {
    const completed = {
      id: 'completed-reel',
      url: 'https://www.facebook.com/reel/2058460874874165',
    };
    const trackedUrl =
      'https://m.facebook.com/reel/2058460874874165/?__cft__[0]=tracking&__tn__=%2CO%2CP-R';

    const result = partitionDownloadQueueUrls([trackedUrl], [completed]);

    expect(result.alreadyQueuedItems).toEqual([completed]);
    expect(result.newUrls).toEqual([]);
  });

  test('ignores URL fragments and keeps genuinely new links', () => {
    const queued = {
      id: 'queued-video',
      url: 'https://example.com/watch?id=1',
    };

    const result = partitionDownloadQueueUrls(
      ['https://example.com/watch?id=1#player', 'https://example.com/watch?id=2'],
      [queued],
    );

    expect(result.alreadyQueuedItems).toEqual([queued]);
    expect(result.newUrls).toEqual(['https://example.com/watch?id=2']);
  });
});
