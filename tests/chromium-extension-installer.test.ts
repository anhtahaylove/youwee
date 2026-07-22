import { describe, expect, test } from 'bun:test';
import { readdir, readFile } from 'node:fs/promises';

const repoRoot = new URL('../', import.meta.url);
const bundledExtensionResource = '../extensions/youwee-webext/dist/chromium';
const bundledExtensionTarget = 'Youwee-Extension-Chromium';
const bundledFirefoxResource =
  'target/dependency-cache/windows/Youwee-Extension-Firefox-signed.xpi';
const bundledFirefoxTarget = 'Youwee-Extension-Firefox-signed.xpi';
const localizedKeys = [
  'chromiumBundledDesc',
  'bundledReady',
  'openBundledFolder',
  'copyFolderPath',
  'copyExtensionsAddress',
  'chromiumBundledStep1',
  'chromiumBundledStep2',
  'openBundledFolderError',
  'copyInstallValueError',
  'recommended',
  'installFromAmo',
  'firefoxAdvanced',
  'firefoxAdvancedDesc',
  'firefoxBundledDesc',
  'firefoxBundledAdvancedDesc',
  'openBundledXpi',
];

describe('Windows Chromium extension installer', () => {
  test('refreshes and validates the extension before packaging', async () => {
    const script = await readFile(
      new URL('scripts/prepare-windows-dependencies.ps1', repoRoot),
      'utf8',
    );

    expect(script).toContain('& bun run ext:build');
    expect(script).toContain('$chromiumManifest.manifest_version -ne 3');
    expect(script).toContain(`'${bundledExtensionResource}' = '${bundledExtensionTarget}'`);
    expect(script).toContain('firefox-amo-bridge.json');
    expect(script).toContain(`'${bundledFirefoxResource}' = '${bundledFirefoxTarget}'`);
  });

  test('adds an offline bilingual install guide to the Chromium bundle', async () => {
    const buildScript = await readFile(
      new URL('extensions/youwee-webext/scripts/build.mjs', repoRoot),
      'utf8',
    );

    expect(buildScript).toContain("path.join(outDir, 'INSTALL.txt')");
    expect(buildScript).toContain('Open chrome://extensions');
    expect(buildScript).toContain('Mở chrome://extensions');
  });

  test('normalizes the Windows resource path before opening or copying it', async () => {
    const section = await readFile(
      new URL('src/components/settings/sections/ExtensionSection.tsx', repoRoot),
      'utf8',
    );

    expect(section).toContain('normalizeAssetPath(path)');
  });

  test('recommends AMO while keeping the signed XPI in advanced installation', async () => {
    const section = await readFile(
      new URL('src/components/settings/sections/ExtensionSection.tsx', repoRoot),
      'utf8',
    );

    expect(section).toContain(
      'https://addons.mozilla.org/firefox/addon/youwee-download-companion/',
    );
    expect(section).toContain(
      'https://addons.mozilla.org/firefox/downloads/latest/youwee-download-companion/latest.xpi',
    );
    expect(section).not.toContain('Youwee-Extension-Firefox-signed.xpi');
    expect(section).toContain("invoke<string | null>('get_bundled_firefox_extension_path')");
    expect(section).toContain('openBundledFirefoxPackage');
    expect(section).toContain("t('extension.openBundledXpi')");
    expect(section).toContain("t('extension.installFromAmo')");
    expect(section).toContain("t('extension.firefoxAdvanced')");
    expect(section).toContain('<CollapsibleContent>');
    expect(section.indexOf('FIREFOX_AMO_URL')).toBeLessThan(
      section.indexOf('FIREFOX_DOWNLOAD_URL'),
    );
  });

  test('provides beginner setup copy in every locale', async () => {
    const localesRoot = new URL('src/i18n/locales/', repoRoot);
    const localeDirs = (await readdir(localesRoot, { withFileTypes: true })).filter((entry) =>
      entry.isDirectory(),
    );

    expect(localeDirs.length).toBeGreaterThan(0);
    for (const localeDir of localeDirs) {
      const settings = JSON.parse(
        await readFile(new URL(`${localeDir.name}/settings.json`, localesRoot), 'utf8'),
      );
      for (const key of localizedKeys) {
        expect(
          settings.extension[key],
          `${localeDir.name} is missing extension.${key}`,
        ).toBeTruthy();
      }
    }
  });
});
