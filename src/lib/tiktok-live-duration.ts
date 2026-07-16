export function parseTikTokLiveDurationPart(value: string, max?: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return 0;
  return Math.max(0, max === undefined ? parsed : Math.min(max, parsed));
}

export function splitTikTokLiveDuration(seconds: number): {
  hours: string;
  minutes: string;
  seconds: string;
} {
  const safeSeconds = Math.max(0, Math.floor(seconds));
  return {
    hours: Math.floor(safeSeconds / 3600).toString(),
    minutes: Math.floor((safeSeconds % 3600) / 60).toString(),
    seconds: (safeSeconds % 60).toString(),
  };
}

export function joinTikTokLiveDuration(hours: string, minutes: string, seconds: string): string {
  const total =
    parseTikTokLiveDurationPart(hours) * 3600 +
    parseTikTokLiveDurationPart(minutes, 59) * 60 +
    parseTikTokLiveDurationPart(seconds, 59);
  return total > 0 ? total.toString() : '';
}

export function formatTikTokLiveDuration(seconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(seconds));
  const units = [
    [Math.floor(safeSeconds / 86400), 'd'],
    [Math.floor((safeSeconds % 86400) / 3600), 'h'],
    [Math.floor((safeSeconds % 3600) / 60), 'm'],
    [safeSeconds % 60, 's'],
  ] as const;
  const formatted = units
    .filter(([value]) => value > 0)
    .map(([value, unit]) => `${value}${unit}`)
    .join(' ');
  return formatted || '0s';
}

export function formatTikTokLiveDurationSetting(secondsValue: string): string {
  const seconds = Number.parseInt(secondsValue, 10);
  if (!Number.isFinite(seconds) || seconds <= 0) return '∞';
  return formatTikTokLiveDuration(seconds);
}
