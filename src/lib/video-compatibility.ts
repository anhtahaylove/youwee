import type {
  Format,
  ItemDownloadSettings,
  ItemUniversalSettings,
  PreferredFps,
  Quality,
  VideoCodec,
  VideoCompatibilityMode,
} from '@/lib/types';

const VIDEO_COMPATIBILITY_MODES = new Set<VideoCompatibilityMode>(['original', 'h264']);

export function normalizeVideoCompatibilityMode(value: unknown): VideoCompatibilityMode {
  return typeof value === 'string' && VIDEO_COMPATIBILITY_MODES.has(value as VideoCompatibilityMode)
    ? (value as VideoCompatibilityMode)
    : 'original';
}

export function resolveItemVideoCompatibilityMode(
  itemSettings:
    | Partial<Pick<ItemDownloadSettings | ItemUniversalSettings, 'videoCompatibilityMode'>>
    | null
    | undefined,
): VideoCompatibilityMode {
  return normalizeVideoCompatibilityMode(itemSettings?.videoCompatibilityMode);
}

export function normalizeVideoCompatibilityForMedia(
  quality: Quality,
  format: Format,
  mode: unknown,
): VideoCompatibilityMode {
  const normalized = normalizeVideoCompatibilityMode(mode);
  return quality === 'audio' || format !== 'mp4' ? 'original' : normalized;
}

export function applyVideoCompatibilityPreset<
  T extends {
    quality: Quality;
    format: Format;
    videoCodec: VideoCodec;
    videoCompatibilityMode: VideoCompatibilityMode;
    preferredFps: PreferredFps;
  },
>(settings: T, mode: VideoCompatibilityMode): T {
  if (mode === 'h264') {
    return {
      ...settings,
      quality: settings.quality === 'audio' ? 'best' : settings.quality,
      format: 'mp4',
      videoCodec: 'auto',
      videoCompatibilityMode: 'h264',
      preferredFps: 'original',
    };
  }

  return {
    ...settings,
    videoCodec: 'auto',
    videoCompatibilityMode: 'original',
    preferredFps: 'original',
  };
}
