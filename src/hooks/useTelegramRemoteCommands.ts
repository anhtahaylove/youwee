import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { type MutableRefObject, useCallback, useEffect, useRef } from 'react';
import type { Page } from '@/components/layout';
import { useDownload } from '@/contexts/DownloadContext';
import { useUniversal } from '@/contexts/UniversalContext';
import { normalizeExternalVideoUrl, resolveExternalRouteTarget } from '@/lib/external-link';
import type { DownloadItem, ExternalEnqueueOptions, Quality } from '@/lib/types';
import { isSafeUrl } from '@/lib/utils';

interface TelegramDownloadCommandEvent {
  command: 'add' | 'download' | 'status' | 'queue' | 'run' | 'stop';
  url?: string | null;
  quality?: string | null;
  chatId: string;
  messageThreadId?: number | null;
}

interface TelegramTikTokLiveCommandEvent {
  command:
    | 'watchlist'
    | 'status'
    | 'add'
    | 'remove'
    | 'enable'
    | 'disable'
    | 'inspect'
    | 'record'
    | 'stop';
  target?: string | null;
  chatId: string;
  messageThreadId?: number | null;
}

type TikTokLiveWatchStatus =
  | 'offline'
  | 'checking'
  | 'online'
  | 'recording'
  | 'backoff'
  | 'recoverable'
  | 'error';

interface TikTokLiveWatchEntry {
  id: string;
  targetInput: string;
  targetUrl: string;
  username?: string | null;
  enabled: boolean;
  autoRecord: boolean;
  status: TikTokLiveWatchStatus;
  activeJobId?: string | null;
  lastError?: string | null;
  lastCheckedAt?: number | null;
  lastOnlineAt?: number | null;
  lastRecordingAt?: number | null;
  lastOutcome?: string | null;
  lastSegmentCount?: number | null;
  lastRefreshCount?: number | null;
  lastReconnectCount?: number | null;
}

interface TikTokLiveRecorderConfig {
  maxConcurrentRecordings: number;
  activeRecordings: number;
  hardLimit: number;
}

type StartLockRef = MutableRefObject<{
  youtube: boolean;
  universal: boolean;
}>;

type QueueSource = 'YouTube' | 'Universal';

interface QueueEntry {
  item: DownloadItem;
  source: QueueSource;
  index: number;
}

function summarizeItems(items: DownloadItem[]) {
  return items.reduce(
    (summary, item) => {
      if (item.status === 'pending') {
        summary.pending += 1;
      } else if (item.status === 'downloading' || item.status === 'fetching') {
        summary.downloading += 1;
      } else if (item.status === 'completed') {
        summary.completed += 1;
      } else if (item.status === 'error') {
        summary.error += 1;
      }
      return summary;
    },
    { pending: 0, downloading: 0, completed: 0, error: 0 },
  );
}

function truncateText(text: string, maxLength = 80) {
  const normalized = text.trim().replace(/\s+/g, ' ');
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength - 1)}…`;
}

function statusIcon(status: DownloadItem['status']) {
  if (status === 'pending') return '⏳';
  if (status === 'downloading' || status === 'fetching') return '⬇️';
  if (status === 'completed') return '✅';
  if (status === 'error') return '❌';
  return '•';
}

function formatStatusLabel(status: DownloadItem['status']) {
  if (status === 'fetching') return 'fetching';
  return status;
}

function formatQueueEntry(entry: QueueEntry, displayIndex: number) {
  const { item, source, index } = entry;
  const title = truncateText(item.title || item.url, 90);
  const progress =
    item.status === 'downloading' || item.status === 'fetching' ? ` · ${item.progress}%` : '';
  return [
    `${displayIndex}. ${statusIcon(item.status)} ${formatStatusLabel(item.status)}${progress}`,
    `${source} #${index + 1}`,
    title,
  ].join('\n');
}

function formatTimestamp(seconds?: number | null) {
  if (!seconds) return 'never';
  return new Date(seconds * 1000).toLocaleString();
}

function tiktokLiveStatusIcon(status: TikTokLiveWatchStatus) {
  if (status === 'recording') return '🔴';
  if (status === 'online') return '🟢';
  if (status === 'backoff' || status === 'recoverable') return '🟡';
  if (status === 'error') return '❌';
  if (status === 'checking') return '🔎';
  return '⚫';
}

function formatTikTokLiveTarget(entry: TikTokLiveWatchEntry) {
  return entry.username ? `@${entry.username}` : entry.targetInput || entry.targetUrl;
}

function formatTikTokLiveEntry(entry: TikTokLiveWatchEntry, index: number) {
  const details = [
    `${index + 1}. ${tiktokLiveStatusIcon(entry.status)} ${formatTikTokLiveTarget(entry)}`,
    `Status: ${entry.status}${entry.enabled ? '' : ' · disabled'}`,
    entry.lastOutcome ? `Last outcome: ${entry.lastOutcome}` : undefined,
    entry.lastSegmentCount ? `Segments: ${entry.lastSegmentCount}` : undefined,
    entry.lastReconnectCount ? `Reconnects: ${entry.lastReconnectCount}` : undefined,
  ].filter(Boolean);
  return details.join('\n');
}

function tiktokTargetMatches(entry: TikTokLiveWatchEntry, target: string) {
  const raw = target.trim();
  const normalized = raw.toLowerCase();
  const username = raw.replace(/^@/, '').toLowerCase();
  return (
    entry.id === raw ||
    entry.targetInput.toLowerCase() === normalized ||
    entry.targetUrl.toLowerCase() === normalized ||
    entry.username?.toLowerCase() === username ||
    entry.targetUrl.toLowerCase().includes(`/@${username}/live`)
  );
}

function buildTikTokLiveWatchlistReply(entries: TikTokLiveWatchEntry[]) {
  if (entries.length === 0) {
    return '📭 TikTok Live watchlist is empty.\nUse /tl_add @username to add one.';
  }

  return [
    `📺 TikTok Live watchlist (${entries.length})`,
    '',
    entries.slice(0, 8).map(formatTikTokLiveEntry).join('\n\n'),
  ].join('\n');
}

function buildTikTokLiveStatusReply(
  entries: TikTokLiveWatchEntry[],
  config: TikTokLiveRecorderConfig,
  target?: string | null,
) {
  if (target) {
    const entry = entries.find((item) => tiktokTargetMatches(item, target));
    if (!entry) return `TikTok Live target not found: ${target}`;
    return [
      `${tiktokLiveStatusIcon(entry.status)} ${formatTikTokLiveTarget(entry)}`,
      `Status: ${entry.status}`,
      `Enabled: ${entry.enabled ? 'yes' : 'no'}`,
      `Auto-record: ${entry.autoRecord ? 'yes' : 'no'}`,
      `Last checked: ${formatTimestamp(entry.lastCheckedAt)}`,
      `Last online: ${formatTimestamp(entry.lastOnlineAt)}`,
      `Last recording: ${formatTimestamp(entry.lastRecordingAt)}`,
      entry.lastOutcome ? `Last outcome: ${entry.lastOutcome}` : undefined,
      entry.lastError ? `Last error: ${entry.lastError}` : undefined,
    ]
      .filter(Boolean)
      .join('\n');
  }

  const recording = entries.filter((entry) => entry.status === 'recording').length;
  const online = entries.filter((entry) => entry.status === 'online').length;
  const enabled = entries.filter((entry) => entry.enabled).length;
  return [
    '📡 TikTok Live Recorder',
    `Active rooms: ${config.activeRecordings}/${config.maxConcurrentRecordings}`,
    `Configured hard limit: ${config.hardLimit}`,
    `Watchlist: ${entries.length} total · ${enabled} enabled`,
    `Online: ${online}`,
    `Recording: ${recording}`,
  ].join('\n');
}

function buildStatusReply(
  youtubeItems: DownloadItem[],
  universalItems: DownloadItem[],
  isDownloading: boolean,
) {
  const youtube = summarizeItems(youtubeItems);
  const universal = summarizeItems(universalItems);
  const total = {
    pending: youtube.pending + universal.pending,
    downloading: youtube.downloading + universal.downloading,
    completed: youtube.completed + universal.completed,
    error: youtube.error + universal.error,
  };
  const isActive = isDownloading || total.downloading > 0;
  const totalItems = total.pending + total.downloading + total.completed + total.error;

  return [
    `${isActive ? '⬇️' : '🟢'} Youwee ${isActive ? 'is downloading' : 'is idle'}`,
    '',
    '📊 Queue status',
    `⏳ Pending: ${total.pending}`,
    `⬇️ Downloading: ${total.downloading}`,
    `✅ Completed: ${total.completed}`,
    `❌ Error: ${total.error}`,
    '',
    `📦 Total: ${totalItems}`,
  ].join('\n');
}

function buildQueueReply(youtubeItems: DownloadItem[], universalItems: DownloadItem[]) {
  const entries: QueueEntry[] = [
    ...youtubeItems.map((item, index) => ({ item, source: 'YouTube' as const, index })),
    ...universalItems.map((item, index) => ({ item, source: 'Universal' as const, index })),
  ];

  if (entries.length === 0) {
    return ['📭 Queue is empty.', '', 'Send a link or use /add <url> to add one.'].join('\n');
  }

  const activeEntries = entries.filter((entry) => entry.item.status !== 'completed');
  const recentEntries = (activeEntries.length > 0 ? activeEntries : entries).slice(-5).reverse();

  return [
    '📋 Recent queue items',
    '',
    recentEntries.map((entry, index) => formatQueueEntry(entry, index + 1)).join('\n\n'),
  ].join('\n');
}

function hasStartableItems(items: DownloadItem[]) {
  return items.some((item) => item.status === 'pending' || item.status === 'error');
}

function parseTelegramQuality(token?: string | null): ExternalEnqueueOptions | null {
  const normalized = token?.trim().toLowerCase();
  if (!normalized) return {};

  if (normalized === 'audio' || normalized === 'mp3') {
    return {
      mediaType: 'audio',
      quality: 'audio',
    };
  }

  const allowedQualities = new Set<Quality>([
    'best',
    '8k',
    '4k',
    '2k',
    '1080',
    '720',
    '480',
    '360',
  ]);

  if (allowedQualities.has(normalized as Quality)) {
    return {
      mediaType: 'video',
      quality: normalized as Quality,
    };
  }

  return null;
}

export function useTelegramRemoteCommands(
  setCurrentPage: (page: Page) => void,
  startLockRef: StartLockRef,
) {
  const download = useDownload();
  const universal = useUniversal();
  const latestRef = useRef({ download, setCurrentPage, universal });
  latestRef.current = { download, setCurrentPage, universal };

  const sendTelegramReply = useCallback(
    async (chatId: string, messageThreadId: number | null | undefined, text: string) => {
      try {
        await invoke('send_telegram_reply', {
          chatId,
          messageThreadId: messageThreadId ?? null,
          text,
        });
      } catch (error) {
        console.error('Failed to send Telegram reply:', error);
      }
    },
    [],
  );

  const handleTelegramDownloadCommand = useCallback(
    async (payload: TelegramDownloadCommandEvent) => {
      const { download, setCurrentPage, universal } = latestRef.current;
      const reply = (text: string) =>
        sendTelegramReply(payload.chatId, payload.messageThreadId, text);

      if (payload.command === 'status') {
        await reply(
          buildStatusReply(
            download.items,
            universal.items,
            download.isDownloading || universal.isDownloading,
          ),
        );
        return;
      }

      if (payload.command === 'queue') {
        await reply(buildQueueReply(download.items, universal.items));
        return;
      }

      if (payload.command === 'run') {
        const isBusy =
          download.isDownloading ||
          universal.isDownloading ||
          startLockRef.current.youtube ||
          startLockRef.current.universal;

        if (isBusy) {
          await reply('Youwee is already downloading.');
          return;
        }

        const shouldStartYoutube = hasStartableItems(download.items);
        const shouldStartUniversal = hasStartableItems(universal.items);

        if (!shouldStartYoutube && !shouldStartUniversal) {
          await reply('No pending downloads in the queue.');
          return;
        }

        if (shouldStartYoutube) {
          setCurrentPage('youtube');
          startLockRef.current.youtube = true;
          void download.startDownload().finally(() => {
            startLockRef.current.youtube = false;
          });
        }

        if (shouldStartUniversal) {
          if (!shouldStartYoutube) {
            setCurrentPage('universal');
          }
          startLockRef.current.universal = true;
          void universal.startDownload().finally(() => {
            startLockRef.current.universal = false;
          });
        }

        await reply('Started pending downloads.');
        return;
      }

      if (payload.command === 'stop') {
        const wasDownloading = download.isDownloading || universal.isDownloading;
        if (download.isDownloading) {
          await download.stopDownload();
        }
        if (universal.isDownloading) {
          await universal.stopDownload();
        }
        startLockRef.current.youtube = false;
        startLockRef.current.universal = false;
        await reply(
          wasDownloading ? 'Stopped the current download.' : 'Youwee is not downloading.',
        );
        return;
      }

      if (!payload.url) {
        await reply('No valid URL found in that command.');
        return;
      }

      const normalizedUrl = normalizeExternalVideoUrl(payload.url.trim());
      if (!isSafeUrl(normalizedUrl)) {
        await reply('No valid URL found in that command.');
        return;
      }

      const enqueueOptions = parseTelegramQuality(payload.quality);
      if (!enqueueOptions) {
        await reply(
          'Unsupported quality. Use: best, 8k, 4k, 2k, 1080, 720, 480, 360, audio, or mp3.',
        );
        return;
      }

      const routeTarget = resolveExternalRouteTarget('auto', normalizedUrl);

      try {
        if (routeTarget === 'youtube') {
          setCurrentPage('youtube');
          const result = await download.enqueueExternalUrl(normalizedUrl, enqueueOptions);
          if (!result.added) {
            await reply('This URL is already in the Youwee queue.');
            return;
          }

          if (payload.command === 'download') {
            if (download.isDownloading || startLockRef.current.youtube) {
              await reply('Added to the queue. Youwee is already downloading.');
              return;
            }

            startLockRef.current.youtube = true;
            void download.startDownload().finally(() => {
              startLockRef.current.youtube = false;
            });
            await reply('Added to the queue and started download.');
            return;
          }

          await reply('Added to the Youwee queue.');
          return;
        }

        setCurrentPage('universal');
        const result = await universal.enqueueExternalUrl(normalizedUrl, enqueueOptions);
        if (!result.added) {
          await reply('This URL is already in the Youwee queue.');
          return;
        }

        if (payload.command === 'download') {
          if (universal.isDownloading || startLockRef.current.universal) {
            await reply('Added to the queue. Youwee is already downloading.');
            return;
          }

          startLockRef.current.universal = true;
          void universal.startDownload().finally(() => {
            startLockRef.current.universal = false;
          });
          await reply('Added to the queue and started download.');
          return;
        }

        await reply('Added to the Youwee queue.');
      } catch (error) {
        console.error('Failed to handle Telegram command:', error);
        await reply('Failed to add that URL to Youwee.');
      }
    },
    [sendTelegramReply, startLockRef],
  );

  const handleTelegramTikTokLiveCommand = useCallback(
    async (payload: TelegramTikTokLiveCommandEvent) => {
      const { setCurrentPage } = latestRef.current;
      const reply = (text: string) =>
        sendTelegramReply(payload.chatId, payload.messageThreadId, text);
      const listEntries = () => invoke<TikTokLiveWatchEntry[]>('list_tiktok_live_watchlist');
      const getConfig = () => invoke<TikTokLiveRecorderConfig>('get_tiktok_live_recorder_config');
      const findEntry = async (target?: string | null) => {
        const entries = await listEntries();
        return {
          entries,
          entry: target ? entries.find((item) => tiktokTargetMatches(item, target)) : undefined,
        };
      };
      const ensureEntry = async (target: string) => {
        const { entry } = await findEntry(target);
        if (entry) return entry;
        return invoke<TikTokLiveWatchEntry>('save_tiktok_live_watch_entry', {
          entry: {
            id: null,
            input: target,
            outputDir: '',
            preferredQuality: 'auto',
            preferredTransport: 'auto',
            durationSeconds: null,
            cookieMode: null,
            cookieBrowser: null,
            cookieBrowserProfile: null,
            cookieFilePath: null,
            pollIntervalSeconds: 60,
            recordMode: 'oncePerLive',
            cooldownSeconds: 3600,
            filenameTemplate: null,
          },
        });
      };

      try {
        setCurrentPage('tiktok-live');

        if (payload.command === 'watchlist') {
          await reply(buildTikTokLiveWatchlistReply(await listEntries()));
          return;
        }

        if (payload.command === 'status') {
          const [entries, config] = await Promise.all([listEntries(), getConfig()]);
          await reply(buildTikTokLiveStatusReply(entries, config, payload.target));
          return;
        }

        if (!payload.target?.trim()) {
          await reply(`Missing TikTok Live target. Example: /tl_${payload.command} @username`);
          return;
        }

        const target = payload.target.trim();

        if (payload.command === 'add') {
          const entry = await ensureEntry(target);
          await reply(`Added TikTok Live target: ${formatTikTokLiveTarget(entry)}`);
          return;
        }

        if (payload.command === 'remove') {
          const { entry } = await findEntry(target);
          if (!entry) {
            await reply(`TikTok Live target not found: ${target}`);
            return;
          }
          await invoke('delete_tiktok_live_watch_entry', { id: entry.id });
          await reply(`Removed TikTok Live target: ${formatTikTokLiveTarget(entry)}`);
          return;
        }

        if (payload.command === 'enable' || payload.command === 'disable') {
          const { entry } = await findEntry(target);
          if (!entry) {
            await reply(`TikTok Live target not found: ${target}`);
            return;
          }
          const enabled = payload.command === 'enable';
          await invoke('set_tiktok_live_watch_entry_enabled', { id: entry.id, enabled });
          await reply(
            `${enabled ? 'Enabled' : 'Disabled'} TikTok Live target: ${formatTikTokLiveTarget(
              entry,
            )}`,
          );
          return;
        }

        if (payload.command === 'inspect') {
          const entry = await ensureEntry(target);
          await invoke('inspect_tiktok_live_watch_entry', { id: entry.id });
          const updated = (await findEntry(entry.id)).entry ?? entry;
          await reply(
            `Checked ${formatTikTokLiveTarget(updated)}: ${updated.status}${
              updated.lastError ? ` (${updated.lastError})` : ''
            }`,
          );
          return;
        }

        if (payload.command === 'record') {
          const entry = await ensureEntry(target);
          await invoke('record_tiktok_live_watch_entry', { id: entry.id });
          await reply(`Started TikTok Live recording: ${formatTikTokLiveTarget(entry)}`);
          return;
        }

        if (payload.command === 'stop') {
          const { entry } = await findEntry(target);
          if (!entry) {
            await reply(`TikTok Live target not found: ${target}`);
            return;
          }
          if (!entry.activeJobId) {
            await reply(`No active TikTok Live recording for ${formatTikTokLiveTarget(entry)}.`);
            return;
          }
          await invoke('cancel_tiktok_live_recording', { jobId: entry.activeJobId });
          await reply(`Stopping TikTok Live recording: ${formatTikTokLiveTarget(entry)}`);
        }
      } catch (error) {
        console.error('Failed to handle Telegram TikTok Live command:', error);
        await reply('Failed to handle that TikTok Live command.');
      }
    },
    [sendTelegramReply],
  );

  useEffect(() => {
    let disposed = false;
    const unlistenPromise = listen<TelegramDownloadCommandEvent>(
      'telegram-download-command',
      (event) => {
        void handleTelegramDownloadCommand(event.payload);
      },
    );

    unlistenPromise.then((unlisten) => {
      if (disposed) {
        unlisten();
      }
    });

    return () => {
      disposed = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [handleTelegramDownloadCommand]);

  useEffect(() => {
    let disposed = false;
    const unlistenPromise = listen<TelegramTikTokLiveCommandEvent>(
      'telegram-tiktok-live-command',
      (event) => {
        void handleTelegramTikTokLiveCommand(event.payload);
      },
    );

    unlistenPromise.then((unlisten) => {
      if (disposed) {
        unlisten();
      }
    });

    return () => {
      disposed = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [handleTelegramTikTokLiveCommand]);
}
