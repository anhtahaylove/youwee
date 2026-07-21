import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';

const extensionRoot = new URL('../extensions/youwee-webext/', import.meta.url);
const repoRoot = new URL('../', import.meta.url);
const amoListingUrl = 'https://addons.mozilla.org/firefox/addon/youwee-download-companion/';

describe('Firefox extension validation', () => {
  test('declares current-page URL transmission as browsing activity', async () => {
    const manifest = JSON.parse(
      await readFile(new URL('manifest.firefox.json', extensionRoot), 'utf8'),
    );

    expect(manifest.browser_specific_settings.gecko.strict_min_version).toBe('140.0');
    expect(manifest.version).toBe('0.19.1.37');
    expect(manifest.browser_specific_settings.gecko.update_url).toBeUndefined();
    expect(manifest.browser_specific_settings.gecko_android.strict_min_version).toBe('142.0');
    expect(manifest.browser_specific_settings.gecko.data_collection_permissions).toEqual({
      required: ['browsingActivity'],
    });
  });

  test('requests only the permissions used by both store packages', async () => {
    for (const manifestName of ['manifest.firefox.json', 'manifest.chromium.json']) {
      const manifest = JSON.parse(await readFile(new URL(manifestName, extensionRoot), 'utf8'));

      expect(manifest.permissions).toEqual(['activeTab', 'storage', 'scripting']);
      expect(manifest.web_accessible_resources[0].matches).not.toContain('<all_urls>');
    }
  });

  test('builds Firefox packages for AMO without a self-hosted update URL', async () => {
    const buildScript = await readFile(new URL('scripts/build.mjs', extensionRoot), 'utf8');
    const packageScript = await readFile(new URL('scripts/package.mjs', extensionRoot), 'utf8');

    expect(buildScript).toContain("buildTarget('firefox-amo'");
    expect(buildScript).not.toContain(
      'delete manifest.browser_specific_settings?.gecko?.update_url',
    );
    expect(packageScript).toContain('Youwee-Extension-Firefox-AMO.zip');
  });

  test('bridges existing self-hosted users to the approved AMO build', async () => {
    const sourceManifest = JSON.parse(
      await readFile(new URL('manifest.firefox.json', extensionRoot), 'utf8'),
    );
    const bridge = JSON.parse(
      await readFile(new URL('firefox-amo-bridge.json', extensionRoot), 'utf8'),
    );
    const extensionId = sourceManifest.browser_specific_settings.gecko.id;
    const update = bridge.addons[extensionId].updates[0];

    expect(update.version).toBe(sourceManifest.version);
    expect(update.update_link).toBe(
      'https://addons.mozilla.org/firefox/downloads/file/4904844/youwee_download_companion-0.19.1.37.xpi',
    );
    expect(update.update_hash).toBe(
      'sha256:e71f177072f008d1990762573185f66334be8ce22adf52cbcb575503732dd7fd',
    );
    expect(update.applications.gecko.strict_min_version).toBe(
      sourceManifest.browser_specific_settings.gecko.strict_min_version,
    );
  });

  for (const sourceFile of ['src/content.js', 'src/popup.js']) {
    test(`${sourceFile} avoids innerHTML assignments rejected by AMO`, async () => {
      const source = await readFile(new URL(sourceFile, extensionRoot), 'utf8');
      expect(source).not.toContain('innerHTML');
    });
  }

  test('publishes AMO as stable while retaining the one-time migration bridge', async () => {
    const workflow = await readFile(new URL('.github/workflows/build.yml', repoRoot), 'utf8');

    expect(workflow).toContain('Validate Firefox AMO migration bridge');
    expect(workflow).toContain('firefox-updates.json');
    expect(workflow).toContain(amoListingUrl);
    expect(workflow).not.toContain('web-ext sign');
    expect(workflow).not.toContain('--channel unlisted');
    expect(workflow).not.toContain('Youwee-Extension-Firefox-signed.xpi');
  });
});
