import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';

const extensionRoot = new URL('../extensions/youwee-webext/', import.meta.url);

describe('Firefox extension validation', () => {
  test('declares current-page URL transmission as browsing activity', async () => {
    const manifest = JSON.parse(
      await readFile(new URL('manifest.firefox.json', extensionRoot), 'utf8'),
    );

    expect(manifest.browser_specific_settings.gecko.strict_min_version).toBe('140.0');
    expect(manifest.browser_specific_settings.gecko_android.strict_min_version).toBe('142.0');
    expect(manifest.browser_specific_settings.gecko.data_collection_permissions).toEqual({
      required: ['browsingActivity'],
    });
  });

  for (const sourceFile of ['src/content.js', 'src/popup.js']) {
    test(`${sourceFile} avoids innerHTML assignments rejected by AMO`, async () => {
      const source = await readFile(new URL(sourceFile, extensionRoot), 'utf8');
      expect(source).not.toContain('innerHTML');
    });
  }
});
