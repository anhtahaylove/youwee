import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { downloadDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';
import {
  Activity,
  BookmarkPlus,
  CheckCircle2,
  ChevronDown,
  ClipboardPaste,
  Clock3,
  Eye,
  FileCheck2,
  Folder,
  HardDrive,
  Layers3,
  Link2,
  ListVideo,
  Loader2,
  Pencil,
  Play,
  Radio,
  RefreshCw,
  Search,
  Settings2,
  Square,
  Trash2,
  Wifi,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ThemePicker } from '@/components/settings/ThemePicker';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { cacheRemoteThumbnailUrl } from '@/lib/asset-access';
import { extractBackendError, localizeBackendError } from '@/lib/backend-error';
import { buildCookieProxyInvokeOptions, loadNetworkSettings } from '@/lib/network-config';
import { openFileLocation } from '@/lib/open-file-location';
import { extractUrls } from '@/lib/sources';
import { cn } from '@/lib/utils';

type TikTokLiveVariant = {
  formatId: string;
  ext?: string;
  protocol?: string;
  quality?: string;
  resolution?: string;
  width?: number;
  height?: number;
  fps?: number;
  vcodec?: string;
  acodec?: string;
  tbr?: number;
  note?: string;
};

type TikTokLiveInspectResult = {
  input: string;
  targetUrl: string;
  sessionId?: string;
  title: string;
  uploader?: string;
  thumbnail?: string;
  avatar?: string;
  viewerCount?: number;
  isLive?: boolean;
  liveStatus?: string;
  variants: TikTokLiveVariant[];
  selectedVariant?: TikTokLiveVariant;
};

type TikTokLiveRecordResult = {
  jobId: string;
  historyId: string;
  filepath: string;
  title: string;
  filesize?: number;
  partial: boolean;
};

type TikTokLiveStatusEvent = {
  jobId: string;
  state: 'metadata-retry' | 'recording' | 'refreshing-stream' | 'merging-segments';
  attempt?: number;
  total?: number;
  autoReconnect?: boolean;
};

type TikTokLiveRecoveryJob = {
  id: string;
  target: string;
  title: string;
  outputDir: string;
  status: 'interrupted' | 'recoverable' | 'failed';
  segmentCount: number;
  hasMedia: boolean;
  refreshCount: number;
  reconnectCount: number;
  startedAt: number;
  updatedAt: number;
  errorMessage?: string;
};

type TikTokLiveWatchStatus =
  | 'offline'
  | 'checking'
  | 'online'
  | 'recording'
  | 'backoff'
  | 'recoverable'
  | 'error';

type TikTokLiveRecordMode = 'oncePerLive' | 'alwaysAfterCooldown' | 'manualOnly';

type TikTokLiveWatchEntry = {
  id: string;
  targetInput: string;
  targetUrl: string;
  username?: string;
  enabled: boolean;
  autoRecord: boolean;
  outputDir: string;
  preferredQuality?: string;
  preferredTransport?: string;
  durationSeconds?: number;
  cookieMode?: string;
  cookieBrowser?: string;
  cookieBrowserProfile?: string;
  cookieFilePath?: string;
  pollIntervalSeconds: number;
  recordMode: TikTokLiveRecordMode;
  cooldownSeconds: number;
  filenameTemplate?: string;
  scheduleEnabled: boolean;
  scheduleDays?: string;
  scheduleStartMinute?: number;
  scheduleEndMinute?: number;
  backoffAttempt: number;
  nextCheckAt: number;
  status: TikTokLiveWatchStatus;
  activeJobId?: string;
  lastError?: string;
  lastCheckedAt?: number;
  lastOnlineAt?: number;
  lastRecordingAt?: number;
  lastSessionId?: string;
  lastOutcome?: string;
  lastCompletedAt?: number;
  lastStartedJobId?: string;
  lastSegmentCount: number;
  lastRefreshCount: number;
  lastReconnectCount: number;
  lastFileSize?: number;
  lastTitle?: string;
  lastUploader?: string;
  thumbnail?: string;
  avatar?: string;
  lastViewerCount?: number;
  createdAt: number;
  updatedAt: number;
};

type TikTokLiveRecorderConfig = {
  maxConcurrentRecordings: number;
  activeRecordings: number;
  hardLimit: number;
};

type TikTokLiveTelemetrySnapshot = {
  activeRecordings: number;
  maxConcurrentRecordings: number;
  watchedStreamers: number;
  enabledWatchers: number;
  recoverableJobs: number;
  totalSegments: number;
  totalRefreshes: number;
  totalReconnects: number;
  totalRecordedBytes: number;
  resourceWarning?: 'limitHigh' | 'multiRoomActive';
};

type TikTokLiveWatchAuthSnapshot = Pick<
  TikTokLiveWatchEntry,
  'cookieMode' | 'cookieBrowser' | 'cookieBrowserProfile' | 'cookieFilePath'
>;

const QUALITY_OPTIONS = ['auto', 'origin', 'uhd_60', 'uhd', 'hd_60', 'hd', 'sd', 'ld', 'ao'];
const TRANSPORT_OPTIONS = ['auto', 'hls', 'flv', 'lls'];
const RECORD_MODE_OPTIONS: TikTokLiveRecordMode[] = [
  'oncePerLive',
  'alwaysAfterCooldown',
  'manualOnly',
];
const CAPTURE_PRESETS = [
  { id: 'origin', quality: 'origin', transport: 'auto' },
  { id: 'prefer60', quality: 'uhd_60', transport: 'auto' },
  { id: 'audio', quality: 'ao', transport: 'auto' },
] as const;
const DURATION_PRESETS = [
  { value: '60', label: '1m' },
  { value: '300', label: '5m' },
  { value: '1800', label: '30m' },
  { value: '3600', label: '1h' },
] as const;
const POLL_INTERVAL_PRESETS = [
  { value: '30', label: '30s' },
  { value: '60', label: '1m' },
  { value: '120', label: '2m' },
  { value: '300', label: '5m' },
  { value: '600', label: '10m' },
] as const;
const COOLDOWN_PRESETS = [
  { value: '0', label: '0' },
  { value: '900', label: '15m' },
  { value: '3600', label: '1h' },
  { value: '21600', label: '6h' },
  { value: '86400', label: '24h' },
] as const;
const FILENAME_TEMPLATE_PRESETS = [
  { id: 'automatic', value: '' },
  { id: 'username', value: '{username}_{date}_{time}' },
  { id: 'title', value: '{title}_{date}_{time}' },
] as const;

function isAbsolutePath(path: string): boolean {
  return path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path);
}

async function resolveDefaultOutputPath(): Promise<string> {
  try {
    const path = await downloadDir();
    if (isAbsolutePath(path)) return path;
  } catch {}

  return '';
}

function formatSize(bytes?: number): string {
  if (!bytes) return '';
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function minuteToTime(minute?: number): string {
  if (!Number.isFinite(minute)) return '';
  const safeMinute = Math.max(0, Math.min(1439, minute || 0));
  const hour = Math.floor(safeMinute / 60)
    .toString()
    .padStart(2, '0');
  const value = (safeMinute % 60).toString().padStart(2, '0');
  return `${hour}:${value}`;
}

function timeToMinute(value: string): number | null {
  const [hour, minute] = value.split(':').map((part) => Number.parseInt(part, 10));
  if (!Number.isFinite(hour) || !Number.isFinite(minute)) return null;
  const total = hour * 60 + minute;
  return total >= 0 && total < 24 * 60 ? total : null;
}

function variantLabel(variant?: TikTokLiveVariant): string {
  if (!variant) return '';
  const fps =
    typeof variant.fps === 'number' && Number.isFinite(variant.fps)
      ? `${Number.isInteger(variant.fps) ? variant.fps : variant.fps.toFixed(2)} FPS`
      : undefined;
  return [
    variant.formatId,
    variant.resolution,
    fps,
    variant.vcodec,
    variant.protocol || variant.ext,
    variant.tbr ? `${Math.round(variant.tbr)} kbps` : undefined,
  ]
    .filter(Boolean)
    .join(' · ');
}

function watchStatusClass(status: TikTokLiveWatchStatus): string {
  if (status === 'online' || status === 'recording') {
    return 'bg-green-500/10 text-green-600 dark:text-green-400';
  }
  if (status === 'recoverable' || status === 'backoff') {
    return 'bg-amber-500/10 text-amber-600 dark:text-amber-400';
  }
  if (status === 'error') {
    return 'bg-red-500/10 text-red-500';
  }
  return 'bg-blue-500/10 text-blue-600 dark:text-blue-400';
}

function isCancellationError(error: unknown): boolean {
  return extractBackendError(error).code === 'DOWNLOAD_CANCELLED';
}

function minutesFromSeconds(seconds: number): string {
  const minutes = seconds / 60;
  return Number.isInteger(minutes) ? minutes.toString() : Number(minutes.toFixed(2)).toString();
}

function formatDuration(secondsValue: string): string {
  const seconds = Number.parseInt(secondsValue, 10);
  if (!Number.isFinite(seconds) || seconds <= 0) return '∞';
  if (seconds % 3600 === 0) return `${seconds / 3600}h`;
  if (seconds % 60 === 0) return `${seconds / 60}m`;
  return `${seconds}s`;
}

function formatViewerCount(value: number, locale: string): string {
  try {
    return new Intl.NumberFormat(locale).format(value);
  } catch {
    return value.toLocaleString();
  }
}

function TikTokLiveThumbnail({
  src,
  alt,
  className,
}: {
  src?: string;
  alt: string;
  className: string;
}) {
  const [thumbnail, setThumbnail] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setThumbnail(null);
    void cacheRemoteThumbnailUrl(src).then((url) => {
      if (active) setThumbnail(url);
    });
    return () => {
      active = false;
    };
  }, [src]);

  if (!thumbnail) {
    return (
      <div
        className={cn(
          'grid shrink-0 place-items-center border border-dashed border-border/60 bg-muted/20 text-muted-foreground',
          className,
        )}
      >
        <Radio className="h-5 w-5" />
      </div>
    );
  }

  return (
    <img
      src={thumbnail}
      alt={alt}
      className={cn('shrink-0 border border-border/60 object-cover', className)}
      onError={() => setThumbnail(null)}
    />
  );
}

export function TikTokLivePage() {
  const { t, i18n } = useTranslation(['pages', 'universal']);
  const [input, setInput] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [duration, setDuration] = useState('60');
  const [durationMode, setDurationMode] = useState('60');
  const [customDurationMinutes, setCustomDurationMinutes] = useState('15');
  const [quality, setQuality] = useState('origin');
  const [transport, setTransport] = useState('auto');
  const [autoReconnect, setAutoReconnect] = useState(true);
  const [workspaceView, setWorkspaceView] = useState<'record' | 'watchlist'>('record');
  const [inspectResult, setInspectResult] = useState<TikTokLiveInspectResult | null>(null);
  const [recordResult, setRecordResult] = useState<TikTokLiveRecordResult | null>(null);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [isInspecting, setIsInspecting] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [isCancelling, setIsCancelling] = useState(false);
  const [recoveryJobs, setRecoveryJobs] = useState<TikTokLiveRecoveryJob[]>([]);
  const [recoveryActionId, setRecoveryActionId] = useState<string | null>(null);
  const [deleteCandidate, setDeleteCandidate] = useState<TikTokLiveRecoveryJob | null>(null);
  const [watchEntries, setWatchEntries] = useState<TikTokLiveWatchEntry[]>([]);
  const [watchPollInterval, setWatchPollInterval] = useState('60');
  const [watchPollIntervalMode, setWatchPollIntervalMode] = useState('60');
  const [watchRecordMode, setWatchRecordMode] = useState<TikTokLiveRecordMode>('oncePerLive');
  const [watchCooldown, setWatchCooldown] = useState('3600');
  const [watchCooldownMode, setWatchCooldownMode] = useState('3600');
  const [watchFilenameTemplate, setWatchFilenameTemplate] = useState('');
  const [watchFilenameTemplateMode, setWatchFilenameTemplateMode] = useState('automatic');
  const [watchScheduleEnabled, setWatchScheduleEnabled] = useState(false);
  const [watchScheduleDays, setWatchScheduleDays] = useState('');
  const [watchScheduleStart, setWatchScheduleStart] = useState('00:00');
  const [watchScheduleEnd, setWatchScheduleEnd] = useState('23:59');
  const [recorderConfig, setRecorderConfig] = useState<TikTokLiveRecorderConfig | null>(null);
  const [telemetrySnapshot, setTelemetrySnapshot] = useState<TikTokLiveTelemetrySnapshot | null>(
    null,
  );
  const [maxConcurrentRecordings, setMaxConcurrentRecordings] = useState('1');
  const [watchActionId, setWatchActionId] = useState<string | null>(null);
  const [editingWatchId, setEditingWatchId] = useState<string | null>(null);
  const [editingWatchAuth, setEditingWatchAuth] = useState<TikTokLiveWatchAuthSnapshot | null>(
    null,
  );
  const [watchDeleteCandidate, setWatchDeleteCandidate] = useState<TikTokLiveWatchEntry | null>(
    null,
  );
  const activeInspectJobIdRef = useRef<string | null>(null);
  const activeJobIdRef = useRef<string | null>(null);
  const watchRulesRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    void resolveDefaultOutputPath().then(setOutputDir);
  }, []);

  useEffect(() => {
    const unlistenPromise = listen<TikTokLiveStatusEvent>('tiktok-live-status', ({ payload }) => {
      if (
        payload.jobId !== activeInspectJobIdRef.current &&
        payload.jobId !== activeJobIdRef.current
      ) {
        return;
      }

      if (payload.state === 'metadata-retry') {
        setStatus(
          t('tiktokLive.status.retryingMetadata', {
            attempt: payload.attempt,
            total: payload.total,
          }),
        );
      } else if (payload.state === 'refreshing-stream') {
        setStatus(
          t('tiktokLive.status.refreshingStream', {
            attempt: payload.attempt,
            total: payload.total,
          }),
        );
      } else if (payload.state === 'merging-segments') {
        setStatus(t('tiktokLive.status.mergingSegments'));
      } else if (payload.state === 'recording') {
        setStatus(
          t(
            payload.autoReconnect
              ? 'tiktokLive.status.recordingReconnect'
              : 'tiktokLive.status.recording',
          ),
        );
      }
    });

    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [t]);

  const invokeOptions = useMemo(() => {
    const { cookieSettings, proxySettings } = loadNetworkSettings();
    return buildCookieProxyInvokeOptions(cookieSettings, proxySettings);
  }, []);

  const refreshRecoveryJobs = useCallback(async () => {
    try {
      setRecoveryJobs(await invoke<TikTokLiveRecoveryJob[]>('list_tiktok_live_recovery_jobs'));
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
    }
  }, []);

  const refreshWatchlist = useCallback(async () => {
    try {
      setWatchEntries(await invoke<TikTokLiveWatchEntry[]>('list_tiktok_live_watchlist'));
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
    }
  }, []);

  const refreshRecorderConfig = useCallback(async () => {
    try {
      const config = await invoke<TikTokLiveRecorderConfig>('get_tiktok_live_recorder_config');
      setRecorderConfig(config);
      setMaxConcurrentRecordings(config.maxConcurrentRecordings.toString());
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
    }
  }, []);

  const refreshTelemetry = useCallback(async () => {
    try {
      setTelemetrySnapshot(await invoke<TikTokLiveTelemetrySnapshot>('get_tiktok_live_telemetry'));
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
    }
  }, []);

  useEffect(() => {
    void refreshRecoveryJobs();
    void refreshWatchlist();
    void refreshRecorderConfig();
    void refreshTelemetry();
  }, [refreshRecoveryJobs, refreshRecorderConfig, refreshTelemetry, refreshWatchlist]);

  useEffect(() => {
    const unlistenPromise = listen('tiktok-live-watchlist-updated', () => {
      void refreshWatchlist();
      void refreshRecoveryJobs();
      void refreshRecorderConfig();
      void refreshTelemetry();
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshRecorderConfig, refreshRecoveryJobs, refreshTelemetry, refreshWatchlist]);

  const updateInput = useCallback((value: string) => {
    setInput(value);
    setInspectResult(null);
    setRecordResult(null);
    setStatus('');
    setError('');
  }, []);

  const pasteTarget = useCallback(async () => {
    try {
      const clipboardText = (await navigator.clipboard.readText()).trim();
      if (!clipboardText) return;

      const target =
        extractUrls(clipboardText).find((url) =>
          /(^|\.)tiktok\.com$/i.test(new URL(url).hostname),
        ) || clipboardText;
      updateInput(target);
    } catch {
      // Clipboard permission can be denied by the webview. Keep the input usable manually.
    }
  }, [updateInput]);

  const applyCapturePreset = useCallback((presetId: string) => {
    const preset = CAPTURE_PRESETS.find((candidate) => candidate.id === presetId);
    if (!preset) return;

    setQuality(preset.quality);
    setTransport(preset.transport);
    setAutoReconnect(true);
    setInspectResult(null);
    setStatus('');
  }, []);

  const selectOutputFolder = useCallback(async () => {
    const folder = await open({
      directory: true,
      multiple: false,
      title: t('tiktokLive.output.select'),
    });
    if (typeof folder === 'string') {
      setOutputDir(folder);
    }
  }, [t]);

  const inspectLive = useCallback(async () => {
    if (!input.trim()) return;
    const jobId = crypto.randomUUID();
    activeInspectJobIdRef.current = jobId;
    setIsInspecting(true);
    setError('');
    setRecordResult(null);
    setStatus(t('tiktokLive.status.inspecting'));
    try {
      const result = await invoke<TikTokLiveInspectResult>('inspect_tiktok_live', {
        jobId,
        input: input.trim(),
        preferredQuality: quality,
        preferredTransport: transport,
        ...invokeOptions,
      });
      setInspectResult(result);
      setStatus(
        result.isLive === false
          ? t('tiktokLive.status.notLive')
          : t('tiktokLive.status.inspectReady', { count: result.variants.length }),
      );
    } catch (err) {
      const backendError = extractBackendError(err);
      setInspectResult(null);
      if (backendError.code === 'TIKTOK_LIVE_OFFLINE') {
        setStatus(t('tiktokLive.status.notLive'));
      } else {
        setError(localizeBackendError(backendError));
        setStatus(t('tiktokLive.status.failed'));
      }
    } finally {
      activeInspectJobIdRef.current = null;
      setIsInspecting(false);
    }
  }, [input, invokeOptions, quality, t, transport]);

  const startRecording = useCallback(async () => {
    if (!input.trim()) return;
    const jobId = crypto.randomUUID();
    activeJobIdRef.current = jobId;
    setIsRecording(true);
    setIsCancelling(false);
    setError('');
    setRecordResult(null);
    setStatus(t('tiktokLive.status.preparing'));
    try {
      const seconds = Number.parseInt(duration, 10);
      const result = await invoke<TikTokLiveRecordResult>('record_tiktok_live', {
        jobId,
        input: input.trim(),
        outputDir,
        durationSeconds: Number.isFinite(seconds) && seconds > 0 ? seconds : null,
        preferredQuality: quality,
        preferredTransport: transport,
        autoReconnect,
        ...invokeOptions,
      });
      setRecordResult(result);
      setStatus(
        t(result.partial ? 'tiktokLive.status.partialSaved' : 'tiktokLive.status.recorded'),
      );
    } catch (err) {
      if (isCancellationError(err)) {
        setStatus(t('tiktokLive.status.cancelled'));
        return;
      }
      const backendError = extractBackendError(err);
      if (backendError.code === 'TIKTOK_LIVE_OFFLINE') {
        setStatus(t('tiktokLive.status.notLive'));
      } else {
        setError(localizeBackendError(backendError));
        setStatus(t('tiktokLive.status.failed'));
      }
    } finally {
      activeJobIdRef.current = null;
      setIsRecording(false);
      setIsCancelling(false);
      void refreshRecoveryJobs();
    }
  }, [
    autoReconnect,
    duration,
    input,
    invokeOptions,
    outputDir,
    quality,
    refreshRecoveryJobs,
    t,
    transport,
  ]);

  const cancelRecording = useCallback(async () => {
    const jobId = activeJobIdRef.current;
    if (!jobId) return;
    setIsCancelling(true);
    setError('');
    setStatus(t('tiktokLive.status.cancelling'));
    try {
      await invoke('cancel_tiktok_live_recording', { jobId });
    } catch (err) {
      setIsCancelling(false);
      setError(localizeBackendError(extractBackendError(err)));
      setStatus(t('tiktokLive.status.failed'));
    }
  }, [t]);

  const finalizeRecovery = useCallback(
    async (job: TikTokLiveRecoveryJob) => {
      setRecoveryActionId(job.id);
      setError('');
      setStatus(t('tiktokLive.recovery.finalizing'));
      try {
        const result = await invoke<TikTokLiveRecordResult>('finalize_tiktok_live_recovery', {
          jobId: job.id,
        });
        setRecordResult(result);
        setStatus(t('tiktokLive.recovery.finalized'));
      } catch (err) {
        setError(localizeBackendError(extractBackendError(err)));
        setStatus(t('tiktokLive.status.failed'));
      } finally {
        setRecoveryActionId(null);
        void refreshRecoveryJobs();
      }
    },
    [refreshRecoveryJobs, t],
  );

  const continueRecovery = useCallback(
    async (job: TikTokLiveRecoveryJob) => {
      activeJobIdRef.current = job.id;
      setRecoveryActionId(job.id);
      setIsRecording(true);
      setError('');
      setRecordResult(null);
      setStatus(t('tiktokLive.recovery.continuing'));
      try {
        const result = await invoke<TikTokLiveRecordResult>('continue_tiktok_live_recovery', {
          jobId: job.id,
          cookieSkipPatterns: invokeOptions.cookieSkipPatterns,
          proxyUrl: invokeOptions.proxyUrl,
        });
        setRecordResult(result);
        setStatus(
          t(result.partial ? 'tiktokLive.status.partialSaved' : 'tiktokLive.status.recorded'),
        );
      } catch (err) {
        if (isCancellationError(err)) {
          setStatus(t('tiktokLive.status.cancelled'));
        } else {
          setError(localizeBackendError(extractBackendError(err)));
          setStatus(t('tiktokLive.status.failed'));
        }
      } finally {
        activeJobIdRef.current = null;
        setRecoveryActionId(null);
        setIsRecording(false);
        setIsCancelling(false);
        void refreshRecoveryJobs();
      }
    },
    [invokeOptions.cookieSkipPatterns, invokeOptions.proxyUrl, refreshRecoveryJobs, t],
  );

  const deleteRecovery = useCallback(async () => {
    if (!deleteCandidate) return;
    const jobId = deleteCandidate.id;
    setRecoveryActionId(jobId);
    setDeleteCandidate(null);
    setError('');
    try {
      await invoke('delete_tiktok_live_recovery', { jobId });
      setStatus(t('tiktokLive.recovery.deleted'));
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
      setStatus(t('tiktokLive.status.failed'));
    } finally {
      setRecoveryActionId(null);
      void refreshRecoveryJobs();
    }
  }, [deleteCandidate, refreshRecoveryJobs, t]);

  const updateRecorderLimit = useCallback(
    async (value = maxConcurrentRecordings) => {
      const limitValue = Number.parseInt(value, 10);
      setWatchActionId('recorder-limit');
      setError('');
      try {
        const config = await invoke<TikTokLiveRecorderConfig>('set_tiktok_live_recorder_config', {
          maxConcurrentRecordings: Number.isFinite(limitValue) && limitValue > 0 ? limitValue : 1,
        });
        setRecorderConfig(config);
        setMaxConcurrentRecordings(config.maxConcurrentRecordings.toString());
        await refreshTelemetry();
        setStatus(t('tiktokLive.watchlist.recorderLimitUpdated'));
      } catch (err) {
        setError(localizeBackendError(extractBackendError(err)));
      } finally {
        setWatchActionId(null);
      }
    },
    [maxConcurrentRecordings, refreshTelemetry, t],
  );

  const saveWatchEntry = useCallback(async () => {
    if (!input.trim() || !outputDir) return;
    const actionId = editingWatchId || 'new';
    setWatchActionId(actionId);
    setError('');
    try {
      const durationValue = Number.parseInt(duration, 10);
      const pollIntervalValue = Number.parseInt(watchPollInterval, 10);
      const cooldownValue = Number.parseInt(watchCooldown, 10);
      const watchAuth = editingWatchAuth ?? invokeOptions;
      await invoke<TikTokLiveWatchEntry>('save_tiktok_live_watch_entry', {
        entry: {
          id: editingWatchId,
          input: input.trim(),
          outputDir,
          preferredQuality: quality,
          preferredTransport: transport,
          durationSeconds:
            Number.isFinite(durationValue) && durationValue > 0 ? durationValue : null,
          cookieMode: watchAuth.cookieMode,
          cookieBrowser: watchAuth.cookieBrowser,
          cookieBrowserProfile: watchAuth.cookieBrowserProfile,
          cookieFilePath: watchAuth.cookieFilePath,
          pollIntervalSeconds:
            Number.isFinite(pollIntervalValue) && pollIntervalValue > 0 ? pollIntervalValue : 60,
          recordMode: watchRecordMode,
          cooldownSeconds:
            Number.isFinite(cooldownValue) && cooldownValue >= 0 ? cooldownValue : 3600,
          filenameTemplate: watchFilenameTemplate.trim() || null,
          scheduleEnabled: watchScheduleEnabled,
          scheduleDays: watchScheduleDays.trim() || null,
          scheduleStartMinute: watchScheduleEnabled ? timeToMinute(watchScheduleStart) : null,
          scheduleEndMinute: watchScheduleEnabled ? timeToMinute(watchScheduleEnd) : null,
          title: inspectResult?.title,
          uploader: inspectResult?.uploader,
          thumbnail: inspectResult?.thumbnail,
          avatar: inspectResult?.avatar,
          viewerCount: inspectResult?.viewerCount,
        },
      });
      setStatus(t(editingWatchId ? 'tiktokLive.watchlist.updated' : 'tiktokLive.watchlist.added'));
      setEditingWatchId(null);
      setEditingWatchAuth(null);
      await refreshWatchlist();
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
      setStatus(t('tiktokLive.status.failed'));
    } finally {
      setWatchActionId(null);
    }
  }, [
    duration,
    editingWatchAuth,
    editingWatchId,
    input,
    inspectResult,
    invokeOptions,
    outputDir,
    quality,
    refreshWatchlist,
    t,
    transport,
    watchCooldown,
    watchFilenameTemplate,
    watchPollInterval,
    watchRecordMode,
    watchScheduleDays,
    watchScheduleEnabled,
    watchScheduleEnd,
    watchScheduleStart,
  ]);

  const editWatchEntry = useCallback((entry: TikTokLiveWatchEntry) => {
    setWorkspaceView('watchlist');
    setEditingWatchId(entry.id);
    setInput(entry.targetInput);
    setOutputDir(entry.outputDir);
    const durationSeconds = entry.durationSeconds ?? 0;
    const durationValue = durationSeconds.toString();
    const savedDurationPreset = DURATION_PRESETS.some((preset) => preset.value === durationValue)
      ? durationValue
      : durationSeconds === 0
        ? '0'
        : 'custom';
    setDuration(durationValue);
    setDurationMode(savedDurationPreset);
    if (savedDurationPreset === 'custom') {
      setCustomDurationMinutes(minutesFromSeconds(durationSeconds));
    }
    setQuality(entry.preferredQuality === 'auto' ? 'origin' : entry.preferredQuality || 'origin');
    setTransport(entry.preferredTransport || 'auto');
    const pollIntervalValue = entry.pollIntervalSeconds.toString();
    setWatchPollInterval(pollIntervalValue);
    setWatchPollIntervalMode(
      POLL_INTERVAL_PRESETS.some((preset) => preset.value === pollIntervalValue)
        ? pollIntervalValue
        : 'custom',
    );
    setWatchRecordMode(entry.recordMode || 'oncePerLive');
    const cooldownValue = (entry.cooldownSeconds ?? 3600).toString();
    setWatchCooldown(cooldownValue);
    setWatchCooldownMode(
      COOLDOWN_PRESETS.some((preset) => preset.value === cooldownValue) ? cooldownValue : 'custom',
    );
    const filenameTemplateValue = entry.filenameTemplate || '';
    setWatchFilenameTemplate(filenameTemplateValue);
    setWatchFilenameTemplateMode(
      FILENAME_TEMPLATE_PRESETS.find((preset) => preset.value === filenameTemplateValue)?.id ??
        'custom',
    );
    setWatchScheduleEnabled(entry.scheduleEnabled);
    setWatchScheduleDays(entry.scheduleDays || '');
    setWatchScheduleStart(minuteToTime(entry.scheduleStartMinute) || '00:00');
    setWatchScheduleEnd(minuteToTime(entry.scheduleEndMinute) || '23:59');
    setEditingWatchAuth({
      cookieMode: entry.cookieMode,
      cookieBrowser: entry.cookieBrowser,
      cookieBrowserProfile: entry.cookieBrowserProfile,
      cookieFilePath: entry.cookieFilePath,
    });
    setInspectResult(null);
    setRecordResult(null);
    setError('');
    setStatus('');
    requestAnimationFrame(() => {
      watchRulesRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
  }, []);

  const toggleWatchEntry = useCallback(
    async (entry: TikTokLiveWatchEntry, enabled: boolean) => {
      setWatchActionId(entry.id);
      setError('');
      try {
        await invoke('set_tiktok_live_watch_entry_enabled', { id: entry.id, enabled });
        await refreshWatchlist();
      } catch (err) {
        setError(localizeBackendError(extractBackendError(err)));
      } finally {
        setWatchActionId(null);
      }
    },
    [refreshWatchlist],
  );

  const inspectWatchEntry = useCallback(
    async (entry: TikTokLiveWatchEntry) => {
      setWatchActionId(entry.id);
      setError('');
      try {
        await invoke('inspect_tiktok_live_watch_entry', { id: entry.id });
        setStatus(t('tiktokLive.watchlist.inspected'));
        await refreshWatchlist();
      } catch (err) {
        setError(localizeBackendError(extractBackendError(err)));
      } finally {
        setWatchActionId(null);
      }
    },
    [refreshWatchlist, t],
  );

  const recordWatchEntry = useCallback(
    async (entry: TikTokLiveWatchEntry) => {
      setWatchActionId(entry.id);
      setError('');
      try {
        await invoke('record_tiktok_live_watch_entry', { id: entry.id });
        setStatus(t('tiktokLive.watchlist.recordingStarted'));
        await refreshWatchlist();
      } catch (err) {
        setError(localizeBackendError(extractBackendError(err)));
      } finally {
        setWatchActionId(null);
      }
    },
    [refreshWatchlist, t],
  );

  const cancelWatchRecording = useCallback(
    async (entry: TikTokLiveWatchEntry) => {
      if (!entry.activeJobId) return;
      setWatchActionId(entry.id);
      setError('');
      try {
        await invoke('cancel_tiktok_live_recording', { jobId: entry.activeJobId });
        setStatus(t('tiktokLive.status.cancelling'));
      } catch (err) {
        setError(localizeBackendError(extractBackendError(err)));
      } finally {
        setWatchActionId(null);
      }
    },
    [t],
  );

  const deleteWatchEntry = useCallback(async () => {
    if (!watchDeleteCandidate) return;
    const id = watchDeleteCandidate.id;
    setWatchActionId(id);
    setWatchDeleteCandidate(null);
    setError('');
    try {
      await invoke('delete_tiktok_live_watch_entry', { id });
      if (editingWatchId === id) {
        setEditingWatchId(null);
        setEditingWatchAuth(null);
      }
      setStatus(t('tiktokLive.watchlist.deleted'));
      await refreshWatchlist();
    } catch (err) {
      setError(localizeBackendError(extractBackendError(err)));
    } finally {
      setWatchActionId(null);
    }
  }, [editingWatchId, refreshWatchlist, t, watchDeleteCandidate]);

  const busy = isInspecting || isRecording || recoveryActionId !== null;
  const parsedDuration = Number.parseInt(duration, 10);
  const hasValidDuration =
    duration === '0' || (Number.isFinite(parsedDuration) && parsedDuration > 0);
  const canInspect = input.trim().length > 0 && !busy;
  const canRecord = canInspect && outputDir.length > 0 && hasValidDuration;
  const canCancel = isRecording && !isCancelling;
  const watchBusy = watchActionId !== null;
  const canSaveWatch =
    input.trim().length > 0 && outputDir.length > 0 && hasValidDuration && !busy && !watchBusy;
  const localTelemetry = useMemo(() => {
    const watchStats = watchEntries.reduce(
      (totals, entry) => ({
        segments: totals.segments + entry.lastSegmentCount,
        refreshes: totals.refreshes + entry.lastRefreshCount,
        reconnects: totals.reconnects + entry.lastReconnectCount,
        fileSize: totals.fileSize + (entry.lastFileSize || 0),
      }),
      { segments: 0, refreshes: 0, reconnects: 0, fileSize: 0 },
    );
    return {
      active: recorderConfig?.activeRecordings ?? 0,
      watched: watchEntries.length,
      enabled: watchEntries.filter((entry) => entry.enabled).length,
      recoverable: recoveryJobs.length,
      ...watchStats,
      resourceWarning: (recorderConfig?.maxConcurrentRecordings ?? 1) > 1 ? 'limitHigh' : null,
    };
  }, [
    recorderConfig?.activeRecordings,
    recorderConfig?.maxConcurrentRecordings,
    recoveryJobs.length,
    watchEntries,
  ]);
  const telemetry = telemetrySnapshot
    ? {
        active: telemetrySnapshot.activeRecordings,
        watched: telemetrySnapshot.watchedStreamers,
        enabled: telemetrySnapshot.enabledWatchers,
        recoverable: telemetrySnapshot.recoverableJobs,
        segments: telemetrySnapshot.totalSegments,
        refreshes: telemetrySnapshot.totalRefreshes,
        reconnects: telemetrySnapshot.totalReconnects,
        fileSize: telemetrySnapshot.totalRecordedBytes,
        resourceWarning: telemetrySnapshot.resourceWarning || null,
      }
    : localTelemetry;
  const activeCapturePreset =
    CAPTURE_PRESETS.find(
      (preset) => preset.quality === quality && preset.transport === transport && autoReconnect,
    )?.id ?? 'custom';
  const captureSummary =
    activeCapturePreset === 'custom'
      ? `${quality.replace('_', ' · ').toUpperCase()} · ${transport.toUpperCase()}`
      : t(`tiktokLive.presets.${activeCapturePreset}`);
  const selectedScheduleDays = useMemo(() => {
    return new Set(
      watchScheduleDays
        .split(',')
        .map((value) => Number.parseInt(value.trim(), 10))
        .filter((value) => Number.isInteger(value) && value >= 0 && value <= 6),
    );
  }, [watchScheduleDays]);
  const scheduleLocale = i18n.resolvedLanguage || i18n.language || 'en';
  const scheduleDayLabels = useMemo(() => {
    const formatter = new Intl.DateTimeFormat(scheduleLocale, {
      weekday: 'short',
      timeZone: 'UTC',
    });
    return Array.from({ length: 7 }, (_, day) => ({
      day,
      label: formatter.format(new Date(Date.UTC(2023, 0, 2 + day))),
    }));
  }, [scheduleLocale]);
  const toggleScheduleDay = useCallback((day: number) => {
    setWatchScheduleDays((current) => {
      const days = new Set(
        current
          .split(',')
          .map((value) => Number.parseInt(value.trim(), 10))
          .filter((value) => Number.isInteger(value) && value >= 0 && value <= 6),
      );
      if (days.has(day)) {
        days.delete(day);
      } else {
        days.add(day);
      }
      return [...days].sort((left, right) => left - right).join(',');
    });
  }, []);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <header className="flex h-14 flex-shrink-0 items-center justify-between px-4 sm:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <div className="relative grid h-9 w-9 shrink-0 place-items-center rounded-xl border border-cyan-500/20 bg-cyan-500/10 text-cyan-500">
            <Radio className="h-5 w-5" />
            <span
              className={cn(
                'absolute right-0 top-0 h-2.5 w-2.5 rounded-full border-2 border-background',
                telemetry.active > 0 ? 'animate-pulse bg-rose-500' : 'bg-muted-foreground/40',
              )}
            />
          </div>
          <div className="min-w-0">
            <h1 className="truncate text-base font-semibold sm:text-lg">{t('tiktokLive.title')}</h1>
            <p className="hidden truncate text-xs text-muted-foreground sm:block">
              {t('tiktokLive.subtitle')}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Badge className="hidden gap-1.5 bg-rose-500/10 text-rose-600 sm:inline-flex dark:text-rose-400">
            <span
              className={cn(
                'h-1.5 w-1.5 rounded-full',
                telemetry.active > 0 ? 'animate-pulse bg-rose-500' : 'bg-current opacity-40',
              )}
            />
            {t('tiktokLive.telemetry.active', { count: telemetry.active })}
          </Badge>
          <ThemePicker />
        </div>
      </header>

      <div className="mx-4 sm:mx-6 h-px bg-gradient-to-r from-transparent via-border/50 to-transparent" />

      <main className="flex-1 space-y-4 overflow-y-auto px-4 py-5 sm:px-6">
        <section className="space-y-4 rounded-xl border border-border/60 bg-card/40 p-4">
          <form
            className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto_auto]"
            onSubmit={(event) => {
              event.preventDefault();
              void inspectLive();
            }}
          >
            <div className="relative">
              <Link2 className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                id="tiktok-live-target"
                value={input}
                onChange={(event) => updateInput(event.target.value)}
                disabled={busy}
                placeholder={t('tiktokLive.input.placeholder')}
                aria-label={t('tiktokLive.input.label')}
                className="h-11 pl-9 pr-10"
                autoComplete="off"
              />
              {input && !busy && (
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  onClick={() => updateInput('')}
                  className="absolute right-1 top-1/2 h-8 w-8 -translate-y-1/2 rounded-md text-muted-foreground"
                  aria-label={t('tiktokLive.input.clear')}
                >
                  <X className="h-4 w-4" />
                </Button>
              )}
            </div>
            <Button
              type="button"
              variant="outline"
              onClick={() => void pasteTarget()}
              disabled={busy}
              className="h-11 gap-2 border-dashed px-4"
              aria-label={t('urlInput.paste', { ns: 'universal' })}
              title={t('urlInput.paste', { ns: 'universal' })}
            >
              <ClipboardPaste className="h-4 w-4" />
              <span className="md:hidden lg:inline">
                {t('urlInput.paste', { ns: 'universal' })}
              </span>
            </Button>
            <Button type="submit" disabled={!canInspect} className="h-11 gap-2 px-5">
              {isInspecting ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Search className="h-4 w-4" />
              )}
              {t('tiktokLive.actions.inspect')}
            </Button>
          </form>

          <div className="space-y-2">
            <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
              <Settings2 className="h-3.5 w-3.5" />
              {t('tiktokLive.presets.quickSetup')}
            </p>
            <div className="grid gap-2 sm:grid-cols-3">
              {CAPTURE_PRESETS.map((preset) => {
                const active = activeCapturePreset === preset.id;
                const description = t(`tiktokLive.presets.${preset.id}Description`);
                return (
                  <Button
                    key={preset.id}
                    type="button"
                    variant="outline"
                    aria-pressed={active}
                    title={description}
                    onClick={() => applyCapturePreset(preset.id)}
                    disabled={busy}
                    className={cn(
                      'h-auto min-h-16 justify-start whitespace-normal border-dashed px-3 py-2 text-left transition-colors',
                      active &&
                        'border-cyan-500/50 bg-cyan-500/10 text-cyan-700 hover:bg-cyan-500/15 dark:text-cyan-300',
                    )}
                  >
                    <span className="flex flex-col gap-0.5">
                      <span className="text-sm font-medium">
                        {t(`tiktokLive.presets.${preset.id}`)}
                      </span>
                      <span className="text-[11px] font-normal leading-snug opacity-75">
                        {description}
                      </span>
                    </span>
                  </Button>
                );
              })}
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <div>
              <label
                htmlFor="tiktok-live-duration-preset"
                className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
              >
                <Clock3 className="h-3.5 w-3.5" />
                {t('tiktokLive.duration')}
              </label>
              <Select
                value={durationMode}
                onValueChange={(value) => {
                  setDurationMode(value);
                  if (value === 'custom') {
                    const minutes = Number.parseFloat(customDurationMinutes);
                    setDuration(
                      Number.isFinite(minutes) && minutes > 0
                        ? Math.round(minutes * 60).toString()
                        : '900',
                    );
                    return;
                  }
                  setDuration(value);
                }}
                disabled={busy}
              >
                <SelectTrigger id="tiktok-live-duration-preset" className="h-10">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {DURATION_PRESETS.map((preset) => (
                    <SelectItem key={preset.value} value={preset.value}>
                      {preset.label}
                    </SelectItem>
                  ))}
                  <SelectItem value="0">{t('tiktokLive.presets.unlimited')}</SelectItem>
                  <SelectItem value="custom">{t('tiktokLive.presets.custom')}</SelectItem>
                </SelectContent>
              </Select>
              {durationMode === 'custom' && (
                <div className="mt-2 space-y-1">
                  <label
                    htmlFor="tiktok-live-duration"
                    className="text-xs font-medium text-muted-foreground"
                  >
                    {t('tiktokLive.presets.customDurationMinutes')}
                  </label>
                  <Input
                    id="tiktok-live-duration"
                    type="number"
                    min="1"
                    step="1"
                    inputMode="numeric"
                    value={customDurationMinutes}
                    onChange={(event) => {
                      const value = event.target.value;
                      const minutes = Number.parseFloat(value);
                      setCustomDurationMinutes(value);
                      setDuration(
                        Number.isFinite(minutes) && minutes > 0
                          ? Math.round(minutes * 60).toString()
                          : '',
                      );
                    }}
                    disabled={busy}
                    className="h-10"
                    aria-describedby="tiktok-live-duration-help"
                  />
                  <p
                    id="tiktok-live-duration-help"
                    className="text-[11px] leading-snug text-muted-foreground"
                  >
                    {t('tiktokLive.presets.customDurationHelp')}
                  </p>
                </div>
              )}
            </div>
            <div>
              <p className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                <Folder className="h-3.5 w-3.5" />
                {t('tiktokLive.output.label')}
              </p>
              <Button
                variant="outline"
                className="h-10 w-full justify-start gap-2 px-3"
                onClick={() => void selectOutputFolder()}
                disabled={busy}
                title={outputDir || t('tiktokLive.output.empty')}
              >
                <Folder className="h-4 w-4 shrink-0" />
                <span className="truncate">{outputDir || t('tiktokLive.output.empty')}</span>
              </Button>
            </div>
          </div>

          <Collapsible className="rounded-lg border border-border/50 bg-muted/15">
            <CollapsibleTrigger asChild>
              <button
                type="button"
                className="flex w-full items-center justify-between gap-3 px-3 py-2.5 text-left text-sm font-medium"
              >
                <span className="flex items-center gap-2">
                  <Settings2 className="h-4 w-4 text-cyan-500" />
                  {t('tiktokLive.presets.advanced')}
                </span>
                <ChevronDown className="h-4 w-4 text-muted-foreground transition-transform [[data-state=open]_&]:rotate-180" />
              </button>
            </CollapsibleTrigger>
            <CollapsibleContent className="border-t border-border/50 px-3 py-3">
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <label
                    htmlFor="tiktok-live-quality"
                    className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
                  >
                    <Settings2 className="h-3.5 w-3.5" />
                    {t('tiktokLive.quality')}
                  </label>
                  <Select
                    value={quality}
                    onValueChange={(value) => {
                      setQuality(value);
                      setInspectResult(null);
                      setStatus('');
                    }}
                    disabled={busy}
                  >
                    <SelectTrigger id="tiktok-live-quality" className="h-10">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {QUALITY_OPTIONS.map((option) => (
                        <SelectItem key={option} value={option}>
                          {option.replace('_', ' · ').toUpperCase()}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div>
                  <label
                    htmlFor="tiktok-live-transport"
                    className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
                  >
                    <Wifi className="h-3.5 w-3.5" />
                    {t('tiktokLive.transport')}
                  </label>
                  <Select
                    value={transport}
                    onValueChange={(value) => {
                      setTransport(value);
                      setInspectResult(null);
                      setStatus('');
                    }}
                    disabled={busy}
                  >
                    <SelectTrigger id="tiktok-live-transport" className="h-10">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {TRANSPORT_OPTIONS.map((option) => (
                        <SelectItem key={option} value={option}>
                          {option.toUpperCase()}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>
              <div className="mt-3 flex items-start justify-between gap-4 rounded-lg bg-muted/25 px-3 py-2.5">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 text-sm font-medium">
                    <RefreshCw className="h-4 w-4 text-cyan-500" />
                    {t('tiktokLive.autoReconnect.label')}
                  </div>
                  <p className="mt-0.5 text-xs text-muted-foreground">
                    {t('tiktokLive.autoReconnect.description')}
                  </p>
                </div>
                <Switch
                  checked={autoReconnect}
                  onCheckedChange={setAutoReconnect}
                  disabled={busy}
                  aria-label={t('tiktokLive.autoReconnect.label')}
                />
              </div>
            </CollapsibleContent>
          </Collapsible>

          <Tabs
            value={workspaceView}
            onValueChange={(value) => setWorkspaceView(value as 'record' | 'watchlist')}
            className="space-y-3"
          >
            <TabsList className="grid h-11 w-full grid-cols-2 rounded-xl bg-muted/50 p-1 sm:max-w-md">
              <TabsTrigger value="record" className="h-9 gap-2 rounded-lg">
                <Radio className="h-4 w-4" />
                {t('tiktokLive.actions.record')}
              </TabsTrigger>
              <TabsTrigger value="watchlist" className="h-9 gap-2 rounded-lg">
                <ListVideo className="h-4 w-4" />
                {t('tiktokLive.watchlist.title')}
                {watchEntries.length > 0 && (
                  <span className="rounded-full bg-cyan-500/15 px-1.5 py-0.5 text-[10px] text-cyan-600 dark:text-cyan-400">
                    {watchEntries.length}
                  </span>
                )}
              </TabsTrigger>
            </TabsList>

            <TabsContent ref={watchRulesRef} value="watchlist" className="mt-0 space-y-3">
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <p className="mb-1 text-xs text-muted-foreground">
                    {t('tiktokLive.watchlist.pollInterval')}
                  </p>
                  <Select
                    value={watchPollIntervalMode}
                    onValueChange={(value) => {
                      setWatchPollIntervalMode(value);
                      if (value === 'custom') {
                        if (watchPollIntervalMode !== 'custom') setWatchPollInterval('90');
                        return;
                      }
                      setWatchPollInterval(value);
                    }}
                    disabled={busy || watchBusy}
                  >
                    <SelectTrigger aria-label={t('tiktokLive.watchlist.pollInterval')}>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {POLL_INTERVAL_PRESETS.map((preset) => (
                        <SelectItem key={preset.value} value={preset.value}>
                          {preset.label}
                        </SelectItem>
                      ))}
                      <SelectItem value="custom">{t('tiktokLive.presets.custom')}</SelectItem>
                    </SelectContent>
                  </Select>
                  {watchPollIntervalMode === 'custom' && (
                    <div className="relative mt-2">
                      <Input
                        type="number"
                        min="30"
                        max="3600"
                        step="1"
                        inputMode="numeric"
                        value={watchPollInterval}
                        onChange={(event) => setWatchPollInterval(event.target.value)}
                        disabled={busy || watchBusy}
                        className="pr-8"
                        aria-label={t('tiktokLive.watchlist.pollInterval')}
                      />
                      <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">
                        s
                      </span>
                    </div>
                  )}
                </div>
                <div>
                  <p className="mb-1 text-xs text-muted-foreground">
                    {t('tiktokLive.watchlist.recordMode.label')}
                  </p>
                  <Select
                    value={watchRecordMode}
                    onValueChange={(value) => setWatchRecordMode(value as TikTokLiveRecordMode)}
                    disabled={busy || watchBusy}
                  >
                    <SelectTrigger aria-label={t('tiktokLive.watchlist.recordMode.label')}>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {RECORD_MODE_OPTIONS.map((option) => (
                        <SelectItem key={option} value={option}>
                          {t(`tiktokLive.watchlist.recordMode.${option}`)}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <div className="rounded-lg border border-border/50 bg-muted/20 px-3 py-2.5">
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="text-sm font-medium">
                      {t('tiktokLive.watchlist.schedule.enabled')}
                    </div>
                    <p className="mt-0.5 text-xs text-muted-foreground">
                      {t('tiktokLive.watchlist.schedule.description')}
                    </p>
                  </div>
                  <Switch
                    checked={watchScheduleEnabled}
                    onCheckedChange={setWatchScheduleEnabled}
                    disabled={busy || watchBusy}
                    aria-label={t('tiktokLive.watchlist.schedule.enabled')}
                  />
                </div>
                {watchScheduleEnabled && (
                  <div className="mt-3 grid gap-3 xl:grid-cols-[minmax(0,2fr)_1fr_1fr]">
                    <fieldset className="min-w-0">
                      <legend className="mb-1 text-xs text-muted-foreground">
                        {t('tiktokLive.watchlist.schedule.days')}
                      </legend>
                      <div className="flex flex-wrap gap-1.5">
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          aria-pressed={selectedScheduleDays.size === 0}
                          onClick={() => setWatchScheduleDays('')}
                          disabled={busy || watchBusy}
                          className={cn(
                            'h-9 border-dashed px-3',
                            selectedScheduleDays.size === 0 &&
                              'border-cyan-500/50 bg-cyan-500/10 text-cyan-700 dark:text-cyan-300',
                          )}
                        >
                          {t('tiktokLive.presets.everyDay')}
                        </Button>
                        {scheduleDayLabels.map(({ day, label }) => {
                          const selected = selectedScheduleDays.has(day);
                          return (
                            <Button
                              key={`schedule-${label}`}
                              type="button"
                              size="sm"
                              variant="outline"
                              aria-pressed={selected}
                              onClick={() => toggleScheduleDay(day)}
                              disabled={busy || watchBusy}
                              className={cn(
                                'h-9 min-w-10 border-dashed px-2',
                                selected &&
                                  'border-cyan-500/50 bg-cyan-500/10 text-cyan-700 dark:text-cyan-300',
                              )}
                            >
                              {label}
                            </Button>
                          );
                        })}
                      </div>
                    </fieldset>
                    <div>
                      <p className="mb-1 text-xs text-muted-foreground">
                        {t('tiktokLive.watchlist.schedule.start')}
                      </p>
                      <Input
                        type="time"
                        value={watchScheduleStart}
                        onChange={(event) => setWatchScheduleStart(event.target.value)}
                        disabled={busy || watchBusy}
                        aria-label={t('tiktokLive.watchlist.schedule.start')}
                      />
                    </div>
                    <div>
                      <p className="mb-1 text-xs text-muted-foreground">
                        {t('tiktokLive.watchlist.schedule.end')}
                      </p>
                      <Input
                        type="time"
                        value={watchScheduleEnd}
                        onChange={(event) => setWatchScheduleEnd(event.target.value)}
                        disabled={busy || watchBusy}
                        aria-label={t('tiktokLive.watchlist.schedule.end')}
                      />
                    </div>
                  </div>
                )}
              </div>

              <Collapsible className="rounded-lg border border-border/50 bg-muted/15">
                <CollapsibleTrigger asChild>
                  <button
                    type="button"
                    className="flex w-full items-center justify-between gap-3 px-3 py-2.5 text-left text-sm font-medium"
                  >
                    <span className="flex items-center gap-2">
                      <Settings2 className="h-4 w-4 text-cyan-500" />
                      {t('tiktokLive.presets.advancedRules')}
                    </span>
                    <ChevronDown className="h-4 w-4 text-muted-foreground transition-transform [[data-state=open]_&]:rotate-180" />
                  </button>
                </CollapsibleTrigger>
                <CollapsibleContent className="border-t border-border/50 px-3 py-3">
                  <div
                    className={cn(
                      'grid gap-3',
                      watchRecordMode === 'alwaysAfterCooldown' && 'sm:grid-cols-2',
                    )}
                  >
                    {watchRecordMode === 'alwaysAfterCooldown' && (
                      <div>
                        <p className="mb-1 text-xs text-muted-foreground">
                          {t('tiktokLive.watchlist.cooldownSeconds')}
                        </p>
                        <Select
                          value={watchCooldownMode}
                          onValueChange={(value) => {
                            setWatchCooldownMode(value);
                            if (value === 'custom') {
                              if (watchCooldownMode !== 'custom') setWatchCooldown('1800');
                              return;
                            }
                            setWatchCooldown(value);
                          }}
                          disabled={busy || watchBusy}
                        >
                          <SelectTrigger aria-label={t('tiktokLive.watchlist.cooldownSeconds')}>
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {COOLDOWN_PRESETS.map((preset) => (
                              <SelectItem key={preset.value} value={preset.value}>
                                {preset.label}
                              </SelectItem>
                            ))}
                            <SelectItem value="custom">{t('tiktokLive.presets.custom')}</SelectItem>
                          </SelectContent>
                        </Select>
                        {watchCooldownMode === 'custom' && (
                          <div className="relative mt-2">
                            <Input
                              type="number"
                              min="0"
                              max="604800"
                              step="1"
                              inputMode="numeric"
                              value={watchCooldown}
                              onChange={(event) => setWatchCooldown(event.target.value)}
                              disabled={busy || watchBusy}
                              className="pr-8"
                              aria-label={t('tiktokLive.watchlist.cooldownSeconds')}
                            />
                            <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">
                              s
                            </span>
                          </div>
                        )}
                      </div>
                    )}
                    <div>
                      <p className="mb-1 text-xs text-muted-foreground">
                        {t('tiktokLive.watchlist.filenameTemplate')}
                      </p>
                      <Select
                        value={watchFilenameTemplateMode}
                        onValueChange={(value) => {
                          setWatchFilenameTemplateMode(value);
                          const preset = FILENAME_TEMPLATE_PRESETS.find(
                            (candidate) => candidate.id === value,
                          );
                          if (preset) {
                            setWatchFilenameTemplate(preset.value);
                          } else if (watchFilenameTemplateMode !== 'custom') {
                            setWatchFilenameTemplate('{username}_{date}');
                          }
                        }}
                        disabled={busy || watchBusy}
                      >
                        <SelectTrigger aria-label={t('tiktokLive.watchlist.filenameTemplate')}>
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="automatic">
                            {t('tiktokLive.presets.automatic')}
                          </SelectItem>
                          <SelectItem value="username">
                            {t('tiktokLive.presets.filenameUsername')}
                          </SelectItem>
                          <SelectItem value="title">
                            {t('tiktokLive.presets.filenameTitle')}
                          </SelectItem>
                          <SelectItem value="custom">{t('tiktokLive.presets.custom')}</SelectItem>
                        </SelectContent>
                      </Select>
                      {watchFilenameTemplateMode === 'custom' && (
                        <Input
                          value={watchFilenameTemplate}
                          onChange={(event) => setWatchFilenameTemplate(event.target.value)}
                          disabled={busy || watchBusy}
                          className="mt-2"
                          placeholder="{username}_{date}_{time}"
                          aria-label={t('tiktokLive.watchlist.filenameTemplate')}
                        />
                      )}
                    </div>
                  </div>
                </CollapsibleContent>
              </Collapsible>

              <div className="flex flex-wrap items-center justify-end gap-2">
                {editingWatchId && (
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setEditingWatchId(null);
                      setEditingWatchAuth(null);
                    }}
                    disabled={watchBusy}
                  >
                    {t('tiktokLive.watchlist.actions.cancelEdit')}
                  </Button>
                )}
                <Button
                  onClick={() => void saveWatchEntry()}
                  disabled={!canSaveWatch}
                  className="gap-2 bg-cyan-600 text-white hover:bg-cyan-500"
                >
                  {watchActionId === (editingWatchId || 'new') ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <BookmarkPlus className="h-4 w-4" />
                  )}
                  {t(
                    editingWatchId
                      ? 'tiktokLive.watchlist.actions.update'
                      : 'tiktokLive.watchlist.actions.add',
                  )}
                </Button>
              </div>
            </TabsContent>

            <TabsContent value="record" className="mt-0">
              <div className="flex flex-col gap-3 rounded-xl border border-rose-500/20 bg-rose-500/5 p-3 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex min-w-0 items-center gap-3">
                  <div className="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-rose-500/10 text-rose-500">
                    {isRecording ? (
                      <Radio className="h-5 w-5 animate-pulse" />
                    ) : (
                      <Play className="h-5 w-5" />
                    )}
                  </div>
                  <div className="min-w-0">
                    <p className="text-sm font-medium">{t('tiktokLive.actions.record')}</p>
                    <p className="mt-0.5 truncate text-xs text-muted-foreground">
                      {captureSummary} · {formatDuration(duration)}
                    </p>
                  </div>
                </div>
                {!isRecording ? (
                  <Button
                    onClick={() => void startRecording()}
                    disabled={!canRecord}
                    className="min-w-44 gap-2 bg-rose-600 text-white hover:bg-rose-500"
                  >
                    <Radio className="h-4 w-4" />
                    {t('tiktokLive.actions.record')}
                  </Button>
                ) : (
                  <Button
                    variant="destructive"
                    onClick={() => void cancelRecording()}
                    className="min-w-44 gap-2"
                    disabled={!canCancel}
                  >
                    {isCancelling ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Square className="h-4 w-4" />
                    )}
                    {t('tiktokLive.actions.cancel')}
                  </Button>
                )}
              </div>
            </TabsContent>
          </Tabs>

          {status && !error && (
            <output
              className={cn(
                'flex items-center gap-2 rounded-xl border px-3 py-2.5 text-sm',
                isRecording
                  ? 'border-rose-500/20 bg-rose-500/10 text-rose-600 dark:text-rose-400'
                  : 'border-cyan-500/20 bg-cyan-500/10 text-cyan-700 dark:text-cyan-300',
              )}
              aria-live="polite"
            >
              {busy || watchBusy ? (
                <Loader2 className="h-4 w-4 shrink-0 animate-spin" />
              ) : (
                <CheckCircle2 className="h-4 w-4 shrink-0" />
              )}
              {status}
            </output>
          )}

          {error && (
            <div
              className="rounded-xl border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-500"
              role="alert"
            >
              {error}
            </div>
          )}
        </section>

        <section
          aria-label={t('tiktokLive.telemetry.title')}
          className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4"
        >
          {[
            {
              icon: Activity,
              value: t('tiktokLive.telemetry.active', { count: telemetry.active }),
              detail: t('tiktokLive.telemetry.recoverable', { count: telemetry.recoverable }),
              tone: 'bg-rose-500/10 text-rose-500',
            },
            {
              icon: Wifi,
              value: t('tiktokLive.telemetry.watched', {
                count: telemetry.watched,
                enabled: telemetry.enabled,
              }),
              detail: `${telemetry.enabled}/${telemetry.watched}`,
              tone: 'bg-cyan-500/10 text-cyan-500',
            },
            {
              icon: Layers3,
              value: t('tiktokLive.telemetry.segments', { count: telemetry.segments }),
              detail: `${t('tiktokLive.telemetry.refreshes', {
                count: telemetry.refreshes,
              })} · ${t('tiktokLive.telemetry.reconnects', {
                count: telemetry.reconnects,
              })}`,
              tone: 'bg-blue-500/10 text-blue-500',
            },
            {
              icon: HardDrive,
              value: t('tiktokLive.telemetry.recordedSize', {
                size: formatSize(telemetry.fileSize) || '0 KB',
              }),
              detail: t('tiktokLive.output.label'),
              tone: 'bg-emerald-500/10 text-emerald-500',
            },
          ].map(({ icon: Icon, value, detail, tone }) => (
            <div
              key={value}
              className="flex items-center gap-3 rounded-xl border border-border/60 bg-card/40 p-3"
            >
              <div className={cn('grid h-9 w-9 shrink-0 place-items-center rounded-lg', tone)}>
                <Icon className="h-4 w-4" />
              </div>
              <div className="min-w-0">
                <p className="truncate text-xs font-medium text-foreground">{value}</p>
                <p className="mt-0.5 truncate text-[11px] text-muted-foreground">{detail}</p>
              </div>
            </div>
          ))}
          {telemetry.resourceWarning && (
            <p className="rounded-xl border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-600 sm:col-span-2 xl:col-span-4 dark:text-amber-400">
              {t(`tiktokLive.telemetry.warnings.${telemetry.resourceWarning}`)}
            </p>
          )}
        </section>

        {workspaceView === 'watchlist' && (
          <section className="space-y-3 rounded-xl border border-cyan-500/20 bg-cyan-500/5 p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h2 className="flex items-center gap-2 text-sm font-medium">
                  <ListVideo className="h-4 w-4 text-cyan-500" />
                  {t('tiktokLive.watchlist.title')}
                </h2>
                <p className="mt-1 text-xs text-muted-foreground">
                  {t('tiktokLive.watchlist.description')}
                </p>
              </div>
              <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
                <div className="flex items-center gap-2 rounded-xl bg-muted/25 px-2 py-1">
                  <span className="text-xs text-muted-foreground">
                    {t('tiktokLive.watchlist.maxRooms')}
                  </span>
                  <Select
                    value={maxConcurrentRecordings}
                    onValueChange={(value) => void updateRecorderLimit(value)}
                    disabled={watchBusy}
                  >
                    <SelectTrigger
                      className="h-8 w-16"
                      aria-label={t('tiktokLive.watchlist.maxRooms')}
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {Array.from({ length: recorderConfig?.hardLimit ?? 4 }, (_, index) => {
                        const value = (index + 1).toString();
                        return (
                          <SelectItem key={value} value={value}>
                            {value}
                          </SelectItem>
                        );
                      })}
                    </SelectContent>
                  </Select>
                </div>
                <Badge className="bg-cyan-500/10 text-cyan-600 dark:text-cyan-400">
                  {recorderConfig
                    ? `${recorderConfig.activeRecordings}/${recorderConfig.maxConcurrentRecordings}`
                    : watchEntries.length}
                </Badge>
              </div>
            </div>

            {watchEntries.length === 0 ? (
              <div className="grid min-h-32 place-items-center rounded-xl border border-dashed border-border/60 bg-background/20 px-4 py-6 text-center">
                <div>
                  <ListVideo className="mx-auto h-6 w-6 text-muted-foreground/60" />
                  <p className="mt-2 text-sm font-medium">{t('tiktokLive.watchlist.title')}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {t('tiktokLive.watchlist.empty')}
                  </p>
                </div>
              </div>
            ) : (
              <div className="grid gap-3">
                {watchEntries.map((entry) => {
                  const actionPending = watchActionId === entry.id;
                  const recording = entry.status === 'recording';
                  const recoverable = entry.status === 'recoverable';
                  return (
                    <article
                      key={entry.id}
                      className="rounded-xl border border-border/60 bg-card/60 p-3 transition-colors hover:border-cyan-500/25"
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="flex min-w-0 items-start gap-3">
                          <TikTokLiveThumbnail
                            src={entry.avatar || entry.thumbnail}
                            alt={
                              entry.lastTitle ||
                              (entry.username ? `@${entry.username}` : entry.targetInput)
                            }
                            className="h-12 w-12 rounded-full"
                          />
                          <div className="min-w-0">
                            <p className="truncate text-sm font-medium">
                              {entry.lastTitle ||
                                (entry.username ? `@${entry.username}` : entry.targetInput)}
                            </p>
                            <p className="mt-1 truncate text-xs text-muted-foreground">
                              {entry.lastUploader ||
                                (entry.username ? `@${entry.username}` : entry.targetUrl)}
                            </p>
                            {entry.lastViewerCount !== undefined && (
                              <span className="mt-1 inline-flex items-center gap-1 rounded bg-rose-500/10 px-1.5 py-0.5 text-[11px] text-rose-600 dark:text-rose-400">
                                <Eye className="h-3 w-3" />
                                {t(
                                  entry.status === 'online' || entry.status === 'recording'
                                    ? 'tiktokLive.watchlist.viewers'
                                    : 'tiktokLive.watchlist.lastViewers',
                                  {
                                    count: formatViewerCount(entry.lastViewerCount, i18n.language),
                                  },
                                )}
                              </span>
                            )}
                            <p className="mt-1 truncate text-[11px] text-muted-foreground/70">
                              {entry.targetUrl}
                            </p>
                          </div>
                        </div>
                        <div className="flex shrink-0 items-center gap-2">
                          <Badge className={watchStatusClass(entry.status)}>
                            {t(`tiktokLive.watchlist.status.${entry.status}`)}
                          </Badge>
                          <Switch
                            checked={entry.enabled}
                            onCheckedChange={(enabled) => void toggleWatchEntry(entry, enabled)}
                            disabled={watchBusy}
                            aria-label={t('tiktokLive.watchlist.enabled')}
                          />
                        </div>
                      </div>
                      <div className="mt-3 grid gap-x-4 gap-y-1 rounded-lg bg-muted/20 p-2.5 text-xs text-muted-foreground sm:grid-cols-2">
                        <span className="truncate" title={entry.outputDir}>
                          {t('tiktokLive.output.label')}: {entry.outputDir}
                        </span>
                        <span>
                          {t('tiktokLive.watchlist.intervalValue', {
                            seconds: entry.pollIntervalSeconds,
                          })}
                        </span>
                        <span>
                          {t('tiktokLive.watchlist.recordMode.label')}:{' '}
                          {t(`tiktokLive.watchlist.recordMode.${entry.recordMode}`)}
                        </span>
                        <span>
                          {t('tiktokLive.watchlist.cooldownValue', {
                            seconds: entry.cooldownSeconds,
                          })}
                        </span>
                        {entry.scheduleEnabled && (
                          <span>
                            {t('tiktokLive.watchlist.schedule.value', {
                              days:
                                entry.scheduleDays || t('tiktokLive.watchlist.schedule.everyDay'),
                              start: minuteToTime(entry.scheduleStartMinute) || '00:00',
                              end: minuteToTime(entry.scheduleEndMinute) || '23:59',
                            })}
                          </span>
                        )}
                        {entry.lastCheckedAt && (
                          <span>
                            {t('tiktokLive.watchlist.lastChecked')}:{' '}
                            {new Date(entry.lastCheckedAt * 1000).toLocaleString()}
                          </span>
                        )}
                        {entry.enabled && !recoverable && (
                          <span>
                            {t('tiktokLive.watchlist.nextCheck')}:{' '}
                            {new Date(entry.nextCheckAt * 1000).toLocaleString()}
                          </span>
                        )}
                        {entry.lastOutcome && (
                          <span>
                            {t('tiktokLive.watchlist.lastOutcome')}: {entry.lastOutcome}
                          </span>
                        )}
                        {(entry.lastSegmentCount > 0 ||
                          entry.lastRefreshCount > 0 ||
                          entry.lastReconnectCount > 0) && (
                          <span>
                            {t('tiktokLive.watchlist.telemetry', {
                              segments: entry.lastSegmentCount,
                              refreshes: entry.lastRefreshCount,
                              reconnects: entry.lastReconnectCount,
                            })}
                          </span>
                        )}
                        {entry.lastFileSize && (
                          <span>
                            {t('tiktokLive.watchlist.lastSize')}: {formatSize(entry.lastFileSize)}
                          </span>
                        )}
                      </div>
                      {entry.lastError && (
                        <p className="mt-2 rounded-lg bg-amber-500/10 px-2.5 py-2 text-xs text-amber-600 dark:text-amber-400">
                          {t(`tiktokLive.watchlist.errors.${entry.lastError}`, {
                            defaultValue: entry.lastError,
                          })}
                        </p>
                      )}
                      <div className="mt-3 flex flex-wrap gap-2">
                        <Button
                          size="sm"
                          variant="outline"
                          className="gap-2"
                          disabled={watchBusy || recording || recoverable}
                          onClick={() => void inspectWatchEntry(entry)}
                        >
                          {actionPending ? (
                            <Loader2 className="h-4 w-4 animate-spin" />
                          ) : (
                            <Eye className="h-4 w-4" />
                          )}
                          {t('tiktokLive.watchlist.actions.inspect')}
                        </Button>
                        {recording ? (
                          <Button
                            size="sm"
                            variant="destructive"
                            className="gap-2"
                            disabled={watchBusy || !entry.activeJobId}
                            onClick={() => void cancelWatchRecording(entry)}
                          >
                            <Square className="h-4 w-4" />
                            {t('tiktokLive.watchlist.actions.stop')}
                          </Button>
                        ) : (
                          <Button
                            size="sm"
                            variant="outline"
                            className="gap-2"
                            disabled={busy || watchBusy || recoverable}
                            onClick={() => void recordWatchEntry(entry)}
                          >
                            <Radio className="h-4 w-4" />
                            {t('tiktokLive.watchlist.actions.record')}
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="outline"
                          className="gap-2"
                          disabled={watchBusy || recording || recoverable}
                          onClick={() => editWatchEntry(entry)}
                        >
                          <Pencil className="h-4 w-4" />
                          {t('tiktokLive.watchlist.actions.edit')}
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          className="gap-2 border-red-500/30 text-red-500 hover:bg-red-500/10 hover:text-red-500"
                          disabled={watchBusy || recording}
                          onClick={() => setWatchDeleteCandidate(entry)}
                        >
                          <Trash2 className="h-4 w-4" />
                          {t('tiktokLive.watchlist.actions.remove')}
                        </Button>
                      </div>
                    </article>
                  );
                })}
              </div>
            )}
          </section>
        )}

        {recoveryJobs.length > 0 && (
          <section className="space-y-3 rounded-xl border border-amber-500/20 bg-amber-500/5 p-4">
            <div>
              <h2 className="flex items-center gap-2 text-sm font-medium">
                <RefreshCw className="h-4 w-4 text-amber-500" />
                {t('tiktokLive.recovery.title')}
              </h2>
              <p className="mt-1 text-xs text-muted-foreground">
                {t('tiktokLive.recovery.description')}
              </p>
            </div>
            <div className="grid gap-3">
              {recoveryJobs.map((job) => {
                const actionPending = recoveryActionId === job.id;
                return (
                  <div key={job.id} className="rounded-xl border border-border/60 bg-card/50 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">{job.title}</p>
                        <p className="mt-1 truncate text-xs text-muted-foreground">{job.target}</p>
                        <p
                          className="mt-1 truncate text-xs text-muted-foreground"
                          title={job.outputDir}
                        >
                          {t('tiktokLive.output.label')}: {job.outputDir}
                        </p>
                      </div>
                      <Badge className="shrink-0 bg-amber-500/10 text-amber-600 dark:text-amber-400">
                        {t(`tiktokLive.recovery.status.${job.status}`)}
                      </Badge>
                    </div>
                    <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                      <span>{t('tiktokLive.recovery.segments', { count: job.segmentCount })}</span>
                      <span>
                        {t('tiktokLive.recovery.reconnects', { count: job.reconnectCount })}
                      </span>
                      <span>{new Date(job.updatedAt * 1000).toLocaleString()}</span>
                    </div>
                    {job.errorMessage && (
                      <p className="mt-2 text-xs text-muted-foreground">{job.errorMessage}</p>
                    )}
                    <div className="mt-3 flex flex-wrap gap-2">
                      <Button
                        size="sm"
                        variant="outline"
                        className="gap-2"
                        disabled={busy}
                        onClick={() => void continueRecovery(job)}
                      >
                        {actionPending ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <Play className="h-4 w-4" />
                        )}
                        {t('tiktokLive.recovery.actions.continue')}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="gap-2"
                        disabled={busy || !job.hasMedia}
                        onClick={() => void finalizeRecovery(job)}
                      >
                        <FileCheck2 className="h-4 w-4" />
                        {t('tiktokLive.recovery.actions.finalize')}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="gap-2 border-red-500/30 text-red-500 hover:bg-red-500/10 hover:text-red-500"
                        disabled={busy}
                        onClick={() => setDeleteCandidate(job)}
                      >
                        <Trash2 className="h-4 w-4" />
                        {t('tiktokLive.recovery.actions.delete')}
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {workspaceView === 'record' && !inspectResult && !recordResult && (
          <section className="grid min-h-32 place-items-center rounded-xl border border-dashed border-border/60 bg-muted/10 px-4 py-6 text-center">
            <div>
              <Search className="mx-auto h-6 w-6 text-muted-foreground/60" />
              <p className="mt-2 text-sm font-medium">{t('tiktokLive.actions.inspect')}</p>
              <p className="mt-1 text-xs text-muted-foreground">{t('tiktokLive.subtitle')}</p>
            </div>
          </section>
        )}

        {workspaceView === 'record' && inspectResult && (
          <section className="space-y-4 rounded-xl border border-cyan-500/20 bg-cyan-500/5 p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <TikTokLiveThumbnail
                  src={inspectResult.thumbnail}
                  alt={inspectResult.title}
                  className="h-20 w-16 rounded-lg"
                />
                <div className="min-w-0">
                  <h2 className="truncate text-sm font-medium">{inspectResult.title}</h2>
                  <div className="mt-1 flex min-w-0 items-center gap-2">
                    {inspectResult.avatar && (
                      <TikTokLiveThumbnail
                        src={inspectResult.avatar}
                        alt={inspectResult.uploader || inspectResult.title}
                        className="h-6 w-6 rounded-full"
                      />
                    )}
                    <p className="truncate text-xs text-muted-foreground">
                      {inspectResult.uploader || inspectResult.targetUrl}
                    </p>
                  </div>
                </div>
              </div>
              <div className="flex shrink-0 flex-col items-end gap-2">
                <Badge
                  className={cn(
                    'gap-1.5',
                    inspectResult.isLive === false
                      ? 'bg-amber-500/10 text-amber-600'
                      : 'bg-rose-500/10 text-rose-600 dark:text-rose-400',
                  )}
                >
                  <span className="h-1.5 w-1.5 rounded-full bg-current" />
                  {inspectResult.liveStatus ||
                    (inspectResult.isLive === false ? 'offline' : 'live')}
                </Badge>
                {inspectResult.viewerCount !== undefined && (
                  <Badge className="gap-1.5 bg-rose-500/10 text-rose-600 dark:text-rose-400">
                    <Eye className="h-3.5 w-3.5" />
                    {t('tiktokLive.watchlist.viewers', {
                      count: formatViewerCount(inspectResult.viewerCount, i18n.language),
                    })}
                  </Badge>
                )}
              </div>
            </div>
            {inspectResult.selectedVariant && (
              <div className="rounded-lg border border-cyan-500/20 bg-background/40 px-3 py-2 text-xs">
                <span className="font-medium text-foreground">{t('tiktokLive.selected')}:</span>{' '}
                <span className="text-muted-foreground">
                  {variantLabel(inspectResult.selectedVariant)}
                </span>
              </div>
            )}
            <div className="grid gap-2 md:grid-cols-2">
              {inspectResult.variants.slice(0, 8).map((variant) => {
                const selected = variant.formatId === inspectResult.selectedVariant?.formatId;
                return (
                  <div
                    key={variant.formatId}
                    className={cn(
                      'flex min-w-0 items-center gap-2 rounded-lg border px-3 py-2 text-xs',
                      selected
                        ? 'border-cyan-500/30 bg-cyan-500/10'
                        : 'border-border/50 bg-background/30',
                    )}
                  >
                    {selected ? (
                      <CheckCircle2 className="h-4 w-4 shrink-0 text-cyan-500" />
                    ) : (
                      <span className="h-2 w-2 shrink-0 rounded-full bg-muted-foreground/30" />
                    )}
                    <span className="truncate">{variantLabel(variant)}</span>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {workspaceView === 'record' && recordResult && (
          <section
            className={cn(
              'flex flex-col gap-3 rounded-xl border p-4 sm:flex-row sm:items-center sm:justify-between',
              recordResult.partial
                ? 'border-amber-500/20 bg-amber-500/5'
                : 'border-emerald-500/20 bg-emerald-500/5',
            )}
          >
            <div className="flex min-w-0 items-center gap-3">
              <div
                className={cn(
                  'grid h-10 w-10 shrink-0 place-items-center rounded-xl',
                  recordResult.partial
                    ? 'bg-amber-500/10 text-amber-500'
                    : 'bg-emerald-500/10 text-emerald-500',
                )}
              >
                <FileCheck2 className="h-5 w-5" />
              </div>
              <div className="min-w-0">
                <h2 className="text-sm font-medium">{t('tiktokLive.recordResult.title')}</h2>
                <p className="mt-0.5 truncate text-xs text-muted-foreground">
                  {recordResult.filepath}
                </p>
                {recordResult.filesize && (
                  <p className="mt-0.5 text-xs text-muted-foreground">
                    {formatSize(recordResult.filesize)}
                  </p>
                )}
              </div>
            </div>
            <Button
              size="sm"
              variant="outline"
              className="shrink-0 gap-2"
              onClick={() => void openFileLocation(recordResult.filepath)}
            >
              <Folder className="h-4 w-4" />
              {t('tiktokLive.actions.showInFolder')}
            </Button>
          </section>
        )}
      </main>

      <AlertDialog
        open={deleteCandidate !== null}
        onOpenChange={(open) => !open && setDeleteCandidate(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('tiktokLive.recovery.deleteDialog.title')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('tiktokLive.recovery.deleteDialog.description', {
                title: deleteCandidate?.title || '',
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('tiktokLive.recovery.deleteDialog.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              className="bg-red-600 text-white hover:bg-red-700"
              onClick={() => void deleteRecovery()}
            >
              {t('tiktokLive.recovery.deleteDialog.confirm')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        open={watchDeleteCandidate !== null}
        onOpenChange={(open) => !open && setWatchDeleteCandidate(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('tiktokLive.watchlist.deleteDialog.title')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('tiktokLive.watchlist.deleteDialog.description', {
                target: watchDeleteCandidate?.targetInput || '',
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('tiktokLive.watchlist.deleteDialog.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              className="bg-red-600 text-white hover:bg-red-700"
              onClick={() => void deleteWatchEntry()}
            >
              {t('tiktokLive.watchlist.deleteDialog.confirm')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
