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

export function useYtdlpAutoUpdateToast({
  onOpenDependencies,
}: {
  onOpenDependencies?: () => void;
} = {}) {
  const { t } = useTranslation('settings');
  const toast = useToast();
  const {
    ytdlpSource,
    ytdlpChannel,
    ytdlpAllVersions,
    isLoading,
    isChannelLoading,
    isChannelDownloading,
    isAutoDownloadingYtdlp,
    checkChannelUpdate,
    downloadChannelBinary,
  } = useDependencies();
  const checkStartedRef = useRef(false);

  useEffect(() => {
    if (checkStartedRef.current) return;
    if (ytdlpSource === 'system') return;
    if (ytdlpChannel === 'bundled') return;
    if (isLoading || isChannelLoading || isChannelDownloading) return;
    if (isAutoDownloadingYtdlp) return;
    if (!isYtdlpAutoUpdateCheckDue(readYtdlpAutoUpdateLastChecked())) return;
    if (!ytdlpAllVersions) return;

    checkStartedRef.current = true;

    const runAutoCheck = async () => {
      const channel: YtdlpChannel = ytdlpChannel;
      let latestVersion: string | null = null;
      let updateAvailable = false;

      try {
        const updateInfo = await checkChannelUpdate(channel, { silent: true });
        latestVersion = updateInfo?.latest_version ?? null;
        updateAvailable = updateInfo?.update_available === true;

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
              startUpdate: () => void downloadChannelBinary(channel),
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
    downloadChannelBinary,
    isAutoDownloadingYtdlp,
    isChannelDownloading,
    isChannelLoading,
    isLoading,
    onOpenDependencies,
    t,
    toast,
    ytdlpAllVersions,
    ytdlpChannel,
    ytdlpSource,
  ]);
}
