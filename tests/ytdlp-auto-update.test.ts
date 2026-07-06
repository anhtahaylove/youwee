import { describe, expect, test } from 'bun:test';
import {
  createYtdlpUpdateNoticeKey,
  createYtdlpUpdateToastAction,
  isYtdlpAutoUpdateCheckDue,
  shouldNotifyYtdlpUpdate,
  YTDLP_AUTO_UPDATE_CHECK_INTERVAL_MS,
} from '../src/lib/ytdlp-auto-update';

describe('yt-dlp auto update checks', () => {
  test('runs when no previous check exists or the previous check is stale', () => {
    const now = 200_000_000;

    expect(isYtdlpAutoUpdateCheckDue(null, now)).toBe(true);
    expect(isYtdlpAutoUpdateCheckDue('not-a-number', now)).toBe(true);
    expect(
      isYtdlpAutoUpdateCheckDue(String(now - YTDLP_AUTO_UPDATE_CHECK_INTERVAL_MS - 1), now),
    ).toBe(true);
  });

  test('skips checks inside the daily interval', () => {
    const now = 200_000_000;

    expect(
      isYtdlpAutoUpdateCheckDue(String(now - YTDLP_AUTO_UPDATE_CHECK_INTERVAL_MS + 1), now),
    ).toBe(false);
  });

  test('notifies once for each channel and latest version', () => {
    const noticeKey = createYtdlpUpdateNoticeKey('stable', 'v2026.06.09');

    expect(shouldNotifyYtdlpUpdate(true, 'stable', 'v2026.06.09', null)).toBe(noticeKey);
    expect(shouldNotifyYtdlpUpdate(true, 'stable', 'v2026.06.09', noticeKey)).toBeNull();
    expect(shouldNotifyYtdlpUpdate(false, 'stable', 'v2026.06.09', null)).toBeNull();
    expect(shouldNotifyYtdlpUpdate(true, 'stable', '', null)).toBeNull();
  });

  test('toast action opens Dependencies and starts the update', () => {
    const events: string[] = [];
    const action = createYtdlpUpdateToastAction({
      toastId: 'ytdlp-update-stable',
      dismissToast: (id) => events.push(`dismiss:${id}`),
      openDependencies: () => events.push('open:dependencies'),
      startUpdate: () => events.push('start:update'),
    });

    action();

    expect(events).toEqual(['dismiss:ytdlp-update-stable', 'open:dependencies', 'start:update']);
  });
});
