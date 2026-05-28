import { BookOpen } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { useDownload } from '@/contexts/DownloadContext';
import type { TelegramStatus } from '@/lib/types';
import { cn } from '@/lib/utils';
import { SettingsCard, SettingsRow, SettingsSection } from '../SettingsSection';

interface RemoteDownloadSectionProps {
  highlightId?: string | null;
}

export function RemoteDownloadSection({ highlightId }: RemoteDownloadSectionProps) {
  const { t } = useTranslation('settings');
  const { settings, updateTelegramSettings, refreshTelegramStatus } = useDownload();
  const [telegramStatus, setTelegramStatus] = useState<TelegramStatus | null>(null);
  const hasTelegramToken = settings.telegramBotToken.trim().length > 0;
  const hasTelegramChatIds = settings.telegramAllowedChatIds.trim().length > 0;

  useEffect(() => {
    if (!settings.telegramEnabled) {
      setTelegramStatus({ state: 'disabled', message: null });
      return;
    }

    if (!hasTelegramToken || !hasTelegramChatIds) {
      setTelegramStatus({
        state: 'error',
        message: hasTelegramToken
          ? t('remoteDownload.telegramErrorChatRequired')
          : t('remoteDownload.telegramErrorTokenRequired'),
      });
      return;
    }

    let cancelled = false;

    const refreshStatus = async () => {
      try {
        const status = await refreshTelegramStatus();
        if (!cancelled) {
          setTelegramStatus(status);
        }
      } catch {
        if (!cancelled) {
          setTelegramStatus({
            state: 'error',
            message: t('remoteDownload.telegramStatusUnavailable'),
          });
        }
      }
    };

    const timer = window.setTimeout(() => {
      void refreshStatus();
    }, 500);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [hasTelegramChatIds, hasTelegramToken, refreshTelegramStatus, settings.telegramEnabled, t]);

  const telegramStatusLabel =
    telegramStatus?.state === 'running'
      ? t('remoteDownload.telegramStatusRunning')
      : telegramStatus?.state === 'error'
        ? t('remoteDownload.telegramStatusError')
        : t('remoteDownload.telegramStatusDisabled');

  return (
    <div className="space-y-8">
      <SettingsSection
        title={t('remoteDownload.telegramRemote')}
        description={t('remoteDownload.telegramRemoteDesc')}
        icon={<i className="fa fa-telegram text-[20px] text-white" aria-hidden="true" />}
        iconClassName="bg-gradient-to-br from-blue-500 to-cyan-600 shadow-blue-500/20"
      >
        <SettingsCard id="telegram-remote" highlight={highlightId === 'telegram-remote'}>
          <SettingsRow
            id="telegram-toggle"
            label={t('remoteDownload.telegramEnable')}
            highlight={highlightId === 'telegram-toggle'}
          >
            <Switch
              checked={settings.telegramEnabled}
              onCheckedChange={(telegramEnabled) => updateTelegramSettings({ telegramEnabled })}
            />
          </SettingsRow>

          <SettingsRow
            id="telegram-status"
            label={t('remoteDownload.telegramStatus')}
            description={telegramStatus?.message || t('remoteDownload.telegramStatusDesc')}
            highlight={highlightId === 'telegram-status'}
          >
            <span
              className={cn(
                'inline-flex rounded px-2 py-1 text-xs font-medium',
                telegramStatus?.state === 'running'
                  ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                  : telegramStatus?.state === 'error'
                    ? 'bg-red-500/10 text-red-600 dark:text-red-400'
                    : 'bg-muted text-muted-foreground',
              )}
            >
              {telegramStatusLabel}
            </span>
          </SettingsRow>

          <SettingsRow
            id="telegram-bot-token"
            label={t('remoteDownload.telegramBotToken')}
            description={t('remoteDownload.telegramBotTokenDesc')}
            highlight={highlightId === 'telegram-bot-token'}
            controlClassName="md:w-[420px]"
          >
            <Input
              type="password"
              value={settings.telegramBotToken}
              onChange={(e) => updateTelegramSettings({ telegramBotToken: e.target.value })}
              placeholder={t('remoteDownload.telegramBotTokenPlaceholder')}
              className="h-9 w-full bg-background"
            />
          </SettingsRow>

          <SettingsRow
            id="telegram-allowed-chat-ids"
            label={t('remoteDownload.telegramAllowedChatIds')}
            description={t('remoteDownload.telegramAllowedChatIdsDesc')}
            highlight={highlightId === 'telegram-allowed-chat-ids'}
            controlClassName="md:w-[420px]"
          >
            <Textarea
              value={settings.telegramAllowedChatIds}
              onChange={(e) => updateTelegramSettings({ telegramAllowedChatIds: e.target.value })}
              placeholder={t('remoteDownload.telegramAllowedChatIdsPlaceholder')}
              className="min-h-20 w-full resize-y bg-background"
            />
          </SettingsRow>

          <SettingsRow
            id="telegram-guide"
            label={t('remoteDownload.telegramGuide')}
            description={t('remoteDownload.telegramGuideDesc')}
            highlight={highlightId === 'telegram-guide'}
          >
            <Dialog>
              <DialogTrigger asChild>
                <Button variant="outline" size="sm" type="button">
                  <BookOpen className="h-4 w-4" />
                  {t('remoteDownload.telegramGuideButton')}
                </Button>
              </DialogTrigger>
              <DialogContent className="max-w-md">
                <DialogHeader>
                  <DialogTitle>{t('remoteDownload.telegramGuideTitle')}</DialogTitle>
                  <DialogDescription>{t('remoteDownload.telegramGuideIntro')}</DialogDescription>
                </DialogHeader>

                <div className="space-y-3">
                  {(['add', 'download', 'help'] as const).map((command) => (
                    <div
                      key={command}
                      className="rounded-md border border-border/60 bg-muted/30 px-3 py-2"
                    >
                      <code className="text-sm font-semibold text-foreground">
                        {t(`remoteDownload.telegramCommand_${command}`)}
                      </code>
                      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                        {t(`remoteDownload.telegramCommand_${command}_desc`)}
                      </p>
                    </div>
                  ))}
                </div>
              </DialogContent>
            </Dialog>
          </SettingsRow>
        </SettingsCard>
      </SettingsSection>
    </div>
  );
}
