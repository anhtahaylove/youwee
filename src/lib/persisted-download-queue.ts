import { invoke } from '@tauri-apps/api/core';
import type { DownloadItem, DownloadProgress, HistoryFileState } from './types';

export type PersistedQueueKind = 'youtube' | 'universal' | 'gallery';

function normalizeQueueItem(item: DownloadItem): DownloadItem {
  const isTransient = item.status === 'fetching' || item.status === 'downloading';
  const shouldResetProgress = isTransient || item.retryState !== undefined;
  const shouldClearError = isTransient || item.status !== 'error';

  return {
    ...item,
    status: isTransient ? 'pending' : item.status,
    progress: shouldResetProgress ? 0 : item.progress,
    speed: '',
    eta: '',
    error: shouldClearError ? undefined : item.error,
    downloadedSize: isTransient ? undefined : item.downloadedSize,
    elapsedTime: isTransient ? undefined : item.elapsedTime,
    retryState: undefined,
  };
}

export function normalizeDownloadQueueItems(items: DownloadItem[]): DownloadItem[] {
  return items.map(normalizeQueueItem);
}

function resetCompletedItemForMissingOutput(item: DownloadItem): DownloadItem {
  return {
    ...item,
    status: 'pending',
    progress: 0,
    speed: '',
    eta: '',
    error: undefined,
    errorCode: 'OUTPUT_FILE_MISSING',
    retryState: undefined,
    completedFilesize: undefined,
    completedResolution: undefined,
    completedFormat: undefined,
    completedFilepath: undefined,
    outputCollisionPolicy: 'overwrite',
  };
}

export function resetMissingCompletedQueueItems(
  items: DownloadItem[],
  missingFilepaths: ReadonlySet<string>,
): DownloadItem[] {
  let changed = false;
  const nextItems = items.map((item) => {
    if (
      item.status !== 'completed' ||
      !item.completedFilepath ||
      !missingFilepaths.has(item.completedFilepath)
    ) {
      return item;
    }

    changed = true;
    return resetCompletedItemForMissingOutput(item);
  });

  return changed ? nextItems : items;
}

export function isDownloadProgressForItem(
  item: Pick<DownloadItem, 'id' | 'completedHistoryId'>,
  progress: Pick<DownloadProgress, 'id' | 'history_id'>,
): boolean {
  return (
    item.id === progress.id ||
    Boolean(item.completedHistoryId && item.completedHistoryId === progress.history_id)
  );
}

export function reconcileQueueItemsWithHistoryStates(
  items: DownloadItem[],
  historyStates: readonly HistoryFileState[],
  missingLegacyFilepaths: ReadonlySet<string> = new Set(),
): DownloadItem[] {
  const stateByHistoryId = new Map(historyStates.map((state) => [state.historyId, state]));
  let changed = false;

  const nextItems = items.map((item) => {
    if (!item.completedHistoryId) {
      if (
        item.status === 'completed' &&
        item.completedFilepath &&
        missingLegacyFilepaths.has(item.completedFilepath)
      ) {
        changed = true;
        return resetCompletedItemForMissingOutput(item);
      }
      return item;
    }

    const state = stateByHistoryId.get(item.completedHistoryId);
    if (!state) return item;

    if (!state.fileExists) {
      if (item.status !== 'completed') return item;
      changed = true;
      return resetCompletedItemForMissingOutput(item);
    }

    const shouldRestoreCompleted =
      item.status === 'pending' && item.errorCode === 'OUTPUT_FILE_MISSING';
    const filepathChanged = item.completedFilepath !== state.filepath;
    if (!shouldRestoreCompleted && !filepathChanged) return item;

    changed = true;
    return {
      ...item,
      status: shouldRestoreCompleted ? 'completed' : item.status,
      progress: shouldRestoreCompleted ? 100 : item.progress,
      speed: shouldRestoreCompleted ? '' : item.speed,
      eta: shouldRestoreCompleted ? '' : item.eta,
      error: shouldRestoreCompleted ? undefined : item.error,
      errorCode: shouldRestoreCompleted ? undefined : item.errorCode,
      completedFilepath: state.filepath,
    };
  });

  return changed ? nextItems : items;
}

export function serializeDownloadQueueItems(items: DownloadItem[]): string {
  return JSON.stringify(normalizeDownloadQueueItems(items));
}

export async function loadPersistedDownloadQueue(
  queueKind: PersistedQueueKind,
): Promise<DownloadItem[]> {
  const itemsJson = await invoke<string | null>('load_download_queue', { queueKind });
  if (!itemsJson) return [];

  const parsed = JSON.parse(itemsJson) as unknown;
  if (!Array.isArray(parsed)) return [];

  return normalizeDownloadQueueItems(parsed as DownloadItem[]);
}

export async function savePersistedDownloadQueueJson(
  queueKind: PersistedQueueKind,
  itemsJson: string,
): Promise<void> {
  await invoke('save_download_queue', {
    queueKind,
    itemsJson,
  });
}

export async function clearPersistedDownloadQueue(queueKind: PersistedQueueKind): Promise<void> {
  await invoke('clear_download_queue', { queueKind });
}

export async function reconcileDownloadQueueFileStates(
  items: DownloadItem[],
): Promise<DownloadItem[]> {
  const historyIds = Array.from(
    new Set(
      items
        .map((item) => item.completedHistoryId)
        .filter((historyId): historyId is string => Boolean(historyId)),
    ),
  );
  const legacyCompletedItems = items.filter(
    (item) => item.status === 'completed' && !item.completedHistoryId && item.completedFilepath,
  );

  const [historyStates, legacyChecks] = await Promise.all([
    historyIds.length > 0
      ? invoke<HistoryFileState[]>('get_history_file_states', { historyIds })
      : Promise.resolve([]),
    Promise.all(
      legacyCompletedItems.map(async (item) => {
        const filepath = item.completedFilepath as string;
        const fileExists = await invoke<boolean>('check_file_exists', { filepath });
        return fileExists ? null : filepath;
      }),
    ),
  ]);

  return reconcileQueueItemsWithHistoryStates(
    items,
    historyStates,
    new Set(legacyChecks.filter((filepath): filepath is string => Boolean(filepath))),
  );
}
