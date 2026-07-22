import { invoke } from '@tauri-apps/api/core';
import {
  BookOpen,
  Check,
  ChevronDown,
  Copy,
  Download,
  ExternalLink,
  FolderOpen,
  Puzzle,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { useToast } from '@/components/ui/toast';
import { normalizeAssetPath } from '@/lib/asset-paths';
import { openFileLocation } from '@/lib/open-file-location';
import { cn } from '@/lib/utils';
import { SettingsCard, SettingsRow, SettingsSection } from '../SettingsSection';

interface ExtensionSectionProps {
  highlightId?: string | null;
}

const RELEASES_LATEST_URL = 'https://github.com/anhtahaylove/youwee/releases/latest';
const EXTENSION_DOCS_URL = 'https://youwee.app/docs/browser-extension';
const CHROMIUM_DOWNLOAD_URL =
  'https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Extension-Chromium.zip';
const FIREFOX_AMO_URL = 'https://addons.mozilla.org/firefox/addon/youwee-download-companion/';
const FIREFOX_DOWNLOAD_URL =
  'https://github.com/anhtahaylove/youwee/releases/latest/download/Youwee-Extension-Firefox-signed.xpi';
const CHROMIUM_EXTENSIONS_PAGE = 'chrome://extensions';

const actionButtonClass = cn(
  'h-9 px-3 rounded-md border border-dashed border-border/70',
  'inline-flex items-center gap-1.5 text-sm font-medium',
  'text-muted-foreground hover:text-foreground',
  'hover:border-primary/50 hover:bg-primary/5 transition-colors',
);

export function ExtensionSection({ highlightId }: ExtensionSectionProps) {
  const { t } = useTranslation('settings');
  const toast = useToast();
  const [bundledChromiumPath, setBundledChromiumPath] = useState<string | null>(null);
  const [copiedValue, setCopiedValue] = useState<'path' | 'address' | null>(null);

  useEffect(() => {
    invoke<string | null>('get_bundled_chromium_extension_path')
      .then((path) => setBundledChromiumPath(path ? normalizeAssetPath(path) : null))
      .catch(() => setBundledChromiumPath(null));
  }, []);

  const copyValue = async (value: string, kind: 'path' | 'address') => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedValue(kind);
    } catch (error) {
      toast.error({
        title: t('extension.copyInstallValueError'),
        message: String(error),
      });
    }
  };

  const openBundledChromiumFolder = async () => {
    if (!bundledChromiumPath) return;
    try {
      await openFileLocation(bundledChromiumPath);
    } catch (error) {
      toast.error({
        title: t('extension.openBundledFolderError'),
        message: String(error),
      });
    }
  };

  return (
    <div className="space-y-8">
      <SettingsSection
        title={t('extension.title')}
        description={t('extension.description')}
        icon={<Puzzle className="w-5 h-5 text-white" />}
        iconClassName="bg-gradient-to-br from-indigo-500 to-blue-600 shadow-indigo-500/20"
      >
        <SettingsCard id="extension-download" highlight={highlightId === 'extension-download'}>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">{t('extension.releaseAssetLabel')}</Badge>
            <Badge variant="outline" className="font-mono text-xs">
              {t('extension.latest')}
            </Badge>
          </div>

          <SettingsRow
            id="extension-desktop-required"
            label={t('extension.desktopRequired')}
            description={t('extension.desktopRequiredDesc')}
            highlight={highlightId === 'extension-desktop-required'}
            className="mt-3"
          >
            <a
              href={RELEASES_LATEST_URL}
              target="_blank"
              rel="noopener noreferrer"
              className={actionButtonClass}
            >
              <Download className="w-3.5 h-3.5" />
              <span>{t('extension.openLatestRelease')}</span>
              <ExternalLink className="w-3.5 h-3.5" />
            </a>
          </SettingsRow>

          <SettingsRow
            id="extension-chromium"
            label={t('extension.chromium')}
            description={t(
              bundledChromiumPath ? 'extension.chromiumBundledDesc' : 'extension.chromiumDesc',
            )}
            highlight={highlightId === 'extension-chromium'}
          >
            {bundledChromiumPath ? (
              <div className="flex flex-wrap items-center justify-end gap-2">
                <Badge className="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                  <Check className="mr-1 h-3 w-3" />
                  {t('extension.bundledReady')}
                </Badge>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-9 gap-1.5 border-dashed"
                  onClick={openBundledChromiumFolder}
                >
                  <FolderOpen className="h-3.5 w-3.5" />
                  {t('extension.openBundledFolder')}
                </Button>
              </div>
            ) : (
              <a
                href={CHROMIUM_DOWNLOAD_URL}
                target="_blank"
                rel="noopener noreferrer"
                className={actionButtonClass}
              >
                <Download className="w-3.5 h-3.5" />
                <span>{t('extension.downloadZip')}</span>
                <ExternalLink className="w-3.5 h-3.5" />
              </a>
            )}
          </SettingsRow>

          <SettingsRow
            id="extension-firefox"
            label={t('extension.firefox')}
            description={t('extension.firefoxDesc')}
            highlight={highlightId === 'extension-firefox'}
            controlClassName="md:max-w-md"
          >
            <div className="flex w-full flex-col gap-2 md:items-end">
              <div className="flex flex-wrap items-center gap-2 md:justify-end">
                <Badge className="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                  <Check className="mr-1 h-3 w-3" />
                  {t('extension.recommended')}
                </Badge>
                <Button asChild size="sm" className="h-9 gap-1.5">
                  <a href={FIREFOX_AMO_URL} target="_blank" rel="noopener noreferrer">
                    <ExternalLink className="h-3.5 w-3.5" />
                    <span>{t('extension.installFromAmo')}</span>
                  </a>
                </Button>
              </div>

              <Collapsible className="w-full">
                <CollapsibleTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="group ml-auto h-8 gap-1.5 px-2 text-xs text-muted-foreground"
                  >
                    {t('extension.firefoxAdvanced')}
                    <ChevronDown className="h-3.5 w-3.5 transition-transform group-data-[state=open]:rotate-180" />
                  </Button>
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <div className="mt-2 flex flex-col gap-2 rounded-md bg-muted/40 p-3 sm:flex-row sm:items-center sm:justify-between">
                    <p className="text-xs text-muted-foreground">
                      {t('extension.firefoxAdvancedDesc')}
                    </p>
                    <a
                      href={FIREFOX_DOWNLOAD_URL}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={cn(actionButtonClass, 'h-8 shrink-0 text-xs')}
                    >
                      <Download className="h-3.5 w-3.5" />
                      <span>{t('extension.downloadXpi')}</span>
                      <ExternalLink className="h-3.5 w-3.5" />
                    </a>
                  </div>
                </CollapsibleContent>
              </Collapsible>
            </div>
          </SettingsRow>

          <SettingsRow
            id="extension-guide"
            label={t('extension.guide')}
            description={t('extension.guideDesc')}
            highlight={highlightId === 'extension-guide'}
          >
            <a
              href={EXTENSION_DOCS_URL}
              target="_blank"
              rel="noopener noreferrer"
              className={actionButtonClass}
            >
              <BookOpen className="w-3.5 h-3.5" />
              <span>{t('extension.openGuide')}</span>
              <ExternalLink className="w-3.5 h-3.5" />
            </a>
          </SettingsRow>
        </SettingsCard>

        <SettingsCard id="extension-install" highlight={highlightId === 'extension-install'}>
          <p className="text-sm font-medium">{t('extension.installSteps')}</p>
          {bundledChromiumPath && (
            <div className="mt-3 flex flex-col gap-3 rounded-xl bg-primary/5 p-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0">
                <p className="flex items-center gap-1.5 text-sm font-medium text-foreground">
                  <Check className="h-4 w-4 text-primary" />
                  {t('extension.bundledReady')}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {t('extension.chromiumBundledDesc')}
                </p>
                <p
                  className="mt-1 truncate font-mono text-xs text-muted-foreground"
                  title={bundledChromiumPath}
                >
                  {bundledChromiumPath}
                </p>
              </div>
              <div className="flex shrink-0 flex-wrap gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 gap-1.5 border-dashed text-xs"
                  onClick={openBundledChromiumFolder}
                >
                  <FolderOpen className="h-3.5 w-3.5" />
                  {t('extension.openBundledFolder')}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 gap-1.5 border-dashed text-xs"
                  onClick={() => copyValue(bundledChromiumPath, 'path')}
                >
                  {copiedValue === 'path' ? (
                    <Check className="h-3.5 w-3.5" />
                  ) : (
                    <Copy className="h-3.5 w-3.5" />
                  )}
                  {copiedValue === 'path' ? t('about.copied') : t('extension.copyFolderPath')}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 gap-1.5 border-dashed text-xs"
                  onClick={() => copyValue(CHROMIUM_EXTENSIONS_PAGE, 'address')}
                >
                  {copiedValue === 'address' ? (
                    <Check className="h-3.5 w-3.5" />
                  ) : (
                    <Copy className="h-3.5 w-3.5" />
                  )}
                  {copiedValue === 'address'
                    ? t('about.copied')
                    : t('extension.copyExtensionsAddress')}
                </Button>
              </div>
            </div>
          )}
          <div className="mt-3 grid gap-4 lg:grid-cols-2">
            <div className="rounded-xl bg-background/60 p-3">
              <p className="text-sm font-medium text-foreground">{t('extension.chromiumSteps')}</p>
              <ol className="mt-2 list-decimal space-y-1 pl-4 text-xs text-muted-foreground">
                <li>
                  {t(
                    bundledChromiumPath
                      ? 'extension.chromiumBundledStep1'
                      : 'extension.chromiumStep1',
                  )}
                </li>
                <li>
                  {t(
                    bundledChromiumPath
                      ? 'extension.chromiumBundledStep2'
                      : 'extension.chromiumStep2',
                  )}
                </li>
                <li>{t('extension.chromiumStep3')}</li>
                <li>{t('extension.chromiumStep4')}</li>
              </ol>
            </div>

            <div className="rounded-xl bg-background/60 p-3">
              <p className="text-sm font-medium text-foreground">{t('extension.firefoxSteps')}</p>
              <ol className="mt-2 list-decimal space-y-1 pl-4 text-xs text-muted-foreground">
                <li>{t('extension.firefoxStep1')}</li>
                <li>{t('extension.firefoxStep2')}</li>
                <li>{t('extension.firefoxStep3')}</li>
              </ol>
            </div>
          </div>
        </SettingsCard>
      </SettingsSection>
    </div>
  );
}
