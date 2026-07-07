import { describe, expect, test } from 'bun:test';
import {
  normalizeUniversalFormatCodec,
  normalizeUniversalVideoCodec,
  resolveUniversalVideoCodec,
} from '../src/lib/universal-settings';

describe('universal codec settings', () => {
  test('defaults unsupported codecs to auto', () => {
    expect(normalizeUniversalVideoCodec(undefined)).toBe('auto');
    expect(normalizeUniversalVideoCodec('hevc')).toBe('auto');
  });

  test('keeps WebM from using H.264', () => {
    expect(normalizeUniversalFormatCodec('webm', 'h264')).toBe('auto');
    expect(normalizeUniversalFormatCodec('mp4', 'h264')).toBe('h264');
  });

  test('old queued items without codec stay auto', () => {
    expect(resolveUniversalVideoCodec(undefined)).toBe('auto');
    expect(resolveUniversalVideoCodec({})).toBe('auto');
    expect(resolveUniversalVideoCodec({ videoCodec: 'vp9' })).toBe('vp9');
  });
});
