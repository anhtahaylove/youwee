import type { Format, ItemUniversalSettings, VideoCodec } from '@/lib/types';

const UNIVERSAL_VIDEO_CODECS = new Set<VideoCodec>(['auto', 'h264', 'vp9', 'av1']);

export function normalizeUniversalVideoCodec(value: unknown): VideoCodec {
  return typeof value === 'string' && UNIVERSAL_VIDEO_CODECS.has(value as VideoCodec)
    ? (value as VideoCodec)
    : 'auto';
}

export function normalizeUniversalFormatCodec(format: Format, value: unknown): VideoCodec {
  const codec = normalizeUniversalVideoCodec(value);
  return format === 'webm' && codec === 'h264' ? 'auto' : codec;
}

export function resolveUniversalVideoCodec(
  itemSettings: Partial<Pick<ItemUniversalSettings, 'videoCodec'>> | null | undefined,
): VideoCodec {
  return itemSettings && 'videoCodec' in itemSettings
    ? normalizeUniversalVideoCodec(itemSettings.videoCodec)
    : 'auto';
}
