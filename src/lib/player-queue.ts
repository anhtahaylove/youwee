export interface MinimalHistoryEntry {
  id: string;
  filepath: string;
  file_exists: boolean;
  format?: string;
  quality?: string;
}

const DIRECT_AUDIO_FORMATS = new Set(['mp3', 'm4a', 'opus', 'ogg', 'flac', 'wav']);
const CONDITIONAL_AUDIO_FORMATS = new Set(['webm']);

function normalizeValue(value?: string): string {
  return value?.trim().toLowerCase() ?? '';
}

function getEntryFormat(entry: MinimalHistoryEntry): string {
  const explicitFormat = normalizeValue(entry.format);
  if (explicitFormat) return explicitFormat;

  const filepath = entry.filepath.trim();
  const dotIndex = filepath.lastIndexOf('.');
  if (dotIndex === -1) return '';
  return normalizeValue(filepath.slice(dotIndex + 1));
}

export function isPlayableAudioEntry(entry: MinimalHistoryEntry): boolean {
  if (!entry.file_exists || !entry.filepath.trim()) return false;

  const format = getEntryFormat(entry);
  const quality = normalizeValue(entry.quality);

  if (DIRECT_AUDIO_FORMATS.has(format)) return true;
  if (CONDITIONAL_AUDIO_FORMATS.has(format) && quality === 'audio') return true;

  return false;
}

export function buildPlayableAudioQueue<T extends MinimalHistoryEntry>(entries: T[]): T[] {
  return entries.filter(isPlayableAudioEntry);
}

export interface ReconciledPlayableQueue<T extends MinimalHistoryEntry> {
  queue: T[];
  currentIndex: number;
  removedCurrent: boolean;
}

export function reconcilePlayableAudioQueue<T extends MinimalHistoryEntry>(
  currentQueue: T[],
  currentIndex: number,
  availableEntries: T[],
): ReconciledPlayableQueue<T> {
  const playableEntries = buildPlayableAudioQueue(availableEntries);
  const playableById = new Map(playableEntries.map((entry) => [entry.id, entry]));
  const currentEntryId = currentQueue[currentIndex]?.id;

  const nextQueue = currentQueue
    .filter((entry) => playableById.has(entry.id))
    .map((entry) => playableById.get(entry.id) ?? entry);

  if (nextQueue.length === 0) {
    return {
      queue: [],
      currentIndex: 0,
      removedCurrent: currentEntryId != null,
    };
  }

  if (currentEntryId) {
    const preservedIndex = nextQueue.findIndex((entry) => entry.id === currentEntryId);
    if (preservedIndex !== -1) {
      return {
        queue: nextQueue,
        currentIndex: preservedIndex,
        removedCurrent: false,
      };
    }
  }

  const survivingBeforeCurrent = currentQueue
    .slice(0, Math.max(0, currentIndex))
    .filter((entry) => playableById.has(entry.id)).length;
  const fallbackIndex = Math.min(survivingBeforeCurrent, nextQueue.length - 1);

  return {
    queue: nextQueue,
    currentIndex: Math.max(0, fallbackIndex),
    removedCurrent: currentEntryId != null,
  };
}
