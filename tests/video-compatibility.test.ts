import { describe, expect, test } from 'bun:test';
import {
  applyVideoCompatibilityPreset,
  normalizeVideoCompatibilityForMedia,
  normalizeVideoCompatibilityMode,
  resolveItemVideoCompatibilityMode,
} from '../src/lib/video-compatibility';

describe('video compatibility presets', () => {
  test('defaults new and legacy settings to Original', () => {
    expect(normalizeVideoCompatibilityMode(undefined)).toBe('original');
    expect(normalizeVideoCompatibilityMode('unknown')).toBe('original');
    expect(resolveItemVideoCompatibilityMode(undefined)).toBe('original');
    expect(resolveItemVideoCompatibilityMode({})).toBe('original');
  });

  test('Compatible H.264 keeps selected source quality and forces MP4', () => {
    expect(
      applyVideoCompatibilityPreset(
        {
          quality: 'best',
          format: 'webm',
          videoCodec: 'vp9',
          videoCompatibilityMode: 'original',
          preferredFps: '30',
        },
        'h264',
      ),
    ).toEqual({
      quality: 'best',
      format: 'mp4',
      videoCodec: 'auto',
      videoCompatibilityMode: 'h264',
      preferredFps: 'original',
    });
  });

  test('Compatible H.264 switches an audio-only selection back to best video', () => {
    expect(
      applyVideoCompatibilityPreset(
        {
          quality: 'audio',
          format: 'mp3',
          videoCodec: 'auto',
          videoCompatibilityMode: 'original',
          preferredFps: 'original',
        },
        'h264',
      ),
    ).toEqual({
      quality: 'best',
      format: 'mp4',
      videoCodec: 'auto',
      videoCompatibilityMode: 'h264',
      preferredFps: 'original',
    });
  });

  test('audio and non-MP4 outputs cannot retain H.264 compatibility mode', () => {
    expect(normalizeVideoCompatibilityForMedia('audio', 'mp3', 'h264')).toBe('original');
    expect(normalizeVideoCompatibilityForMedia('best', 'mkv', 'h264')).toBe('original');
    expect(normalizeVideoCompatibilityForMedia('best', 'mp4', 'h264')).toBe('h264');
  });
});
