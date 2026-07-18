import { describe, expect, test } from 'bun:test';
import { buildHistoryRedownloadIdentity } from '@/contexts/HistoryContext';
import type { HistoryEntry } from '@/lib/types';

describe('Library history re-download identity', () => {
  test('preserves the exact history row and recovered media metadata', () => {
    const entry: HistoryEntry = {
      id: 'history-facebook-reel',
      url: 'https://www.facebook.com/reel/792410310628126',
      title: 'Recovered Facebook Reel title',
      thumbnail: 'https://example.com/thumbnail.jpg',
      filepath: 'C:\\Downloads\\missing.mp4',
      source: 'facebook',
      downloaded_at: '2026-07-18T00:00:00Z',
      file_exists: false,
      tags: [],
      collections: [],
    };

    expect(buildHistoryRedownloadIdentity(entry)).toEqual({
      historyId: 'history-facebook-reel',
      title: 'Recovered Facebook Reel title',
      thumbnail: 'https://example.com/thumbnail.jpg',
      source: 'facebook',
    });
  });
});
