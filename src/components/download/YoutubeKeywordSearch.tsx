import { invoke } from '@tauri-apps/api/core';
import {
  AlertCircle,
  ArrowLeft,
  CheckCircle2,
  CheckSquare,
  Loader2,
  Plus,
  Search,
  Square,
  Video,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { extractBackendError, localizeBackendError } from '@/lib/backend-error';
import type {
  YoutubeSearchQueueResult,
  YoutubeSearchResponse,
  YoutubeSearchVideo,
} from '@/lib/types';
import { cn } from '@/lib/utils';

const DEFAULT_LIMIT = 20;
const MIN_LIMIT = 1;
const MAX_LIMIT = 100;
const STORAGE_KEY = 'youwee-youtube-keyword-search-state';

interface YoutubeKeywordSearchProps {
  disabled?: boolean;
  onBack: () => void;
  onAddResults: (results: YoutubeSearchVideo[]) => Promise<YoutubeSearchQueueResult>;
  queuedVideoIds: Set<string>;
}

function clampLimit(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_LIMIT;
  return Math.min(MAX_LIMIT, Math.max(MIN_LIMIT, Math.round(value)));
}

function mergeVideos(
  current: YoutubeSearchVideo[],
  incoming: YoutubeSearchVideo[],
): YoutubeSearchVideo[] {
  const seen = new Set(current.map((video) => video.id));
  const merged = [...current];
  for (const video of incoming) {
    if (seen.has(video.id)) continue;
    seen.add(video.id);
    merged.push(video);
  }
  return merged;
}

interface StoredYoutubeKeywordSearchState {
  query?: string;
  limit?: number;
  videos?: YoutubeSearchVideo[];
  selectedIds?: string[];
  continuation?: string | null;
}

function loadStoredState(): StoredYoutubeKeywordSearchState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as StoredYoutubeKeywordSearchState;
    return {
      query: typeof parsed.query === 'string' ? parsed.query : '',
      limit: clampLimit(Number(parsed.limit)),
      videos: Array.isArray(parsed.videos) ? parsed.videos : [],
      selectedIds: Array.isArray(parsed.selectedIds) ? parsed.selectedIds : [],
      continuation: typeof parsed.continuation === 'string' ? parsed.continuation : null,
    };
  } catch {
    return {};
  }
}

export function YoutubeKeywordSearch({
  disabled,
  onBack,
  onAddResults,
  queuedVideoIds,
}: YoutubeKeywordSearchProps) {
  const { t } = useTranslation('download');
  const [storedState] = useState(loadStoredState);
  const [query, setQuery] = useState(storedState.query || '');
  const [limit, setLimit] = useState(clampLimit(storedState.limit || DEFAULT_LIMIT));
  const [videos, setVideos] = useState<YoutubeSearchVideo[]>(storedState.videos || []);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(
    () => new Set(storedState.selectedIds || []),
  );
  const [continuation, setContinuation] = useState<string | null>(storedState.continuation || null);
  const [isSearching, setIsSearching] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedVideos = useMemo(
    () => videos.filter((video) => selectedIds.has(video.id) && !queuedVideoIds.has(video.id)),
    [queuedVideoIds, selectedIds, videos],
  );

  useEffect(() => {
    try {
      const state: StoredYoutubeKeywordSearchState = {
        query,
        limit,
        videos,
        selectedIds: Array.from(selectedIds),
        continuation,
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
      // Ignore storage failures; search state can be rebuilt by running the query again.
    }
  }, [continuation, limit, query, selectedIds, videos]);

  const runSearch = useCallback(
    async (nextContinuation?: string | null) => {
      const trimmedQuery = query.trim();
      if (!trimmedQuery) return;

      const loadingMore = Boolean(nextContinuation);
      if (loadingMore) {
        setIsLoadingMore(true);
      } else {
        setIsSearching(true);
        setVideos([]);
        setSelectedIds(new Set());
        setContinuation(null);
      }
      setError(null);

      try {
        const response = await invoke<YoutubeSearchResponse>('search_youtube_videos', {
          query: trimmedQuery,
          limit: clampLimit(limit),
          continuation: nextContinuation || null,
        });

        setVideos((current) =>
          loadingMore ? mergeVideos(current, response.videos) : response.videos,
        );
        setContinuation(response.continuation || null);
      } catch (searchError) {
        const payload = extractBackendError(searchError);
        setError(localizeBackendError(payload));
      } finally {
        setIsSearching(false);
        setIsLoadingMore(false);
      }
    },
    [limit, query],
  );

  const toggleSelected = useCallback(
    (id: string) => {
      if (queuedVideoIds.has(id)) return;
      setSelectedIds((current) => {
        const next = new Set(current);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        return next;
      });
    },
    [queuedVideoIds],
  );

  const selectAll = useCallback(() => {
    setSelectedIds(
      new Set(videos.filter((video) => !queuedVideoIds.has(video.id)).map((video) => video.id)),
    );
  }, [queuedVideoIds, videos]);

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
  }, []);

  const addSelected = useCallback(async () => {
    if (selectedVideos.length === 0) return;
    setIsAdding(true);
    try {
      const result = await onAddResults(selectedVideos);
      if (result.queuedIds.length > 0) {
        setSelectedIds((current) => {
          const next = new Set(current);
          for (const id of result.queuedIds) {
            next.delete(id);
          }
          return next;
        });
      }
    } finally {
      setIsAdding(false);
    }
  }, [onAddResults, selectedVideos]);

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    void runSearch();
  };

  const hasResults = videos.length > 0;
  const busy = disabled || isSearching || isLoadingMore || isAdding;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-4 sm:p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <button
            type="button"
            onClick={onBack}
            className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-background/50 text-muted-foreground transition-colors hover:bg-muted/60 hover:text-foreground"
            title={t('urlInput.keyword.back')}
          >
            <ArrowLeft className="h-4 w-4" />
          </button>
          <div className="min-w-0">
            <h2 className="text-base font-semibold leading-6">{t('urlInput.keyword.pageTitle')}</h2>
            <p className="truncate text-sm text-muted-foreground">
              {t('urlInput.keyword.pageDescription')}
            </p>
          </div>
        </div>
      </div>

      <form
        onSubmit={handleSubmit}
        className="flex flex-col gap-2 rounded-lg border border-border/60 bg-muted/20 p-2.5 sm:flex-row"
      >
        <div className="relative min-w-0 flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            disabled={disabled || isSearching}
            placeholder={t('urlInput.keyword.placeholder')}
            className="h-11 bg-background/50 pl-10"
          />
        </div>
        <div className="flex gap-2">
          <Input
            type="number"
            min={MIN_LIMIT}
            max={MAX_LIMIT}
            value={limit}
            onChange={(event) => setLimit(clampLimit(Number(event.target.value)))}
            disabled={disabled || isSearching}
            className="h-11 w-24 bg-background/50"
            aria-label={t('urlInput.keyword.limitLabel')}
          />
          <Button
            type="submit"
            disabled={disabled || isSearching || !query.trim()}
            className="h-11 gap-2"
          >
            {isSearching ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Search className="h-4 w-4" />
            )}
            {t('urlInput.keyword.search')}
          </Button>
        </div>
      </form>

      <div className="min-h-[220px] flex-1 overflow-hidden rounded-lg border border-border/60 bg-background/45">
        {error ? (
          <div className="flex h-full min-h-[220px] items-center justify-center p-4">
            <div className="max-w-sm text-center">
              <AlertCircle className="mx-auto mb-2 h-7 w-7 text-destructive" />
              <p className="text-sm font-medium">{t('urlInput.keyword.errorTitle')}</p>
              <p className="mt-1 text-sm text-muted-foreground">{error}</p>
            </div>
          </div>
        ) : isSearching ? (
          <div className="flex h-full min-h-[220px] items-center justify-center p-4 text-sm text-muted-foreground">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('urlInput.keyword.searching')}
          </div>
        ) : !hasResults ? (
          <div className="flex h-full min-h-[220px] items-center justify-center p-4">
            <div className="max-w-sm text-center">
              <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-primary/10 text-primary">
                <Video className="h-6 w-6" />
              </div>
              <p className="text-sm font-medium">{t('urlInput.keyword.emptyTitle')}</p>
              <p className="mt-1 text-sm text-muted-foreground">
                {t('urlInput.keyword.emptyDescription')}
              </p>
            </div>
          </div>
        ) : (
          <ScrollArea className="h-full">
            <div className="divide-y divide-border/50">
              {videos.map((video) => {
                const selected = selectedIds.has(video.id);
                const isAdded = queuedVideoIds.has(video.id);
                return (
                  <button
                    type="button"
                    key={video.id}
                    onClick={() => toggleSelected(video.id)}
                    disabled={busy || isAdded}
                    className={cn(
                      'grid w-full grid-cols-[auto_96px_minmax(0,1fr)] gap-3 px-3 py-2.5 text-left transition-colors',
                      isAdded
                        ? 'bg-emerald-500/5'
                        : selected
                          ? 'bg-primary/5'
                          : 'hover:bg-muted/40',
                    )}
                  >
                    <span className="mt-7 text-muted-foreground">
                      {isAdded ? (
                        <CheckCircle2 className="h-4 w-4 text-emerald-500" />
                      ) : selected ? (
                        <CheckSquare className="h-4 w-4 text-primary" />
                      ) : (
                        <Square className="h-4 w-4" />
                      )}
                    </span>
                    <span className="relative aspect-video overflow-hidden rounded-md bg-muted">
                      {video.thumbnail ? (
                        <img
                          src={video.thumbnail}
                          alt=""
                          className="h-full w-full object-cover"
                          loading="lazy"
                        />
                      ) : (
                        <span className="flex h-full w-full items-center justify-center text-muted-foreground">
                          <Video className="h-5 w-5" />
                        </span>
                      )}
                      {video.duration ? (
                        <span className="absolute bottom-1 right-1 rounded bg-black/75 px-1.5 py-0.5 text-[10px] font-medium text-white">
                          {video.duration}
                        </span>
                      ) : null}
                    </span>
                    <span className="min-w-0 self-center">
                      <span className="line-clamp-2 text-sm font-medium leading-5">
                        {video.title}
                      </span>
                      <span className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                        {video.channel ? <span className="truncate">{video.channel}</span> : null}
                        {video.view_count_text ? <span>{video.view_count_text}</span> : null}
                        {video.published_time_text ? (
                          <span>{video.published_time_text}</span>
                        ) : null}
                        {isAdded ? (
                          <span className="inline-flex items-center gap-1 rounded bg-emerald-500/10 px-1.5 py-0.5 font-medium text-emerald-600 dark:text-emerald-400">
                            <CheckCircle2 className="h-3 w-3" />
                            {t('urlInput.keyword.added')}
                          </span>
                        ) : null}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          </ScrollArea>
        )}
      </div>

      {hasResults ? (
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-border/60 bg-muted/25 px-2.5 py-2">
          <div className="text-xs text-muted-foreground">
            {t('urlInput.keyword.selectedCount', {
              selected: selectedVideos.length,
              total: videos.length,
            })}
          </div>
          <div className="flex flex-wrap items-center gap-1.5">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={selectAll}
              disabled={busy || videos.every((video) => queuedVideoIds.has(video.id))}
              className="h-8 gap-1.5 text-xs"
            >
              <CheckSquare className="h-3.5 w-3.5" />
              {t('urlInput.keyword.selectAll')}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={clearSelection}
              disabled={busy || selectedVideos.length === 0}
              className="h-8 gap-1.5 text-xs"
            >
              <X className="h-3.5 w-3.5" />
              {t('urlInput.keyword.clearSelection')}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void runSearch(continuation)}
              disabled={busy || !continuation}
              className="h-8 gap-1.5 text-xs"
            >
              {isLoadingMore ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Search className="h-3.5 w-3.5" />
              )}
              {t('urlInput.keyword.loadMore')}
            </Button>
            <button
              type="button"
              onClick={() => void addSelected()}
              disabled={busy || selectedVideos.length === 0}
              className="inline-flex h-8 items-center gap-1.5 rounded-md px-3 text-xs font-medium btn-gradient disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isAdding ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Plus className="h-3.5 w-3.5" />
              )}
              {t('urlInput.keyword.addSelected')}
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
