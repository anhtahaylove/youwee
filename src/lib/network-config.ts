import type { CookieSettings, ProxySettings } from '@/lib/types';

export const COOKIE_STORAGE_KEY = 'youwee-cookie-settings';
export const PROXY_STORAGE_KEY = 'youwee-proxy-settings';
export const COOKIE_SKIP_CATALOG_CACHE_KEY = 'youwee-cookie-skip-catalog-v2';
export const COOKIE_SKIP_CATALOG_URL =
  'https://raw.githubusercontent.com/anhtahaylove/youwee/main/config/cookie-skip-rules.json';
export const COOKIE_SKIP_CATALOG_TTL_MS = 24 * 60 * 60 * 1000;
export const DEFAULT_COOKIE_SKIP_PATTERNS: string[] = [];

const COOKIE_SKIP_CATALOG_TIMEOUT_MS = 10_000;
const MAX_COOKIE_SKIP_CATALOG_BYTES = 32 * 1024;
const MAX_COOKIE_SKIP_PATTERNS = 50;
const MAX_COOKIE_SKIP_PATTERN_LENGTH = 128;
const DEFAULT_COOKIE_SKIP_CATALOG_REVISION = 'bundled-2026-07-18.2';
const LEGACY_RECOMMENDED_COOKIE_SKIP_PATTERNS = ['facebook.com/reel'];
let activeCookieSkipCatalogRefresh: Promise<CookieSkipCatalogState> | null = null;

export type CookieSkipCatalogSource = 'remote' | 'cache' | 'fallback';

export interface CookieSkipCatalogDocument {
  schemaVersion: 1;
  revision: string;
  updatedAt: string;
  recommendedPatterns: string[];
}

export interface CookieSkipCatalogState {
  patterns: string[];
  revision: string;
  updatedAt: string;
  fetchedAt: number | null;
  source: CookieSkipCatalogSource;
  stale: boolean;
  error?: string;
}

interface CachedCookieSkipCatalog extends CookieSkipCatalogDocument {
  fetchedAt: number;
}

export type CookieProxyInvokeOptions = {
  cookieMode: CookieSettings['mode'];
  cookieBrowser: CookieSettings['browser'] | null;
  cookieBrowserProfile: string | null;
  cookieFilePath: string | null;
  cookieSkipPatterns: string[];
  proxyUrl: string | null;
};

export function normalizeCookieSkipPattern(pattern: string): string {
  const withoutScheme = pattern
    .trim()
    .replace(/^[a-z][a-z0-9+.-]*:\/\//i, '')
    .replace(/^\/+/, '')
    .split(/[?#]/)[0]
    .replace(/\/+$/, '');

  const slashIndex = withoutScheme.indexOf('/');
  if (slashIndex === -1) {
    return withoutScheme.toLowerCase();
  }

  const host = withoutScheme.slice(0, slashIndex).toLowerCase();
  const path = withoutScheme.slice(slashIndex).replace(/\/+$/, '');
  return `${host}${path}`;
}

export function isValidCookieSkipPattern(pattern: string): boolean {
  const normalized = normalizeCookieSkipPattern(pattern);
  if (!normalized || normalized.length > MAX_COOKIE_SKIP_PATTERN_LENGTH || /\s/.test(normalized)) {
    return false;
  }

  const host = normalized.split('/')[0];
  return host.includes('.') && !host.startsWith('.') && !host.endsWith('.');
}

export function sanitizeCookieSkipPatterns(
  patterns: unknown,
  fallback: readonly string[] = DEFAULT_COOKIE_SKIP_PATTERNS,
): string[] {
  if (!Array.isArray(patterns)) {
    return [...fallback];
  }

  const next: string[] = [];
  const seen = new Set<string>();

  for (const pattern of patterns) {
    if (next.length >= MAX_COOKIE_SKIP_PATTERNS || typeof pattern !== 'string') {
      continue;
    }

    const normalized = normalizeCookieSkipPattern(pattern);
    const key = normalized.toLowerCase();
    if (!isValidCookieSkipPattern(normalized) || seen.has(key)) {
      continue;
    }

    next.push(normalized);
    seen.add(key);
  }

  return next;
}

function validateCookieSkipCatalogDocument(value: unknown): CookieSkipCatalogDocument {
  if (!value || typeof value !== 'object') {
    throw new Error('Cookie skip catalog must be an object');
  }

  const candidate = value as Partial<CookieSkipCatalogDocument>;
  if (candidate.schemaVersion !== 1) {
    throw new Error('Unsupported cookie skip catalog schema');
  }
  if (
    typeof candidate.revision !== 'string' ||
    !candidate.revision.trim() ||
    candidate.revision.length > 64
  ) {
    throw new Error('Invalid cookie skip catalog revision');
  }
  if (typeof candidate.updatedAt !== 'string' || Number.isNaN(Date.parse(candidate.updatedAt))) {
    throw new Error('Invalid cookie skip catalog timestamp');
  }
  if (
    !Array.isArray(candidate.recommendedPatterns) ||
    candidate.recommendedPatterns.length > MAX_COOKIE_SKIP_PATTERNS ||
    candidate.recommendedPatterns.some(
      (pattern) => typeof pattern !== 'string' || !isValidCookieSkipPattern(pattern),
    )
  ) {
    throw new Error('Invalid cookie skip catalog rules');
  }

  return {
    schemaVersion: 1,
    revision: candidate.revision.trim(),
    updatedAt: candidate.updatedAt,
    recommendedPatterns: sanitizeCookieSkipPatterns(candidate.recommendedPatterns, []),
  };
}

export function parseCookieSkipCatalog(
  raw: string,
  fetchedAt = Date.now(),
): CookieSkipCatalogState {
  if (new TextEncoder().encode(raw).byteLength > MAX_COOKIE_SKIP_CATALOG_BYTES) {
    throw new Error('Cookie skip catalog is too large');
  }

  const document = validateCookieSkipCatalogDocument(JSON.parse(raw));
  return {
    patterns: document.recommendedPatterns,
    revision: document.revision,
    updatedAt: document.updatedAt,
    fetchedAt,
    source: 'remote',
    stale: false,
  };
}

function getFallbackCookieSkipCatalog(error?: string): CookieSkipCatalogState {
  return {
    patterns: [...DEFAULT_COOKIE_SKIP_PATTERNS],
    revision: DEFAULT_COOKIE_SKIP_CATALOG_REVISION,
    updatedAt: '2026-07-18T00:00:00Z',
    fetchedAt: null,
    source: 'fallback',
    stale: false,
    ...(error ? { error } : {}),
  };
}

export function loadCookieSkipRecommendations(now = Date.now()): CookieSkipCatalogState {
  if (typeof localStorage === 'undefined') {
    return getFallbackCookieSkipCatalog();
  }

  try {
    const raw = localStorage.getItem(COOKIE_SKIP_CATALOG_CACHE_KEY);
    if (!raw) {
      return getFallbackCookieSkipCatalog();
    }

    if (new TextEncoder().encode(raw).byteLength > MAX_COOKIE_SKIP_CATALOG_BYTES) {
      throw new Error('Cached cookie skip catalog is too large');
    }

    const cached = JSON.parse(raw) as Partial<CachedCookieSkipCatalog>;
    const document = validateCookieSkipCatalogDocument(cached);
    if (typeof cached.fetchedAt !== 'number' || !Number.isFinite(cached.fetchedAt)) {
      throw new Error('Invalid cached cookie skip catalog timestamp');
    }

    return {
      patterns: document.recommendedPatterns,
      revision: document.revision,
      updatedAt: document.updatedAt,
      fetchedAt: cached.fetchedAt,
      source: 'cache',
      stale: now - cached.fetchedAt >= COOKIE_SKIP_CATALOG_TTL_MS,
    };
  } catch (error) {
    console.warn('Failed to load cached cookie skip recommendations:', error);
    try {
      localStorage.removeItem(COOKIE_SKIP_CATALOG_CACHE_KEY);
    } catch (cleanupError) {
      console.warn('Failed to remove invalid cookie skip cache:', cleanupError);
    }
    return getFallbackCookieSkipCatalog();
  }
}

async function fetchCookieSkipRecommendations(
  cached: CookieSkipCatalogState,
  fetcher: typeof fetch,
  now: number,
): Promise<CookieSkipCatalogState> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), COOKIE_SKIP_CATALOG_TIMEOUT_MS);

  try {
    const response = await fetcher(COOKIE_SKIP_CATALOG_URL, {
      cache: 'no-store',
      headers: { Accept: 'application/json' },
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }

    const state = parseCookieSkipCatalog(await response.text(), now);
    if (typeof localStorage !== 'undefined') {
      const cache: CachedCookieSkipCatalog = {
        schemaVersion: 1,
        revision: state.revision,
        updatedAt: state.updatedAt,
        recommendedPatterns: state.patterns,
        fetchedAt: now,
      };
      try {
        localStorage.setItem(COOKIE_SKIP_CATALOG_CACHE_KEY, JSON.stringify(cache));
      } catch (cacheError) {
        console.warn('Failed to cache cookie skip recommendations:', cacheError);
      }
    }
    return state;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { ...cached, stale: cached.source === 'cache', error: message };
  } finally {
    clearTimeout(timeout);
  }
}

export async function refreshCookieSkipRecommendations(options?: {
  force?: boolean;
  fetcher?: typeof fetch;
  now?: number;
}): Promise<CookieSkipCatalogState> {
  const now = options?.now ?? Date.now();
  const cached = loadCookieSkipRecommendations(now);
  if (!options?.force && cached.source === 'cache' && !cached.stale) {
    return cached;
  }

  if (!options?.force && activeCookieSkipCatalogRefresh) {
    return activeCookieSkipCatalogRefresh;
  }

  const refresh = fetchCookieSkipRecommendations(cached, options?.fetcher ?? fetch, now);
  if (!options?.force) {
    activeCookieSkipCatalogRefresh = refresh;
  }

  try {
    return await refresh;
  } finally {
    if (activeCookieSkipCatalogRefresh === refresh) {
      activeCookieSkipCatalogRefresh = null;
    }
  }
}

export function normalizeCookieSettings(settings: CookieSettings): CookieSettings {
  const legacyPatterns = sanitizeCookieSkipPatterns(settings.cookieSkipPatterns, []);
  const hasLegacyRecommendedPattern = LEGACY_RECOMMENDED_COOKIE_SKIP_PATTERNS.some((pattern) =>
    legacyPatterns.includes(pattern),
  );
  const hasExplicitRecommendationSetting =
    typeof settings.useRecommendedCookieSkipPatterns === 'boolean';
  const useRecommendedCookieSkipPatterns = hasExplicitRecommendationSetting
    ? settings.useRecommendedCookieSkipPatterns
    : !Array.isArray(settings.cookieSkipPatterns) || hasLegacyRecommendedPattern;
  const personalPatterns =
    !hasExplicitRecommendationSetting && hasLegacyRecommendedPattern
      ? legacyPatterns.filter(
          (pattern) => !LEGACY_RECOMMENDED_COOKIE_SKIP_PATTERNS.includes(pattern),
        )
      : legacyPatterns;

  return {
    ...settings,
    useRecommendedCookieSkipPatterns,
    cookieSkipPatterns: personalPatterns,
  };
}

export function resolveCookieSkipPatterns(
  settings: CookieSettings,
  recommendedPatterns = loadCookieSkipRecommendations().patterns,
): string[] {
  const personalPatterns = sanitizeCookieSkipPatterns(settings.cookieSkipPatterns, []);
  if (settings.useRecommendedCookieSkipPatterns === false) {
    return personalPatterns;
  }

  return sanitizeCookieSkipPatterns([...recommendedPatterns, ...personalPatterns], []);
}

export function loadCookieSettings(): CookieSettings {
  try {
    const saved = localStorage.getItem(COOKIE_STORAGE_KEY);
    if (saved) {
      return normalizeCookieSettings(JSON.parse(saved));
    }
  } catch (error) {
    console.error('Failed to load cookie settings:', error);
  }
  return {
    mode: 'off',
    useRecommendedCookieSkipPatterns: true,
    cookieSkipPatterns: [],
  };
}

export function saveCookieSettings(settings: CookieSettings) {
  try {
    localStorage.setItem(COOKIE_STORAGE_KEY, JSON.stringify(normalizeCookieSettings(settings)));
  } catch (error) {
    console.error('Failed to save cookie settings:', error);
  }
}

export function loadProxySettings(): ProxySettings {
  try {
    const saved = localStorage.getItem(PROXY_STORAGE_KEY);
    if (saved) {
      return JSON.parse(saved);
    }
  } catch (error) {
    console.error('Failed to load proxy settings:', error);
  }
  return { mode: 'off' };
}

export function saveProxySettings(settings: ProxySettings) {
  try {
    localStorage.setItem(PROXY_STORAGE_KEY, JSON.stringify(settings));
  } catch (error) {
    console.error('Failed to save proxy settings:', error);
  }
}

export function buildProxyUrl(settings: ProxySettings): string | undefined {
  if (settings.mode === 'off' || !settings.host || !settings.port) {
    return undefined;
  }

  const protocol = settings.mode === 'socks5' ? 'socks5' : 'http';
  const auth =
    settings.username && settings.password
      ? `${encodeURIComponent(settings.username)}:${encodeURIComponent(settings.password)}@`
      : '';

  return `${protocol}://${auth}${settings.host}:${settings.port}`;
}

export function buildCookieProxyInvokeOptions(
  cookieSettings: CookieSettings,
  proxySettings: ProxySettings,
): CookieProxyInvokeOptions {
  return {
    cookieMode: cookieSettings.mode,
    cookieBrowser: cookieSettings.browser || null,
    cookieBrowserProfile: cookieSettings.browserProfile || null,
    cookieFilePath: cookieSettings.filePath || null,
    cookieSkipPatterns: resolveCookieSkipPatterns(cookieSettings),
    proxyUrl: buildProxyUrl(proxySettings) || null,
  };
}

export function loadNetworkSettings() {
  return {
    cookieSettings: loadCookieSettings(),
    proxySettings: loadProxySettings(),
  };
}
