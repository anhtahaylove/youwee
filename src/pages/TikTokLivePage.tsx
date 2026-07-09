import { invoke } from '@tauri-apps/api/core';
import { downloadDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';
import { Folder, Loader2, Radio, Search, Square } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ThemePicker } from '@/components/settings/ThemePicker';
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
};

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

function isCancellationError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.toLowerCase().includes('cancelled');
}

export function TikTokLivePage() {
  const { t } = useTranslation('pages');
  const [input, setInput] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [duration, setDuration] = useState('60');
  const [quality, setQuality] = useState('auto');
  const [transport, setTransport] = useState('auto');
  const [inspectResult, setInspectResult] = useState<TikTokLiveInspectResult | null>(null);
  const [recordResult, setRecordResult] = useState<TikTokLiveRecordResult | null>(null);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [isInspecting, setIsInspecting] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const activeJobIdRef = useRef<string | null>(null);

  useEffect(() => {
    void resolveDefaultOutputPath().then(setOutputDir);
  }, []);

  const invokeOptions = useMemo(() => {
    const { cookieSettings, proxySettings } = loadNetworkSettings();
    return buildCookieProxyInvokeOptions(cookieSettings, proxySettings);
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
    setIsInspecting(true);
    setError('');
    setRecordResult(null);
    setStatus(t('tiktokLive.status.inspecting'));
    try {
      const result = await invoke<TikTokLiveInspectResult>('inspect_tiktok_live', {
        input,
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
      setError(err instanceof Error ? err.message : String(err));
      setStatus(t('tiktokLive.status.failed'));
    } finally {
      setIsInspecting(false);
    }
  }, [input, invokeOptions, quality, t, transport]);

  const startRecording = useCallback(async () => {
    if (!input.trim()) return;
    const jobId = crypto.randomUUID();
    activeJobIdRef.current = jobId;
    setIsRecording(true);
    setError('');
    setRecordResult(null);
    setStatus(t('tiktokLive.status.recording'));
    try {
      const seconds = Number.parseInt(duration, 10);
      const result = await invoke<TikTokLiveRecordResult>('record_tiktok_live', {
        jobId,
        input,
        outputDir,
        durationSeconds: Number.isFinite(seconds) && seconds > 0 ? seconds : null,
        preferredQuality: quality,
        preferredTransport: transport,
        ...invokeOptions,
      });
      setRecordResult(result);
      setStatus(t('tiktokLive.status.recorded'));
    } catch (err) {
      if (isCancellationError(err)) {
        setStatus(t('tiktokLive.status.cancelled'));
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
      setStatus(t('tiktokLive.status.failed'));
    } finally {
      activeJobIdRef.current = null;
      setIsRecording(false);
    }
  }, [duration, input, invokeOptions, outputDir, quality, t, transport]);

  const cancelRecording = useCallback(async () => {
    const jobId = activeJobIdRef.current;
    if (!jobId) return;
    await invoke('cancel_tiktok_live_recording', { jobId }).catch(() => {});
    setStatus(t('tiktokLive.status.cancelled'));
  }, [t]);

  const busy = isInspecting || isRecording;
  const canSubmit = input.trim().length > 0 && !busy;

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
              onChange={(event) => setInput(event.target.value)}
              disabled={busy}
              placeholder={t('tiktokLive.input.placeholder')}
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
              <Select value={quality} onValueChange={setQuality} disabled={busy}>
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
              <Select value={transport} onValueChange={setTransport} disabled={busy}>
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
              >
                <Folder className="w-4 h-4" />
                <span className="truncate">{outputDir || t('tiktokLive.output.empty')}</span>
              </Button>
            </div>
          </div>

          <div className="flex items-center gap-2">
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
              >
                <Square className="w-4 h-4" />
                {t('tiktokLive.actions.cancel')}
              </Button>
            )}
            {status && <span className="text-sm text-muted-foreground">{status}</span>}
          </div>

          {error && (
            <div className="rounded-xl border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-500">
              {error}
            </div>
          )}
        </section>

        {inspectResult && (
          <section className="rounded-2xl border border-white/[0.08] bg-card/30 p-4 space-y-3">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <h2 className="truncate text-sm font-medium">{inspectResult.title}</h2>
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {inspectResult.uploader || inspectResult.targetUrl}
                </p>
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
          <section className="rounded-2xl border border-green-500/20 bg-green-500/5 p-4 space-y-3">
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
    </div>
  );
}
