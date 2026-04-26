import { describe, expect, test } from 'bun:test';
import { collectAssetScopeCandidates, normalizeAssetPath } from '../src/lib/asset-paths';

describe('normalizeAssetPath', () => {
  test('strips the Windows extended-length prefix and trims whitespace', () => {
    expect(normalizeAssetPath('  \\\\?\\D:\\Music\\track.mp3  ')).toBe('D:\\Music\\track.mp3');
  });

  test('keeps regular paths unchanged', () => {
    expect(normalizeAssetPath('C:\\Users\\86153\\Downloads\\song.mp3')).toBe(
      'C:\\Users\\86153\\Downloads\\song.mp3',
    );
  });
});

describe('collectAssetScopeCandidates', () => {
  test('normalizes, removes empties, and de-duplicates while preserving order', () => {
    expect(
      collectAssetScopeCandidates([
        '  \\\\?\\D:\\Music\\track.mp3  ',
        '',
        'D:\\Music\\track.mp3',
        'C:\\Users\\86153\\Downloads',
        'C:\\Users\\86153\\Downloads',
      ]),
    ).toEqual(['D:\\Music\\track.mp3', 'C:\\Users\\86153\\Downloads']);
  });
});
