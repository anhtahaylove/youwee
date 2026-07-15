import { describe, expect, test } from 'bun:test';
import { updaterRestartsAutomatically } from '../src/hooks/useAppUpdater';

describe('app updater platform behavior', () => {
  test('expects the Windows installer to restart the app automatically', () => {
    expect(updaterRestartsAutomatically('Win32')).toBe(true);
    expect(updaterRestartsAutomatically('MacIntel')).toBe(false);
    expect(updaterRestartsAutomatically('Linux x86_64')).toBe(false);
  });
});
