import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { downloadDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';
import {
  BookmarkPlus,
  Eye,
  FileCheck2,
  Folder,
  Loader2,
  Pencil,
  Play,
  Radio,
  RefreshCw,
  Search,
  Square,
  Trash2,
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
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { extractBackendError, localizeBackendError } from '@/lib/backend-error';
import { buildCookieProxyInvokeOptions, loadNetworkSettings } from '@/lib/network-config';
import { openFileLocation } from '@/lib/open-file-location';
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
  title: string;
  uploader?: string;
  thumbnail?: string;
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
  backoffAttempt: number;
  nextCheckAt: number;
  status: TikTokLiveWatchStatus;
  activeJobId?: string;
  lastError?: string;
  lastCheckedAt?: number;
  lastOnlineAt?: number;
  lastRecordingAt?: number;
  createdAt: number;
  updatedAt: number;
};

type TikTokLiveWatchAuthSnapshot = Pick<
  TikTokLiveWatchEntry,
  'cookieMode' | 'cookieBrowser' | 'cookieBrowserProfile' | 'cookieFilePath'
>;

const QUALITY_OPTIONS = ['auto', 'origin', 'uhd_60', 'uhd', 'hd_60', 'hd', 'sd', 'ld', 'ao'];
const TRANSPORT_OPTIONS = ['auto', 'hls', 'flv', 'lls'];

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

function variantLabel(variant?: TikTokLiveVariant): string {
  if (!variant) return '';
  return [
    variant.formatId,
    variant.resolution,
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

export function TikTokLivePage() {
  const { t } = useTranslation('pages');
  const [input, setInput] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [duration, setDuration] = useState('60');
  const [quality, setQuality] = useState('auto');
  const [transport, setTransport] = useState('auto');
  const [autoReconnect, setAutoReconnect] = useState(true);
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

  useEffect(() => {
    void refreshRecoveryJobs();
    void refreshWatchlist();
  }, [refreshRecoveryJobs, refreshWatchlist]);

  useEffect(() => {
    const unlistenPromise = listen('tiktok-live-watchlist-updated', () => {
      void refreshWatchlist();
      void refreshRecoveryJobs();
    });
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshRecoveryJobs, refreshWatchlist]);

  const updateInput = useCallback((value: string) => {
    setInput(value);
    setInspectResult(null);
    setRecordResult(null);
    setStatus('');
    setError('');
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

  const saveWatchEntry = useCallback(async () => {
    if (!input.trim() || !outputDir) return;
    const actionId = editingWatchId || 'new';
    setWatchActionId(actionId);
    setError('');
    try {
      const durationValue = Number.parseInt(duration, 10);
      const pollIntervalValue = Number.parseInt(watchPollInterval, 10);
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
    invokeOptions,
    outputDir,
    quality,
    refreshWatchlist,
    t,
    transport,
    watchPollInterval,
  ]);

  const editWatchEntry = useCallback((entry: TikTokLiveWatchEntry) => {
    setEditingWatchId(entry.id);
    setInput(entry.targetInput);
    setOutputDir(entry.outputDir);
    setDuration(entry.durationSeconds?.toString() || '0');
    setQuality(entry.preferredQuality || 'auto');
    setTransport(entry.preferredTransport || 'auto');
    setWatchPollInterval(entry.pollIntervalSeconds.toString());
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
  const canSubmit = input.trim().length > 0 && !busy;
  const canCancel = isRecording && !isCancelling;
  const watchBusy = watchActionId !== null;
  const canSaveWatch = input.trim().length > 0 && outputDir.length > 0 && !busy && !watchBusy;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <header className="flex-shrink-0 flex items-center justify-between h-12 sm:h-14 px-4 sm:px-6">
        <div className="min-w-0">
          <h1 className="text-base sm:text-lg font-semibold">{t('tiktokLive.title')}</h1>
          <p className="hidden sm:block text-xs text-muted-foreground">
            {t('tiktokLive.subtitle')}
          </p>
        </div>
        <ThemePicker />
      </header>

      <div className="mx-4 sm:mx-6 h-px bg-gradient-to-r from-transparent via-border/50 to-transparent" />

      <main className="flex-1 overflow-y-auto px-4 sm:px-6 py-5 space-y-4">
        <section className="rounded-2xl border border-white/[0.08] bg-card/40 p-4 space-y-4">
          <div className="grid gap-3 md:grid-cols-[1fr_auto]">
            <Input
              value={input}
              onChange={(event) => updateInput(event.target.value)}
              disabled={busy}
              placeholder={t('tiktokLive.input.placeholder')}
              aria-label={t('tiktokLive.input.label')}
            />
            <Button onClick={() => void inspectLive()} disabled={!canSubmit} className="gap-2">
              {isInspecting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Search className="w-4 h-4" />
              )}
              {t('tiktokLive.actions.inspect')}
            </Button>
          </div>

          <div className="grid gap-3 md:grid-cols-4">
            <div>
              <p className="mb-1 text-xs text-muted-foreground">{t('tiktokLive.quality')}</p>
              <Select
                value={quality}
                onValueChange={(value) => {
                  setQuality(value);
                  setInspectResult(null);
                  setStatus('');
                }}
                disabled={busy}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {QUALITY_OPTIONS.map((option) => (
                    <SelectItem key={option} value={option}>
                      {option}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <p className="mb-1 text-xs text-muted-foreground">{t('tiktokLive.transport')}</p>
              <Select
                value={transport}
                onValueChange={(value) => {
                  setTransport(value);
                  setInspectResult(null);
                  setStatus('');
                }}
                disabled={busy}
              >
                <SelectTrigger>
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
            <div>
              <p className="mb-1 text-xs text-muted-foreground">{t('tiktokLive.duration')}</p>
              <Input
                type="number"
                min="0"
                step="1"
                inputMode="numeric"
                value={duration}
                onChange={(event) => setDuration(event.target.value)}
                disabled={busy}
              />
            </div>
            <div>
              <p className="mb-1 text-xs text-muted-foreground">{t('tiktokLive.output.label')}</p>
              <Button
                variant="outline"
                className="w-full justify-start gap-2"
                onClick={() => void selectOutputFolder()}
                disabled={busy}
                title={outputDir || t('tiktokLive.output.empty')}
              >
                <Folder className="w-4 h-4" />
                <span className="truncate">{outputDir || t('tiktokLive.output.empty')}</span>
              </Button>
            </div>
          </div>

          <div className="flex items-start justify-between gap-4 rounded-xl bg-muted/25 px-3 py-2.5">
            <div className="min-w-0">
              <div className="flex items-center gap-2 text-sm font-medium">
                <RefreshCw className="h-4 w-4 text-blue-500" />
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

          <div className="flex flex-wrap items-center gap-2">
            {!isRecording ? (
              <Button onClick={() => void startRecording()} disabled={!canSubmit} className="gap-2">
                <Radio className="w-4 h-4" />
                {t('tiktokLive.actions.record')}
              </Button>
            ) : (
              <Button
                variant="destructive"
                onClick={() => void cancelRecording()}
                className="gap-2"
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
            {status && (
              <output className="text-sm text-muted-foreground" aria-live="polite">
                {status}
              </output>
            )}
            <div className="ml-auto flex flex-wrap items-center gap-2">
              <Input
                type="number"
                min="30"
                max="3600"
                step="1"
                inputMode="numeric"
                value={watchPollInterval}
                onChange={(event) => setWatchPollInterval(event.target.value)}
                className="h-9 w-24"
                disabled={busy || watchBusy}
                aria-label={t('tiktokLive.watchlist.pollInterval')}
                title={t('tiktokLive.watchlist.pollInterval')}
              />
              <Button
                variant="outline"
                onClick={() => void saveWatchEntry()}
                disabled={!canSaveWatch}
                className="gap-2"
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
            </div>
          </div>

          {error && (
            <div
              className="rounded-xl border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-500"
              role="alert"
            >
              {error}
            </div>
          )}
        </section>

        <section className="rounded-2xl border border-blue-500/20 bg-blue-500/5 p-4 space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 className="text-sm font-medium">{t('tiktokLive.watchlist.title')}</h2>
              <p className="mt-1 text-xs text-muted-foreground">
                {t('tiktokLive.watchlist.description')}
              </p>
            </div>
            <Badge className="bg-blue-500/10 text-blue-600 dark:text-blue-400">
              {watchEntries.length}
            </Badge>
          </div>

          {watchEntries.length === 0 ? (
            <p className="rounded-xl border border-dashed border-border/60 px-3 py-4 text-center text-xs text-muted-foreground">
              {t('tiktokLive.watchlist.empty')}
            </p>
          ) : (
            <div className="grid gap-3">
              {watchEntries.map((entry) => {
                const actionPending = watchActionId === entry.id;
                const recording = entry.status === 'recording';
                const recoverable = entry.status === 'recoverable';
                return (
                  <div key={entry.id} className="rounded-xl border border-border/60 bg-card/50 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">
                          {entry.username ? `@${entry.username}` : entry.targetInput}
                        </p>
                        <p className="mt-1 truncate text-xs text-muted-foreground">
                          {entry.targetUrl}
                        </p>
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
                    <div className="mt-2 grid gap-1 text-xs text-muted-foreground sm:grid-cols-2">
                      <span className="truncate" title={entry.outputDir}>
                        {t('tiktokLive.output.label')}: {entry.outputDir}
                      </span>
                      <span>
                        {t('tiktokLive.watchlist.intervalValue', {
                          seconds: entry.pollIntervalSeconds,
                        })}
                      </span>
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
                    </div>
                    {entry.lastError && (
                      <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
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
                  </div>
                );
              })}
            </div>
          )}
        </section>

        {recoveryJobs.length > 0 && (
          <section className="rounded-2xl border border-amber-500/20 bg-amber-500/5 p-4 space-y-3">
            <div>
              <h2 className="text-sm font-medium">{t('tiktokLive.recovery.title')}</h2>
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

        {inspectResult && (
          <section className="rounded-2xl border border-white/[0.08] bg-card/30 p-4 space-y-3">
            <div className="flex items-start justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                {inspectResult.thumbnail && (
                  <img
                    src={inspectResult.thumbnail.replace(/^http:\/\//, 'https://')}
                    alt=""
                    className="h-14 w-14 shrink-0 rounded-lg object-cover"
                  />
                )}
                <div className="min-w-0">
                  <h2 className="truncate text-sm font-medium">{inspectResult.title}</h2>
                  <p className="mt-1 truncate text-xs text-muted-foreground">
                    {inspectResult.uploader || inspectResult.targetUrl}
                  </p>
                </div>
              </div>
              <Badge
                className={cn(
                  'shrink-0',
                  inspectResult.isLive === false
                    ? 'bg-amber-500/10 text-amber-600'
                    : 'bg-green-500/10 text-green-600',
                )}
              >
                {inspectResult.liveStatus || (inspectResult.isLive === false ? 'offline' : 'live')}
              </Badge>
            </div>
            {inspectResult.selectedVariant && (
              <p className="text-xs text-muted-foreground">
                {t('tiktokLive.selected')}: {variantLabel(inspectResult.selectedVariant)}
              </p>
            )}
            <div className="grid gap-2">
              {inspectResult.variants.slice(0, 8).map((variant) => (
                <div
                  key={variant.formatId}
                  className="flex items-center justify-between gap-3 rounded-xl bg-muted/30 px-3 py-2 text-xs"
                >
                  <span className="truncate">{variantLabel(variant)}</span>
                  <span className="shrink-0 text-muted-foreground">{variant.vcodec || '-'}</span>
                </div>
              ))}
            </div>
          </section>
        )}

        {recordResult && (
          <section
            className={cn(
              'rounded-2xl border p-4 space-y-3',
              recordResult.partial
                ? 'border-amber-500/20 bg-amber-500/5'
                : 'border-green-500/20 bg-green-500/5',
            )}
          >
            <div>
              <h2 className="text-sm font-medium">{t('tiktokLive.recordResult.title')}</h2>
              <p className="mt-1 truncate text-xs text-muted-foreground">{recordResult.filepath}</p>
              {recordResult.filesize && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {formatSize(recordResult.filesize)}
                </p>
              )}
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void openFileLocation(recordResult.filepath)}
            >
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
