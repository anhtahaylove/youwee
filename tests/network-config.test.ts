import { describe, expect, test } from 'bun:test';
import {
  buildCookieProxyInvokeOptions,
  COOKIE_SKIP_CATALOG_CACHE_KEY,
  COOKIE_SKIP_CATALOG_TTL_MS,
  loadCookieSkipRecommendations,
  normalizeCookieSettings,
  normalizeCookieSkipPattern,
  parseCookieSkipCatalog,
  refreshCookieSkipRecommendations,
  resolveCookieSkipPatterns,
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
    expect(sanitizeCookieSkipPatterns(undefined)).toEqual([]);
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

  test('migrates the legacy default into recommended rules without losing personal rules', () => {
    expect(
      normalizeCookieSettings({
        mode: 'browser',
        cookieSkipPatterns: ['facebook.com/reel', 'example.com/public'],
      }),
    ).toMatchObject({
      useRecommendedCookieSkipPatterns: true,
      cookieSkipPatterns: ['example.com/public'],
    });

    expect(normalizeCookieSettings({ mode: 'off', cookieSkipPatterns: [] })).toMatchObject({
      useRecommendedCookieSkipPatterns: false,
      cookieSkipPatterns: [],
    });
  });

  test('merges recommended and personal rules, or keeps personal rules only when disabled', () => {
    const settings = {
      mode: 'browser' as const,
      useRecommendedCookieSkipPatterns: true,
      cookieSkipPatterns: ['example.com/public', 'facebook.com/reel'],
    };

    expect(resolveCookieSkipPatterns(settings, ['facebook.com/reel'])).toEqual([
      'facebook.com/reel',
      'example.com/public',
    ]);
    expect(
      resolveCookieSkipPatterns({ ...settings, useRecommendedCookieSkipPatterns: false }, [
        'facebook.com/reel',
      ]),
    ).toEqual(['example.com/public', 'facebook.com/reel']);
  });

  test('validates the global catalog schema before accepting remote rules', () => {
    const catalog = parseCookieSkipCatalog(
      JSON.stringify({
        schemaVersion: 1,
        revision: '2026-07-18.1',
        updatedAt: '2026-07-18T00:00:00Z',
        recommendedPatterns: ['https://Facebook.com/reel/', 'example.com/public'],
      }),
      123,
    );

    expect(catalog).toMatchObject({
      patterns: ['facebook.com/reel', 'example.com/public'],
      revision: '2026-07-18.1',
      fetchedAt: 123,
      source: 'remote',
      stale: false,
    });

    expect(() =>
      parseCookieSkipCatalog(
        JSON.stringify({
          schemaVersion: 2,
          revision: 'bad',
          updatedAt: '2026-07-18T00:00:00Z',
          recommendedPatterns: ['facebook.com/reel'],
        }),
      ),
    ).toThrow('Unsupported cookie skip catalog schema');
  });

  test('accepts the checked-in global catalog used by production clients', async () => {
    const raw = await Bun.file('config/cookie-skip-rules.json').text();
    expect(parseCookieSkipCatalog(raw).patterns).toEqual([]);
    expect(await Bun.file('config/cookie-skip-rules.schema.json').json()).toMatchObject({
      title: 'Youwee recommended cookie skip rules',
      properties: { schemaVersion: { const: 1 } },
    });
  });

  test('caches the last known catalog and marks it stale after 24 hours', async () => {
    const values = new Map<string, string>();
    const memoryStorage: Storage = {
      get length() {
        return values.size;
      },
      clear: () => values.clear(),
      getItem: (key) => values.get(key) ?? null,
      key: (index) => [...values.keys()][index] ?? null,
      removeItem: (key) => values.delete(key),
      setItem: (key, value) => values.set(key, value),
    };
    const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: memoryStorage,
    });

    try {
      const fetchedAt = 1_000;
      const state = await refreshCookieSkipRecommendations({
        force: true,
        now: fetchedAt,
        fetcher: async () =>
          new Response(
            JSON.stringify({
              schemaVersion: 1,
              revision: '2026-07-18.1',
              updatedAt: '2026-07-18T00:00:00Z',
              recommendedPatterns: ['facebook.com/reel'],
            }),
          ),
      });

      expect(state.source).toBe('remote');
      expect(values.has(COOKIE_SKIP_CATALOG_CACHE_KEY)).toBe(true);
      expect(loadCookieSkipRecommendations(fetchedAt + 1)).toMatchObject({
        source: 'cache',
        stale: false,
      });
      expect(
        loadCookieSkipRecommendations(fetchedAt + COOKIE_SKIP_CATALOG_TTL_MS + 1),
      ).toMatchObject({ source: 'cache', stale: true });
    } finally {
      if (originalStorage) {
        Object.defineProperty(globalThis, 'localStorage', originalStorage);
      } else {
        Reflect.deleteProperty(globalThis, 'localStorage');
      }
    }
  });
});
