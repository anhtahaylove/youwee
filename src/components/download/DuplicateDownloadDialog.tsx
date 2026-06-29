import { AlertTriangle, CheckCircle2, FileQuestion, FolderOpen, RotateCcw } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { DownloadDuplicateReview, DownloadDuplicateReviewAction } from '@/lib/types';
import { cn } from '@/lib/utils';

interface DuplicateDownloadDialogProps {
  review: DownloadDuplicateReview | null;
  onResolve: (action: DownloadDuplicateReviewAction, applyToAll: boolean) => void;
}

function filenameFromPath(filepath: string): string {
  const parts = filepath.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) || filepath;
}

function formatDownloadedAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

export function DuplicateDownloadDialog({ review, onResolve }: DuplicateDownloadDialogProps) {
  const { t } = useTranslation('download');
  const [applyToAll, setApplyToAll] = useState(true);
  const previewItems = useMemo(() => review?.duplicates.slice(0, 5) ?? [], [review]);
  const remainingCount = Math.max(0, (review?.duplicates.length ?? 0) - previewItems.length);

  return (
    <Dialog
      open={Boolean(review)}
      onOpenChange={(open) => {
        if (!open) onResolve('cancel', applyToAll);
      }}
    >
      <DialogContent className="max-w-2xl border-white/10 bg-card/95 p-0 shadow-2xl backdrop-blur-xl">
        <div className="border-b border-border/60 px-6 py-5">
          <DialogHeader className="space-y-2">
            <div className="flex items-center gap-3">
              <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500">
                <AlertTriangle className="h-5 w-5" />
              </span>
              <div>
                <DialogTitle>{t('duplicates.title')}</DialogTitle>
                <DialogDescription>
                  {t('duplicates.description', {
                    count: review?.duplicates.length ?? 0,
                    newCount: review?.newCount ?? 0,
                  })}
                </DialogDescription>
              </div>
            </div>
          </DialogHeader>
        </div>

        <div className="max-h-[48vh] space-y-2 overflow-y-auto px-6 py-4">
          {previewItems.map((item) => (
            <div
              key={`${item.duplicate.historyId}:${item.url}`}
              className="rounded-lg border border-border/60 bg-background/45 p-3"
            >
              <div className="flex gap-3">
                {item.thumbnail || item.duplicate.thumbnail ? (
                  <img
                    src={item.thumbnail || item.duplicate.thumbnail || ''}
                    alt=""
                    className="h-14 w-20 shrink-0 rounded-md object-cover"
                  />
                ) : (
                  <div className="flex h-14 w-20 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
                    <FileQuestion className="h-5 w-5" />
                  </div>
                )}
                <div className="min-w-0 flex-1 space-y-1">
                  <p className="truncate text-sm font-medium text-foreground">{item.title}</p>
                  <p className="truncate text-xs text-muted-foreground">
                    {filenameFromPath(item.duplicate.filepath)}
                  </p>
                  <div className="flex flex-wrap items-center gap-2 text-xs">
                    <span className="text-muted-foreground">
                      {t('duplicates.downloadedAt', {
                        date: formatDownloadedAt(item.duplicate.downloadedAt),
                      })}
                    </span>
                    <span
                      className={cn(
                        'inline-flex items-center gap-1 rounded px-1.5 py-0.5',
                        item.duplicate.fileExists
                          ? 'bg-emerald-500/10 text-emerald-500'
                          : 'bg-amber-500/10 text-amber-500',
                      )}
                    >
                      {item.duplicate.fileExists ? (
                        <CheckCircle2 className="h-3 w-3" />
                      ) : (
                        <FolderOpen className="h-3 w-3" />
                      )}
                      {item.duplicate.fileExists
                        ? t('duplicates.fileExists')
                        : t('duplicates.fileMissing')}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          ))}
          {remainingCount > 0 && (
            <div className="rounded-lg border border-dashed border-border/70 px-3 py-2 text-center text-xs text-muted-foreground">
              {t('duplicates.moreItems', { count: remainingCount })}
            </div>
          )}
        </div>

        <div className="border-t border-border/60 px-6 py-4">
          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              checked={applyToAll}
              onChange={(event) => setApplyToAll(event.currentTarget.checked)}
              className="h-4 w-4 rounded border-border accent-primary"
            />
            {t('duplicates.applyToAll')}
          </label>
        </div>

        <DialogFooter className="gap-2 border-t border-border/60 px-6 py-4 sm:space-x-0">
          <Button variant="outline" onClick={() => onResolve('cancel', applyToAll)}>
            {t('duplicates.cancel')}
          </Button>
          <Button
            variant="outline"
            disabled={!applyToAll}
            onClick={() => onResolve('add', applyToAll)}
          >
            <RotateCcw className="mr-2 h-4 w-4" />
            {t('duplicates.addAgain')}
          </Button>
          <Button disabled={!applyToAll} onClick={() => onResolve('skip', applyToAll)}>
            {t('duplicates.skip')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
