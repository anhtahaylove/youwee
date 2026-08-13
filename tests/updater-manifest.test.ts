import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import { parseUpdaterManifest } from '@/lib/updater-manifest';

const fixtureUrl = new URL('./fixtures/latest.custom.json', import.meta.url);

describe('custom updater manifest', () => {
  test('accepts the signed custom-release fixture for every packaged platform', async () => {
    const manifest = parseUpdaterManifest(JSON.parse(await readFile(fixtureUrl, 'utf8')));

    expect(manifest.version).toBe('0.20.1-custom.2');
    expect(manifest.notes_vi).toBeTruthy();
    expect(manifest['notes_zh-CN']).toBeTruthy();
    expect(Object.keys(manifest.platforms).sort()).toEqual([
      'darwin-aarch64',
      'darwin-x86_64',
      'linux-x86_64',
      'windows-x86_64',
    ]);
  });

  test('rejects a platform without its updater signature', async () => {
    const raw = JSON.parse(await readFile(fixtureUrl, 'utf8'));
    raw.platforms['windows-x86_64'].signature = '';

    expect(() => parseUpdaterManifest(raw)).toThrow('windows-x86_64.signature');
  });
});
