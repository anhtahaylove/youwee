import { RefreshCw, TerminalSquare } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { LogEntry } from '@/components/logs';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { ScrollArea } from '@/components/ui/scroll-area';
import type { LogEntry as PluginLogEntry, PluginSummary } from '@/lib/types';
import { cn } from '@/lib/utils';

interface PluginLogsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  plugin: PluginSummary | null;
  logs: PluginLogEntry[];
  loading: boolean;
  error: string | null;
  onRefresh: () => Promise<void> | void;
}

export function PluginLogsDialog({
  open,
  onOpenChange,
  plugin,
  logs,
  loading,
  error,
  onRefresh,
}: PluginLogsDialogProps) {
  const { t } = useTranslation('settings');

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-4xl h-[80vh] p-0 gap-0 flex flex-col overflow-hidden">
        <DialogHeader className="border-b border-border/60 px-6 py-5">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <DialogTitle className="flex items-center gap-2 text-base">
                <span className="rounded-xl bg-blue-500/10 p-2 text-blue-500">
                  <TerminalSquare className="h-4 w-4" />
                </span>
                <span className="truncate">
                  {plugin
                    ? t('download.pluginLogsTitle', { name: plugin.manifest.name })
                    : t('download.pluginLogsTitleFallback')}
                </span>
              </DialogTitle>
              <DialogDescription className="mt-2 text-xs sm:text-sm">
                {t('download.pluginLogsDesc')}
              </DialogDescription>
            </div>

            <Button variant="outline" size="sm" onClick={onRefresh} disabled={loading}>
              <RefreshCw className={cn('h-4 w-4', loading && 'animate-spin')} />
              {t('download.pluginLogsRefresh')}
            </Button>
          </div>
        </DialogHeader>

        <div className="border-b border-border/60 px-6 py-3 text-xs text-muted-foreground">
          {plugin && (
            <div className="flex flex-wrap items-center gap-2">
              <span>{plugin.manifest.pluginId}</span>
              <span>•</span>
              <span>{t('download.pluginLogsCount', { count: logs.length })}</span>
            </div>
          )}
        </div>

        <ScrollArea className="flex-1">
          <div className="space-y-3 px-6 py-5">
            {error && (
              <div className="rounded-xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                {error}
              </div>
            )}

            {loading && logs.length === 0 ? (
              <div className="rounded-xl border border-border/60 bg-background/40 px-4 py-6 text-sm text-muted-foreground">
                {t('download.pluginLogsLoading')}
              </div>
            ) : logs.length === 0 ? (
              <div className="rounded-xl border border-dashed border-border/70 bg-background/40 px-4 py-8 text-center">
                <p className="text-sm font-medium">{t('download.pluginLogsEmptyTitle')}</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {t('download.pluginLogsEmptyDesc')}
                </p>
              </div>
            ) : (
              logs.map((log) => <LogEntry key={log.id} log={log} />)
            )}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
}
