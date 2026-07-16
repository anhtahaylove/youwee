import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';

const extensionRoot = new URL('../extensions/youwee-webext/', import.meta.url);
const repoRoot = new URL('../', import.meta.url);
const firefoxUpdateUrl =
  'https://github.com/anhtahaylove/youwee/releases/latest/download/firefox-updates.json';

describe('Firefox extension validation', () => {
  test('declares current-page URL transmission as browsing activity', async () => {
    const manifest = JSON.parse(
      await readFile(new URL('manifest.firefox.json', extensionRoot), 'utf8'),
    );

    expect(manifest.browser_specific_settings.gecko.strict_min_version).toBe('140.0');
    expect(manifest.browser_specific_settings.gecko.update_url).toBe(firefoxUpdateUrl);
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

  test('publishes a signed GitHub release manifest for Firefox updates', async () => {
    const workflow = await readFile(new URL('.github/workflows/build.yml', repoRoot), 'utf8');

    expect(workflow).toContain('Youwee-Extension-Firefox-signed.xpi');
    expect(workflow).toContain('firefox-updates.json');
    expect(workflow).toContain('--self-hosted --warnings-as-errors');
    expect(workflow).toContain('unzip -p "$FIREFOX_XPI" manifest.json');
    expect(workflow).toContain('update_hash: ("sha256:" + $hash)');
    expect(workflow).toContain(
      'releases/download/$GITHUB_REF_NAME/Youwee-Extension-Firefox-signed.xpi',
    );
  });
});
