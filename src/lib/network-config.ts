import type { CookieSettings, ProxySettings } from '@/lib/types';

export const COOKIE_STORAGE_KEY = 'youwee-cookie-settings';
export const PROXY_STORAGE_KEY = 'youwee-proxy-settings';

export type CookieProxyInvokeOptions = {
  cookieMode: CookieSettings['mode'];
  cookieBrowser: CookieSettings['browser'] | null;
  cookieBrowserProfile: string | null;
  cookieFilePath: string | null;
  proxyUrl: string | null;
};

export function loadCookieSettings(): CookieSettings {
  try {
    const saved = localStorage.getItem(COOKIE_STORAGE_KEY);
    if (saved) {
      return JSON.parse(saved);
    }
  } catch (error) {
    console.error('Failed to load cookie settings:', error);
  }
  return { mode: 'off' };
}

export function saveCookieSettings(settings: CookieSettings) {
  try {
    localStorage.setItem(COOKIE_STORAGE_KEY, JSON.stringify(settings));
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
    proxyUrl: buildProxyUrl(proxySettings) || null,
  };
}

export function loadNetworkSettings() {
  return {
    cookieSettings: loadCookieSettings(),
    proxySettings: loadProxySettings(),
  };
}
