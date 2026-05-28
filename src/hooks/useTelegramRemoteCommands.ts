import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { type MutableRefObject, useCallback, useEffect } from 'react';
import type { Page } from '@/components/layout';
import { useDownload } from '@/contexts/DownloadContext';
import { useUniversal } from '@/contexts/UniversalContext';
import { normalizeExternalVideoUrl, resolveExternalRouteTarget } from '@/lib/external-link';
import { isSafeUrl } from '@/lib/utils';

interface TelegramDownloadCommandEvent {
  command: 'add' | 'download';
  url: string;
  chatId: string;
}

type StartLockRef = MutableRefObject<{
  youtube: boolean;
  universal: boolean;
}>;

export function useTelegramRemoteCommands(
  setCurrentPage: (page: Page) => void,
  startLockRef: StartLockRef,
) {
  const download = useDownload();
  const universal = useUniversal();

  const sendTelegramReply = useCallback(async (chatId: string, text: string) => {
    try {
      await invoke('send_telegram_reply', { chatId, text });
    } catch (error) {
      console.error('Failed to send Telegram reply:', error);
    }
  }, []);

  const handleTelegramDownloadCommand = useCallback(
    async (payload: TelegramDownloadCommandEvent) => {
      const normalizedUrl = normalizeExternalVideoUrl(payload.url.trim());
      if (!isSafeUrl(normalizedUrl)) {
        await sendTelegramReply(payload.chatId, 'No valid URL found in that command.');
        return;
      }

      const routeTarget = resolveExternalRouteTarget('auto', normalizedUrl);

      try {
        if (routeTarget === 'youtube') {
          setCurrentPage('youtube');
          const result = await download.enqueueExternalUrl(normalizedUrl);
          if (!result.added) {
            await sendTelegramReply(payload.chatId, 'This URL is already in the Youwee queue.');
            return;
          }

          if (payload.command === 'download') {
            if (download.isDownloading || startLockRef.current.youtube) {
              await sendTelegramReply(
                payload.chatId,
                'Added to the queue. Youwee is already downloading.',
              );
              return;
            }

            startLockRef.current.youtube = true;
            try {
              await download.startDownload();
              await sendTelegramReply(payload.chatId, 'Added to the queue and started download.');
            } finally {
              startLockRef.current.youtube = false;
            }
            return;
          }

          await sendTelegramReply(payload.chatId, 'Added to the Youwee queue.');
          return;
        }

        setCurrentPage('universal');
        const result = await universal.enqueueExternalUrl(normalizedUrl);
        if (!result.added) {
          await sendTelegramReply(payload.chatId, 'This URL is already in the Youwee queue.');
          return;
        }

        if (payload.command === 'download') {
          if (universal.isDownloading || startLockRef.current.universal) {
            await sendTelegramReply(
              payload.chatId,
              'Added to the queue. Youwee is already downloading.',
            );
            return;
          }

          startLockRef.current.universal = true;
          try {
            await universal.startDownload();
            await sendTelegramReply(payload.chatId, 'Added to the queue and started download.');
          } finally {
            startLockRef.current.universal = false;
          }
          return;
        }

        await sendTelegramReply(payload.chatId, 'Added to the Youwee queue.');
      } catch (error) {
        console.error('Failed to handle Telegram command:', error);
        await sendTelegramReply(payload.chatId, 'Failed to add that URL to Youwee.');
      }
    },
    [
      download.enqueueExternalUrl,
      download.isDownloading,
      download.startDownload,
      sendTelegramReply,
      setCurrentPage,
      startLockRef,
      universal.enqueueExternalUrl,
      universal.isDownloading,
      universal.startDownload,
    ],
  );

  useEffect(() => {
    const unlisten = listen<TelegramDownloadCommandEvent>('telegram-download-command', (event) => {
      void handleTelegramDownloadCommand(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [handleTelegramDownloadCommand]);
}
