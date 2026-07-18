import { describe, expect, test } from 'bun:test';
import {
  deriveWindowsInstallerVersion,
  parseArgs,
  promoteUnreleased,
  updateAppVersionFile,
} from '../scripts/bump-custom-version.mjs';

describe('custom version bump', () => {
  test('derives the four-part Windows Installer version', () => {
    expect(deriveWindowsInstallerVersion('0.19.1-custom.42')).toBe('0.19.1.42');
  });

  test('updates only the selected app version field', () => {
    expect(
      updateAppVersionFile(
        'package.json',
        '{\n  "version": "0.19.1-custom.41"\n}\n',
        '0.19.1-custom.41',
        '0.19.1-custom.42',
      ),
    ).toContain('"version": "0.19.1-custom.42"');
  });

  test('promotes Unreleased content without discarding it', () => {
    const changelog = '# Changelog\n\n## [Unreleased]\n\n### Fixed\n- Keep this entry\n';
    expect(promoteUnreleased(changelog, '0.19.1-custom.42', '2026-07-19')).toBe(
      '# Changelog\n\n## [Unreleased]\n\n## [0.19.1-custom.42] - 2026-07-19\n\n### Fixed\n- Keep this entry\n',
    );
  });

  test('accepts options before the target version without confusing the date', () => {
    expect(parseArgs(['--date', '2026-07-19', '--write', '0.19.1-custom.42'])).toEqual({
      date: '2026-07-19',
      help: false,
      version: '0.19.1-custom.42',
      write: true,
    });
  });
});
