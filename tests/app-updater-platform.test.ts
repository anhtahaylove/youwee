import { describe, expect, test } from 'bun:test';
import {
  calculateUpdateTransferStats,
  restoreInstalledUpdate,
  updaterRestartsAutomatically,
} from '../src/hooks/useAppUpdater';

describe('app updater platform behavior', () => {
  test('expects the Windows installer to restart the app automatically', () => {
    expect(updaterRestartsAutomatically('Win32')).toBe(true);
    expect(updaterRestartsAutomatically('MacIntel')).toBe(false);
    expect(updaterRestartsAutomatically('Linux x86_64')).toBe(false);
  });
});

describe('post-update release notes', () => {
  const update = {
    version: '0.19.1-custom.35',
    currentVersion: '0.19.1-custom.34',
    body: 'Release notes',
  };

  test('restores notes only after the target version is running', () => {
    const raw = JSON.stringify(update);
    expect(restoreInstalledUpdate(raw, update.version)).toEqual(update);
    expect(restoreInstalledUpdate(raw, update.currentVersion)).toBeNull();
  });

  test('ignores malformed persisted state', () => {
    expect(restoreInstalledUpdate('{invalid', update.version)).toBeNull();
  });
});

describe('update transfer statistics', () => {
  test('calculates current speed and remaining time', () => {
    const stats = calculateUpdateTransferStats(5_000_000, 15_000_000, 1_000_000, 2_000);

    expect(stats.bytesPerSecond).toBe(2_000_000);
    expect(stats.etaSeconds).toBe(5);
  });

  test('smooths later samples to avoid a jumping ETA', () => {
    const stats = calculateUpdateTransferStats(3_000_000, 10_000_000, 2_000_000, 1_000, 2_000_000);

    expect(stats.bytesPerSecond).toBe(1_700_000);
    expect(stats.etaSeconds).toBe(5);
  });
});
