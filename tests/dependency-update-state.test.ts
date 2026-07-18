import { describe, expect, test } from 'bun:test';
import { resolveManagedDependencyUpdateState } from '../src/lib/dependency-update-state';

describe('managed dependency update status', () => {
  test('shows a completed update check before the packaged fallback', () => {
    expect(
      resolveManagedDependencyUpdateState({
        updateInfo: { has_update: false },
        isSystem: false,
        isBundled: true,
      }),
    ).toBe('up-to-date');

    expect(
      resolveManagedDependencyUpdateState({
        updateInfo: { has_update: true },
        isSystem: false,
        isBundled: true,
      }),
    ).toBe('update-available');
  });

  test('keeps source fallbacks until an app-managed update check completes', () => {
    expect(
      resolveManagedDependencyUpdateState({
        updateInfo: null,
        isSystem: false,
        isBundled: true,
      }),
    ).toBe('packaged');

    expect(
      resolveManagedDependencyUpdateState({
        updateInfo: { has_update: false },
        isSystem: true,
        isBundled: false,
      }),
    ).toBe('system');
  });
});
