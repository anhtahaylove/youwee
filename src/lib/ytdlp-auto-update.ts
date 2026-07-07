import type { YtdlpChannel } from '@/lib/types';

export const YTDLP_AUTO_UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
export const YTDLP_AUTO_UPDATE_LAST_CHECKED_KEY = 'youwee_ytdlp_auto_update_last_checked_at';
export const YTDLP_AUTO_UPDATE_LAST_NOTIFIED_KEY = 'youwee_ytdlp_auto_update_last_notified';

export function isYtdlpAutoUpdateCheckDue(
  lastCheckedAt: string | null,
  now = Date.now(),
  intervalMs = YTDLP_AUTO_UPDATE_CHECK_INTERVAL_MS,
): boolean {
  if (!lastCheckedAt) return true;
  const lastChecked = Number(lastCheckedAt);
  if (!Number.isFinite(lastChecked) || lastChecked <= 0) return true;
  return now - lastChecked >= intervalMs;
}

export function createYtdlpUpdateNoticeKey(channel: YtdlpChannel, latestVersion: string): string {
  return `${channel}:${latestVersion.trim().replace(/^v/i, '')}`;
}

export function shouldNotifyYtdlpUpdate(
  updateAvailable: boolean,
  channel: YtdlpChannel,
  latestVersion: string | null | undefined,
  lastNotifiedKey: string | null,
): string | null {
  const normalizedVersion = latestVersion?.trim();
  if (!updateAvailable || !normalizedVersion) return null;
  const noticeKey = createYtdlpUpdateNoticeKey(channel, normalizedVersion);
  return noticeKey === lastNotifiedKey ? null : noticeKey;
}

export function createYtdlpUpdateToastAction({
  toastId,
  dismissToast,
  openDependencies,
  startUpdate,
}: {
  toastId: string;
  dismissToast: (id: string) => void;
  openDependencies: () => void;
  startUpdate: () => void;
}) {
  return () => {
    dismissToast(toastId);
    openDependencies();
    startUpdate();
  };
}

export function readYtdlpAutoUpdateLastChecked(): string | null {
  if (typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(YTDLP_AUTO_UPDATE_LAST_CHECKED_KEY);
  } catch {
    return null;
  }
}

export function writeYtdlpAutoUpdateLastChecked(now = Date.now()) {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(YTDLP_AUTO_UPDATE_LAST_CHECKED_KEY, String(now));
  } catch {
    // Best effort only.
  }
}

export function readYtdlpAutoUpdateLastNotified(): string | null {
  if (typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(YTDLP_AUTO_UPDATE_LAST_NOTIFIED_KEY);
  } catch {
    return null;
  }
}

export function writeYtdlpAutoUpdateLastNotified(noticeKey: string) {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(YTDLP_AUTO_UPDATE_LAST_NOTIFIED_KEY, noticeKey);
  } catch {
    // Best effort only.
  }
}
