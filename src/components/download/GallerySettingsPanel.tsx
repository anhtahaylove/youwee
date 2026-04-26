import { FolderOpen, Settings2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

interface GallerySettings {
  outputPath: string;
  concurrentDownloads: number;
}

interface GallerySettingsPanelProps {
  settings: GallerySettings;
  disabled?: boolean;
  onSelectFolder: () => Promise<void>;
  onConcurrentChange: (concurrent: number) => void;
}

export function GallerySettingsPanel({
  settings,
  disabled,
  onSelectFolder,
  onConcurrentChange,
}: GallerySettingsPanelProps) {
  const { t } = useTranslation('gallery');
  const outputFolderName = settings.outputPath
    ? settings.outputPath.split('/').pop() || settings.outputPath
    : t('settings.notSelected');

  return (
    <div className="flex flex-wrap items-center gap-2">
      <button
        type="button"
        onClick={() => void onSelectFolder()}
        disabled={disabled}
        className="inline-flex items-center gap-2 rounded-lg border border-border/50 bg-card/50 px-3 h-9 text-xs text-foreground transition-colors hover:bg-card"
        title={settings.outputPath || t('settings.selectFolder')}
      >
        <FolderOpen className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="max-w-[180px] truncate">{outputFolderName}</span>
      </button>
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            className="h-9 px-2.5 gap-1.5"
            disabled={disabled}
            title={t('settings.advanced')}
          >
            <Settings2 className="w-3.5 h-3.5" />
            <span className="hidden sm:inline text-xs">{t('settings.more')}</span>
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-72 p-0" align="end" side="bottom" sideOffset={8}>
          <div className="px-4 py-3 border-b bg-muted/30">
            <h4 className="text-sm font-medium">{t('settings.advanced')}</h4>
          </div>
          <div className="p-4 space-y-4">
            <div className="space-y-1.5">
              <Label className="text-[11px] text-muted-foreground">
                {t('settings.parallelDownloads')}
              </Label>
              <Select
                value={String(settings.concurrentDownloads || 1)}
                onValueChange={(value) => onConcurrentChange(Number(value))}
                disabled={disabled}
              >
                <SelectTrigger className="h-8 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {[1, 2, 3, 4, 5].map((n) => (
                    <SelectItem key={n} value={String(n)} className="text-xs">
                      {t('settings.atATime', { count: n })}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
