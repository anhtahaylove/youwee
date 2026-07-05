import { describe, expect, test } from 'bun:test';
import { extractUrls, parseUniversalUrls } from '../src/lib/sources';

describe('extractUrls', () => {
  test('extracts URLs embedded in pasted text', () => {
    expect(extractUrls('Watch this video: https://www.youtube.com/watch?v=abc123.')).toEqual([
      'https://www.youtube.com/watch?v=abc123',
    ]);
  });

  test('preserves order, deduplicates, and skips comment lines', () => {
    expect(
      extractUrls(
        [
          '# https://ignored.example/video',
          'first https://www.instagram.com/reel/abc/',
          'repeat https://www.instagram.com/reel/abc/',
          'second https://www.tiktok.com/@demo/video/123)',
        ].join('\n'),
      ),
    ).toEqual(['https://www.instagram.com/reel/abc/', 'https://www.tiktok.com/@demo/video/123']);
  });

  test('normalizes shell-escaped URLs', () => {
    expect(extractUrls('https://www.youtube.com/watch\\?v=abc123\\&t=30s')).toEqual([
      'https://www.youtube.com/watch?v=abc123&t=30s',
    ]);
  });

  test('keeps universal parser aligned with extraction behavior', () => {
    expect(parseUniversalUrls('link: https://www.facebook.com/reel/123,')).toEqual([
      'https://www.facebook.com/reel/123',
    ]);
  });
});
