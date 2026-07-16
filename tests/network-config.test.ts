import { describe, expect, test } from 'bun:test';
import {
  buildCookieProxyInvokeOptions,
  mergeCookieSkipPatterns,
  normalizeCookieSkipPattern,
  PUBLIC_COOKIE_SKIP_PRESETS,
  sanitizeCookieSkipPatterns,
} from '../src/lib/network-config';

describe('cookie skip patterns', () => {
  test('normalizes domains and domain path prefixes', () => {
    expect(normalizeCookieSkipPattern(' https://Facebook.com/reel/?x=1 ')).toBe(
      'facebook.com/reel',
    );
    expect(normalizeCookieSkipPattern('/facebook.com/reel/')).toBe('facebook.com/reel');
  });

  test('defaults missing saved patterns but preserves an explicit empty list', () => {
    expect(sanitizeCookieSkipPatterns(undefined)).toEqual(['facebook.com/reel']);
    expect(sanitizeCookieSkipPatterns([])).toEqual([]);
  });

  test('passes sanitized skip patterns to backend invoke options', () => {
    const options = buildCookieProxyInvokeOptions(
      {
        mode: 'browser',
        browser: 'chrome',
        cookieSkipPatterns: ['https://facebook.com/reel/', 'bad value'],
      },
      { mode: 'off' },
    );

    expect(options.cookieSkipPatterns).toEqual(['facebook.com/reel']);
  });

  test('adds public presets without replacing defaults or creating duplicates', () => {
    const withYouTube = mergeCookieSkipPatterns(undefined, PUBLIC_COOKIE_SKIP_PRESETS.youtube);

    expect(withYouTube).toEqual(['facebook.com/reel', 'youtube.com/watch', 'youtu.be']);
    expect(mergeCookieSkipPatterns(withYouTube, PUBLIC_COOKIE_SKIP_PRESETS.youtube)).toEqual(
      withYouTube,
    );
    expect(mergeCookieSkipPatterns(withYouTube, PUBLIC_COOKIE_SKIP_PRESETS.instagram)).toEqual([
      ...withYouTube,
      'instagram.com/reel',
      'instagram.com/p',
    ]);
  });
});
