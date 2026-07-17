import { describe, expect, test } from 'bun:test';
import { restoreInstalledUpdate, updaterRestartsAutomatically } from '../src/hooks/useAppUpdater';

describe('app updater platform behavior', () => {
  test('expects the Windows installer to restart the app automatically', () => {
    expect(updaterRestartsAutomatically('Win32')).toBe(true);
    expect(updaterRestartsAutomatically('MacIntel')).toBe(false);
    expect(updaterRestartsAutomatically('Linux x86_64')).toBe(false);
  });
});

describe('post-update release notes', () => {
  const update = {
    version: '0.19.1-custom.34',
    currentVersion: '0.19.1-custom.33',
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
