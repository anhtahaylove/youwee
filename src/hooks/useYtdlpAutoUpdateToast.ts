import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useToast } from '@/components/ui/toast';
import { useDependencies } from '@/contexts/DependenciesContext';
import type { YtdlpChannel } from '@/lib/types';
import {
  createYtdlpUpdateToastAction,
  isYtdlpAutoUpdateCheckDue,
  readYtdlpAutoUpdateLastChecked,
  readYtdlpAutoUpdateLastNotified,
  shouldNotifyYtdlpUpdate,
  writeYtdlpAutoUpdateLastChecked,
  writeYtdlpAutoUpdateLastNotified,
} from '@/lib/ytdlp-auto-update';

function normalizeVersion(version: string) {
  return version.trim().replace(/^v/i, '');
}

export function useYtdlpAutoUpdateToast({
  onOpenDependencies,
}: {
  onOpenDependencies?: () => void;
} = {}) {
  const { t } = useTranslation('settings');
  const toast = useToast();
  const {
    ytdlpSource,
    ytdlpInfo,
    ytdlpChannel,
    ytdlpAllVersions,
    isLoading,
    isChannelLoading,
    isUpdating,
    isChannelDownloading,
    isAutoDownloadingYtdlp,
    checkForUpdate,
    checkChannelUpdate,
    updateYtdlp,
    downloadChannelBinary,
  } = useDependencies();
  const checkStartedRef = useRef(false);

  useEffect(() => {
    if (checkStartedRef.current) return;
    if (ytdlpSource === 'system') return;
    if (!ytdlpInfo || isLoading || isChannelLoading || isUpdating || isChannelDownloading) return;
    if (isAutoDownloadingYtdlp) return;
    if (!isYtdlpAutoUpdateCheckDue(readYtdlpAutoUpdateLastChecked())) return;
    if (ytdlpChannel !== 'bundled' && !ytdlpAllVersions) return;

    checkStartedRef.current = true;

    const runAutoCheck = async () => {
      const channel: YtdlpChannel = ytdlpChannel;
      let latestVersion: string | null = null;
      let updateAvailable = false;

      try {
        if (channel === 'bundled') {
          latestVersion = await checkForUpdate({ silent: true });
          updateAvailable = Boolean(
            latestVersion &&
              ytdlpInfo.version &&
              normalizeVersion(latestVersion) !== normalizeVersion(ytdlpInfo.version),
          );
        } else {
          const updateInfo = await checkChannelUpdate(channel, { silent: true });
          latestVersion = updateInfo?.latest_version ?? null;
          updateAvailable = updateInfo?.update_available === true;
        }

        const noticeKey = shouldNotifyYtdlpUpdate(
          updateAvailable,
          channel,
          latestVersion,
          readYtdlpAutoUpdateLastNotified(),
        );
        if (!noticeKey || !latestVersion) return;

        const toastId = `ytdlp-update-${channel}`;
        writeYtdlpAutoUpdateLastNotified(noticeKey);
        toast.info({
          id: toastId,
          title: t('dependencies.ytdlpUpdateToastTitle'),
          message: t('dependencies.ytdlpUpdateToastMessage', { version: latestVersion }),
          durationMs: 9000,
          action: {
            label: t('dependencies.update'),
            onClick: createYtdlpUpdateToastAction({
              toastId,
              dismissToast: toast.dismiss,
              openDependencies: onOpenDependencies ?? (() => {}),
              startUpdate: () => {
                if (channel === 'bundled') {
                  void updateYtdlp();
                } else {
                  void downloadChannelBinary(channel);
                }
              },
            }),
          },
        });
      } finally {
        writeYtdlpAutoUpdateLastChecked();
      }
    };

    void runAutoCheck();
  }, [
    checkChannelUpdate,
    checkForUpdate,
    downloadChannelBinary,
    isAutoDownloadingYtdlp,
    isChannelDownloading,
    isChannelLoading,
    isLoading,
    isUpdating,
    onOpenDependencies,
    t,
    toast,
    updateYtdlp,
    ytdlpAllVersions,
    ytdlpChannel,
    ytdlpInfo,
    ytdlpSource,
  ]);
}
