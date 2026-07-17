import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import { useCallback, useEffect, useState } from 'react';

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  body?: string;
  bodyVi?: string;
  bodyZhCN?: string;
  date?: string;
}

export interface UpdateProgress {
  downloaded: number;
  total: number;
}

export type UpdateStatus =
  | 'initializing'
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'error'
  | 'external'
  | 'installed'
  | 'up-to-date';

export const PENDING_UPDATE_STORAGE_KEY = 'youwee:pending-update';
const INSTALLED_UPDATE_SHOWN_STORAGE_KEY = 'youwee:installed-update-shown';

export function restoreInstalledUpdate(raw: string | null, currentVersion: string) {
  if (!raw) return null;

  try {
    const update = JSON.parse(raw) as Partial<UpdateInfo>;
    if (
      update.version !== currentVersion ||
      typeof update.currentVersion !== 'string' ||
      update.currentVersion.length === 0
    ) {
      return null;
    }

    return update as UpdateInfo;
  } catch {
    return null;
  }
}

function toUpdateInfo(update: NonNullable<Awaited<ReturnType<typeof check>>>) {
  const raw = update.rawJson as Record<string, unknown>;
  return {
    version: update.version,
    currentVersion: update.currentVersion,
    body: update.body ?? undefined,
    bodyVi: (raw.notes_vi as string) || undefined,
    bodyZhCN: (raw['notes_zh-CN'] as string) || undefined,
    date: update.date ?? undefined,
  } satisfies UpdateInfo;
}

function readPendingUpdate() {
  try {
    return localStorage.getItem(PENDING_UPDATE_STORAGE_KEY);
  } catch {
    return null;
  }
}

function storePendingUpdate(update: UpdateInfo) {
  try {
    localStorage.setItem(PENDING_UPDATE_STORAGE_KEY, JSON.stringify(update));
  } catch {
    // Updating should still work when web storage is unavailable.
  }
}

function clearPendingUpdate() {
  try {
    localStorage.removeItem(PENDING_UPDATE_STORAGE_KEY);
  } catch {
    // Nothing else is required when web storage is unavailable.
  }
}

function readShownUpdateVersion() {
  try {
    return localStorage.getItem(INSTALLED_UPDATE_SHOWN_STORAGE_KEY);
  } catch {
    return null;
  }
}

function storeShownUpdateVersion(version: string) {
  try {
    localStorage.setItem(INSTALLED_UPDATE_SHOWN_STORAGE_KEY, version);
  } catch {
    // The dialog can still be dismissed when web storage is unavailable.
  }
}

async function loadCurrentReleaseInfo(currentVersion: string): Promise<UpdateInfo> {
  const fallback = { version: currentVersion, currentVersion: '' };

  try {
    const raw = await invoke<Record<string, unknown>>('get_current_release_metadata');
    if (raw.version !== currentVersion) return fallback;

    return {
      ...fallback,
      body: typeof raw.notes === 'string' ? raw.notes : undefined,
      bodyVi: typeof raw.notes_vi === 'string' ? raw.notes_vi : undefined,
      bodyZhCN: typeof raw['notes_zh-CN'] === 'string' ? raw['notes_zh-CN'] : undefined,
      date: typeof raw.pub_date === 'string' ? raw.pub_date : undefined,
    };
  } catch {
    return fallback;
  }
}

export const updaterRestartsAutomatically = (platform: string) => platform.includes('Win');

export function useAppUpdater() {
  const [status, setStatus] = useState<UpdateStatus>('initializing');
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    const restoreUpdateState = async () => {
      try {
        const currentVersion = await getVersion();
        if (!active) return;
        const raw = readPendingUpdate();
        const installedUpdate = restoreInstalledUpdate(raw, currentVersion);
        if (installedUpdate) {
          setUpdateInfo(installedUpdate);
          setStatus('installed');
          return;
        }

        if (raw) clearPendingUpdate();
        if (readShownUpdateVersion() === currentVersion) {
          setStatus('idle');
          return;
        }

        const launchedAfterUpdate = await invoke<boolean>('was_launched_after_update').catch(
          () => false,
        );
        if (!active) return;
        if (launchedAfterUpdate) {
          const releaseInfo = await loadCurrentReleaseInfo(currentVersion);
          if (!active) return;
          setUpdateInfo(releaseInfo);
          setStatus('installed');
        } else {
          setStatus('idle');
        }
      } catch {
        if (active) setStatus('idle');
      }
    };

    void restoreUpdateState();

    return () => {
      active = false;
    };
  }, []);

  const isExternalUpdateManaged = useCallback(async () => {
    try {
      return await invoke<boolean>('is_flatpak_environment');
    } catch {
      return false;
    }
  }, []);

  const checkForUpdate = useCallback(async () => {
    setStatus('checking');
    setError(null);

    try {
      if (await isExternalUpdateManaged()) {
        setUpdateInfo(null);
        setStatus('external');
        return false;
      }

      const update = await check();

      if (update) {
        setUpdateInfo(toUpdateInfo(update));
        setStatus('available');
        return true;
      } else {
        setStatus('up-to-date');
        return false;
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to check for updates';
      setError(message);
      setStatus('error');
      return false;
    }
  }, [isExternalUpdateManaged]);

  const downloadAndInstall = useCallback(async () => {
    setStatus('downloading');
    setProgress({ downloaded: 0, total: 0 });

    try {
      if (await isExternalUpdateManaged()) {
        setProgress(null);
        setUpdateInfo(null);
        setStatus('external');
        return;
      }

      const update = await check();

      if (!update) {
        setStatus('up-to-date');
        return;
      }

      const pendingUpdate = toUpdateInfo(update);
      setUpdateInfo(pendingUpdate);
      storePendingUpdate(pendingUpdate);

      let downloaded = 0;
      let contentLength = 0;

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength ?? 0;
            setProgress({ downloaded: 0, total: contentLength });
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            setProgress({ downloaded, total: contentLength });
            break;
          case 'Finished':
            setProgress({ downloaded: contentLength, total: contentLength });
            break;
        }
      });

      setStatus('ready');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to download update';
      setError(message);
      setStatus('error');
    }
  }, [isExternalUpdateManaged]);

  const restartApp = useCallback(async () => {
    await relaunch();
  }, []);

  const dismissUpdate = useCallback(() => {
    if (status === 'installed') {
      if (updateInfo) storeShownUpdateVersion(updateInfo.version);
      clearPendingUpdate();
    }
    setStatus('idle');
    setUpdateInfo(null);
  }, [status, updateInfo]);

  return {
    status,
    updateInfo,
    progress,
    error,
    checkForUpdate,
    downloadAndInstall,
    restartApp,
    dismissUpdate,
  };
}
