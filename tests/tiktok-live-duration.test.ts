import { describe, expect, test } from 'bun:test';
import {
  formatTikTokLiveDuration,
  formatTikTokLiveDurationSetting,
  joinTikTokLiveDuration,
  splitTikTokLiveDuration,
} from '../src/lib/tiktok-live-duration';

describe('TikTok Live duration helpers', () => {
  test('preserves exact hour, minute, and second input', () => {
    expect(joinTikTokLiveDuration('1', '30', '5')).toBe('5405');
    expect(splitTikTokLiveDuration(5405)).toEqual({
      hours: '1',
      minutes: '30',
      seconds: '5',
    });
  });

  test('clamps minute and second fields instead of changing units implicitly', () => {
    expect(joinTikTokLiveDuration('0', '99', '99')).toBe('3599');
  });

  test('formats long recordings from days down to seconds', () => {
    expect(formatTikTokLiveDuration(95405)).toBe('1d 2h 30m 5s');
    expect(formatTikTokLiveDuration(0)).toBe('0s');
    expect(formatTikTokLiveDurationSetting('0')).toBe('∞');
  });
});
