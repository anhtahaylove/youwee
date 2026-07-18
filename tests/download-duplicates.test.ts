import { describe, expect, test } from 'bun:test';
import {
  buildDownloadDuplicateIdentity,
  markInactiveQueueDuplicatesUnique,
  partitionDownloadQueueUrls,
  resolveDownloadedDuplicateCandidates,
} from '@/lib/download-duplicates';
import type { DownloadDuplicateCandidate, DownloadDuplicateMatch } from '@/lib/types';

describe('partitionDownloadQueueUrls', () => {
  test('lets a completed Facebook Reel reach history duplicate review for Add again', () => {
    const reelUrl = 'https://www.facebook.com/reel/2058460874874165';
    const completed = { id: 'completed-reel', url: reelUrl, status: 'completed' };

    const result = partitionDownloadQueueUrls([reelUrl], [completed]);

    expect(result.alreadyQueuedItems).toEqual([]);
    expect(result.newUrls).toEqual([reelUrl]);
  });

  test('keeps a pending Facebook Reel as the active queue item', () => {
    const reelUrl = 'https://www.facebook.com/reel/2058460874874165';
    const pending = { id: 'pending-reel', url: reelUrl, status: 'pending' };

    const result = partitionDownloadQueueUrls([reelUrl], [pending]);

    expect(result.alreadyQueuedItems).toEqual([pending]);
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

describe('resolveDownloadedDuplicateCandidates', () => {
  const candidate = {
    url: 'https://www.facebook.com/reel/123',
    title: 'Facebook Reel',
    duplicateIdentity: {
      canonicalUrl: 'https://www.facebook.com/reel/123',
    },
  } satisfies DownloadDuplicateCandidate;

  function duplicate(fileExists: boolean): DownloadDuplicateMatch {
    return {
      canonicalUrl: 'https://www.facebook.com/reel/123',
      historyId: 'history-123',
      title: 'Previous download',
      filepath: 'C:\\Downloads\\reel.mp4',
      downloadedAt: '2026-07-18T00:00:00Z',
      fileExists,
    };
  }

  test('restores the newest missing history row instead of creating a duplicate', () => {
    const result = resolveDownloadedDuplicateCandidates([candidate], [duplicate(false)]);

    expect(result.existing).toEqual([]);
    expect(result.available[0]).toMatchObject({
      historyId: 'history-123',
      outputCollisionPolicy: 'overwrite',
    });
  });

  test('keeps an existing file behind the duplicate review boundary', () => {
    const match = duplicate(true);
    const result = resolveDownloadedDuplicateCandidates([candidate], [match]);

    expect(result.available).toEqual([]);
    expect(result.existing).toEqual([{ candidate, duplicate: match }]);
  });
});

describe('markInactiveQueueDuplicatesUnique', () => {
  test('keeps Add again collision-safe when downloaded-video history checks are disabled', () => {
    const reelUrl = 'https://www.facebook.com/reel/123';
    const [candidate] = markInactiveQueueDuplicatesUnique(
      [
        {
          url: reelUrl,
          title: 'Facebook Reel',
          duplicateIdentity: buildDownloadDuplicateIdentity(reelUrl),
        },
      ],
      [{ id: 'completed-reel', url: reelUrl, status: 'completed' }],
    );

    expect(candidate?.outputCollisionPolicy).toBe('unique');
  });
});
