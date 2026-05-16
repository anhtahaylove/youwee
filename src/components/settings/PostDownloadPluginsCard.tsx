import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import {
  Braces,
  Download,
  FolderOpen,
  Link2,
  PackageOpen,
  Plus,
  RefreshCw,
  ShieldCheck,
  TerminalSquare,
} from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PluginLogsDialog } from '@/components/settings/PluginLogsDialog';
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
import { localizeUnknownError } from '@/lib/backend-error';
import { saveEnabledPostDownloadPlugins } from '@/lib/post-download-plugins';
import type {
  LogEntry,
  PluginCompatibilitySpec,
  PluginExecutionStatusEvent,
  PluginPackageInspection,
  PluginPermissionApproval,
  PluginProvider,
  PluginRuntimeLanguage,
  PluginSummary,
  RuntimeProviderStatus,
} from '@/lib/types';
import { cn } from '@/lib/utils';
import { SettingsCard } from './SettingsSection';

type InstallPluginSourceInput = {
  kind: 'app-scaffold' | 'local-folder' | 'local-zip' | 'remote-url';
  value: string;
};

const PROVIDER_LABELS: Record<PluginProvider, string> = {
  deno: 'Deno',
  node: 'Node',
  bun: 'Bun',
  python: 'Python',
};

const LANGUAGE_LABELS: Record<PluginRuntimeLanguage, string> = {
  javascript: 'JavaScript',
  python: 'Python',
};

function summarizeRequestedPermissions(
  plugin: PluginSummary | PluginPackageInspection,
  t: (key: string, opts?: Record<string, unknown>) => string,
) {
  const permissions = plugin.manifest.permissions;
  const entries: string[] = [];
  if (permissions.network) entries.push(t('download.pluginPermissionNetwork'));
  if (permissions.readPaths.length > 0) {
    entries.push(
      t('download.pluginPermissionReadPathsCount', { count: permissions.readPaths.length }),
    );
  }
  if (permissions.writePaths.length > 0) {
    entries.push(
      t('download.pluginPermissionWritePathsCount', {
        count: permissions.writePaths.length,
      }),
    );
  }
  if (permissions.env.length > 0) {
    entries.push(t('download.pluginPermissionEnvCount', { count: permissions.env.length }));
  }
  return entries;
}

function currentProvider(plugin: PluginSummary) {
  return (
    plugin.installation.selectedProvider ??
    plugin.manifest.runtime.preferredProvider ??
    plugin.manifest.runtime.supportedProviders[0]
  );
}

function summarizeCompatibility(
  compatibility: PluginCompatibilitySpec | null | undefined,
  t: (key: string, opts?: Record<string, unknown>) => string,
) {
  const entries: string[] = [];
  if (compatibility?.appVersion) {
    entries.push(`${t('download.pluginCompatibilityApp')}: ${compatibility.appVersion}`);
  }
  if (compatibility?.sdkVersion) {
    entries.push(`${t('download.pluginCompatibilitySdk')}: ${compatibility.sdkVersion}`);
  }
  return entries;
}

export function PostDownloadPluginsCard() {
  const { t } = useTranslation('settings');
  const [plugins, setPlugins] = useState<PluginSummary[]>([]);
  const [providers, setProviders] = useState<RuntimeProviderStatus[]>([]);
  const [defaultProviders, setDefaultProviders] = useState<
    Partial<Record<PluginRuntimeLanguage, PluginProvider>>
  >({});
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [inspecting, setInspecting] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [newPluginName, setNewPluginName] = useState('');
  const [newPluginSlug, setNewPluginSlug] = useState('');
  const [importUrl, setImportUrl] = useState('');
  const [inspection, setInspection] = useState<PluginPackageInspection | null>(null);
  const [installSource, setInstallSource] = useState<InstallPluginSourceInput | null>(null);
  const [envDrafts, setEnvDrafts] = useState<Record<string, string>>({});
  const [runtimeStatuses, setRuntimeStatuses] = useState<
    Record<string, { status: string; message?: string | null }>
  >({});
  const [logsOpen, setLogsOpen] = useState(false);
  const [logsLoading, setLogsLoading] = useState(false);
  const [pluginLogs, setPluginLogs] = useState<LogEntry[]>([]);
  const [selectedPluginId, setSelectedPluginId] = useState<string | null>(null);
  const [pluginLogsError, setPluginLogsError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPlugins = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [pluginResult, providerResult] = await Promise.all([
        invoke<PluginSummary[]>('list_plugins'),
        invoke<RuntimeProviderStatus[]>('list_runtime_providers'),
      ]);
      setPlugins(pluginResult);
      setProviders(providerResult);

      const defaults: Partial<Record<PluginRuntimeLanguage, PluginProvider>> = {};
      const statuses: Record<string, { status: string; message?: string | null }> = {};
      for (const plugin of pluginResult) {
        const language = plugin.manifest.runtime.language;
        if (!defaults[language]) {
          defaults[language] = currentProvider(plugin);
        }
        if (plugin.installation.lastExecutionStatus || plugin.installation.lastError) {
          statuses[plugin.manifest.pluginId] = {
            status: plugin.installation.lastExecutionStatus ?? 'idle',
            message: plugin.installation.lastError,
          };
        }
      }
      setDefaultProviders(defaults);
      setRuntimeStatuses(statuses);
      saveEnabledPostDownloadPlugins(
        pluginResult
          .filter((plugin) => plugin.installation.enabled)
          .map((plugin) => plugin.manifest.pluginId),
      );
    } catch (err) {
      console.error('Failed to load plugins:', err);
      setError(t('download.pluginLoadError'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  useEffect(() => {
    let isMounted = true;

    const setup = async () => {
      const unlisten = await listen<PluginExecutionStatusEvent>(
        'plugin-execution-status',
        (event) => {
          if (!isMounted) return;
          setRuntimeStatuses((current) => ({
            ...current,
            [event.payload.pluginId]: {
              status: event.payload.status,
              message: event.payload.message,
            },
          }));
        },
      );

      if (!isMounted) {
        unlisten();
      }

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setup().then((unlisten) => {
      cleanup = unlisten;
    });

    return () => {
      isMounted = false;
      cleanup?.();
    };
  }, []);

  const updatePluginList = (updater: (items: PluginSummary[]) => PluginSummary[]) => {
    setPlugins((current) => {
      const next = updater(current);
      saveEnabledPostDownloadPlugins(
        next
          .filter((plugin) => plugin.installation.enabled)
          .map((plugin) => plugin.manifest.pluginId),
      );
      return next;
    });
  };

  const inspectSource = async (source: InstallPluginSourceInput, command: string, key: string) => {
    setInspecting(true);
    setError(null);
    try {
      const result = await invoke<PluginPackageInspection>(command, { [key]: source.value });
      setInspection(result);
      setInstallSource(source);
    } catch (err) {
      console.error('Failed to inspect plugin package:', err);
      setError(localizeUnknownError(err));
      setInspection(null);
      setInstallSource(null);
    } finally {
      setInspecting(false);
    }
  };

  const handleImportFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t('download.pluginImportFolder'),
    });
    if (typeof selected !== 'string') return;
    await inspectSource({ kind: 'local-folder', value: selected }, 'inspect_plugin_folder', 'path');
  };

  const handleImportZip = async () => {
    const selected = await open({
      directory: false,
      multiple: false,
      title: t('download.pluginImportZip'),
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    });
    if (typeof selected !== 'string') return;
    await inspectSource({ kind: 'local-zip', value: selected }, 'inspect_plugin_zip', 'path');
  };

  const handleInspectUrl = async () => {
    const trimmed = importUrl.trim();
    if (!trimmed) return;
    await inspectSource({ kind: 'remote-url', value: trimmed }, 'inspect_plugin_url', 'url');
  };

  const handleInstallInspection = async () => {
    if (!inspection || !installSource) return;
    setInstalling(true);
    setError(null);
    try {
      await invoke<PluginSummary>('install_plugin', {
        source: installSource,
        trusted: false,
      });
      setInspection(null);
      setInstallSource(null);
      setImportUrl('');
      await loadPlugins();
    } catch (err) {
      console.error('Failed to install plugin:', err);
      setError(localizeUnknownError(err));
    } finally {
      setInstalling(false);
    }
  };

  const handleTogglePlugin = async (plugin: PluginSummary, enabled: boolean) => {
    try {
      await invoke('update_plugin_state', { pluginId: plugin.manifest.pluginId, enabled });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.pluginId === plugin.manifest.pluginId
            ? {
                ...item,
                installation: { ...item.installation, enabled },
              }
            : item,
        ),
      );
    } catch (err) {
      console.error('Failed to update plugin state:', err);
      setError(t('download.pluginStateError'));
    }
  };

  const handleSetTrust = async (plugin: PluginSummary, trusted: boolean) => {
    try {
      await invoke('set_plugin_trust', { pluginId: plugin.manifest.pluginId, trusted });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.pluginId === plugin.manifest.pluginId
            ? {
                ...item,
                installation: {
                  ...item.installation,
                  trusted,
                  enabled: trusted ? item.installation.enabled : false,
                },
              }
            : item,
        ),
      );
    } catch (err) {
      console.error('Failed to update plugin trust:', err);
      setError(t('download.pluginTrustError'));
    }
  };

  const handleApprovePermissions = async (
    plugin: PluginSummary,
    permissions: PluginPermissionApproval,
  ) => {
    try {
      await invoke('approve_plugin_permissions', {
        pluginId: plugin.manifest.pluginId,
        permissions,
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.pluginId === plugin.manifest.pluginId
            ? {
                ...item,
                installation: { ...item.installation, approvedPermissions: permissions },
              }
            : item,
        ),
      );
    } catch (err) {
      console.error('Failed to approve plugin permissions:', err);
      setError(t('download.pluginPermissionError'));
    }
  };

  const handleCreatePlugin = async () => {
    const trimmedName = newPluginName.trim();
    if (!trimmedName) return;

    setCreating(true);
    setError(null);
    try {
      await invoke<PluginSummary>('create_plugin_scaffold', {
        input: {
          name: trimmedName,
          slug: newPluginSlug.trim() || null,
        },
      });
      setNewPluginName('');
      setNewPluginSlug('');
      await loadPlugins();
    } catch (err) {
      console.error('Failed to create plugin:', err);
      setError(t('download.pluginCreateError'));
    } finally {
      setCreating(false);
    }
  };

  const handleOpenPluginDirectory = async (pluginId: string) => {
    try {
      await invoke('open_plugin_directory', { pluginId });
    } catch (err) {
      console.error('Failed to open plugin directory:', err);
      setError(t('download.pluginOpenDirError'));
    }
  };

  const handleSetPluginProvider = async (plugin: PluginSummary, provider: PluginProvider) => {
    try {
      await invoke('set_plugin_provider', { pluginId: plugin.manifest.pluginId, provider });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.pluginId === plugin.manifest.pluginId
            ? {
                ...item,
                installation: { ...item.installation, selectedProvider: provider },
              }
            : item,
        ),
      );
    } catch (err) {
      console.error('Failed to set plugin provider:', err);
      setError(t('download.pluginProviderError'));
    }
  };

  const setEnvDraftValue = (pluginId: string, key: string, value: string) => {
    setEnvDrafts((current) => ({
      ...current,
      [`${pluginId}:${key}`]: value,
    }));
  };

  const getEnvDraftValue = (pluginId: string, key: string) => envDrafts[`${pluginId}:${key}`] ?? '';

  const handleSavePluginEnv = async (plugin: PluginSummary, key: string) => {
    const value = getEnvDraftValue(plugin.manifest.pluginId, key);
    try {
      await invoke('update_plugin_env_values', {
        pluginId: plugin.manifest.pluginId,
        input: {
          values: {
            [key]: value.trim() ? value : null,
          },
        },
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.pluginId === plugin.manifest.pluginId
            ? {
                ...item,
                installation: {
                  ...item.installation,
                  envValueStatus: {
                    ...item.installation.envValueStatus,
                    [key]: value.trim().length > 0,
                  },
                },
              }
            : item,
        ),
      );
      setEnvDraftValue(plugin.manifest.pluginId, key, '');
    } catch (err) {
      console.error('Failed to update plugin env values:', err);
      setError(t('download.pluginEnvSaveError'));
    }
  };

  const handleClearPluginEnv = async (plugin: PluginSummary, key: string) => {
    try {
      await invoke('update_plugin_env_values', {
        pluginId: plugin.manifest.pluginId,
        input: {
          values: {
            [key]: null,
          },
        },
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.pluginId === plugin.manifest.pluginId
            ? {
                ...item,
                installation: {
                  ...item.installation,
                  envValueStatus: {
                    ...item.installation.envValueStatus,
                    [key]: false,
                  },
                },
              }
            : item,
        ),
      );
      setEnvDraftValue(plugin.manifest.pluginId, key, '');
    } catch (err) {
      console.error('Failed to clear plugin env value:', err);
      setError(t('download.pluginEnvSaveError'));
    }
  };

  const handleSetDefaultProvider = async (
    language: PluginRuntimeLanguage,
    provider: PluginProvider,
  ) => {
    try {
      await invoke('set_default_provider_for_language', { language, provider });
      setDefaultProviders((current) => ({ ...current, [language]: provider }));
    } catch (err) {
      console.error('Failed to set default provider:', err);
      setError(t('download.pluginProviderDefaultError'));
    }
  };

  const selectedPlugin =
    selectedPluginId != null
      ? (plugins.find((plugin) => plugin.manifest.pluginId === selectedPluginId) ?? null)
      : null;
  const inspectionCompatibilityEntries = inspection
    ? summarizeCompatibility(inspection.manifest.compatibility, t)
    : [];

  const loadPluginLogs = useCallback(
    async (pluginId: string) => {
      setLogsLoading(true);
      setPluginLogsError(null);
      try {
        const result = await invoke<LogEntry[]>('get_plugin_logs', {
          pluginId,
          limit: 120,
        });
        setPluginLogs(result);
      } catch (err) {
        console.error('Failed to load plugin logs:', err);
        setPluginLogsError(t('download.pluginLogsLoadError'));
      } finally {
        setLogsLoading(false);
      }
    },
    [t],
  );

  const handleOpenPluginLogs = async (pluginId: string) => {
    setSelectedPluginId(pluginId);
    setLogsOpen(true);
    await loadPluginLogs(pluginId);
  };

  return (
    <>
      <SettingsCard className="space-y-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <div className="rounded-xl bg-purple-500/10 p-2 text-purple-500">
                <Braces className="h-4 w-4" />
              </div>
              <div>
                <p className="text-sm font-medium">{t('download.pluginsTitle')}</p>
                <p className="text-xs text-muted-foreground">{t('download.pluginsDesc')}</p>
              </div>
            </div>
          </div>
          <Button variant="outline" size="sm" onClick={loadPlugins} disabled={loading}>
            <RefreshCw className={cn('h-4 w-4', loading && 'animate-spin')} />
            {t('download.pluginReload')}
          </Button>
        </div>

        <div className="grid gap-3 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
          <div className="rounded-xl border border-border/60 bg-background/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium">
              <Plus className="h-4 w-4" />
              <span>{t('download.pluginCreate')}</span>
            </div>
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              <Input
                value={newPluginName}
                onChange={(event) => setNewPluginName(event.target.value)}
                placeholder={t('download.pluginNamePlaceholder')}
              />
              <Input
                value={newPluginSlug}
                onChange={(event) => setNewPluginSlug(event.target.value)}
                placeholder={t('download.pluginSlugPlaceholder')}
              />
            </div>
            <p className="mt-2 text-xs text-muted-foreground">{t('download.pluginCreateHelp')}</p>
            <div className="mt-3">
              <Button onClick={handleCreatePlugin} disabled={creating || !newPluginName.trim()}>
                <Plus className="h-4 w-4" />
                {creating ? t('download.pluginCreating') : t('download.pluginCreate')}
              </Button>
            </div>
          </div>

          <div className="rounded-xl border border-border/60 bg-background/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium">
              <PackageOpen className="h-4 w-4" />
              <span>{t('download.pluginImportTitle')}</span>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <Button variant="outline" size="sm" onClick={handleImportFolder}>
                <FolderOpen className="h-4 w-4" />
                {t('download.pluginImportFolder')}
              </Button>
              <Button variant="outline" size="sm" onClick={handleImportZip}>
                <PackageOpen className="h-4 w-4" />
                {t('download.pluginImportZip')}
              </Button>
            </div>
            <div className="mt-3 flex flex-col gap-2 sm:flex-row">
              <Input
                value={importUrl}
                onChange={(event) => setImportUrl(event.target.value)}
                placeholder={t('download.pluginImportUrlPlaceholder')}
              />
              <Button
                variant="outline"
                onClick={handleInspectUrl}
                disabled={inspecting || !importUrl.trim()}
              >
                <Link2 className="h-4 w-4" />
                {t('download.pluginInspect')}
              </Button>
            </div>
            <p className="mt-2 text-xs text-muted-foreground">{t('download.pluginImportHelp')}</p>
          </div>
        </div>

        {providers.length > 0 && (
          <div className="rounded-xl border border-border/60 bg-background/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium">
              <Download className="h-4 w-4" />
              <span>{t('download.pluginRuntimeTitle')}</span>
            </div>
            <p className="mt-1 text-xs text-muted-foreground">{t('download.pluginRuntimeDesc')}</p>

            <div className="mt-3 grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
              {(['javascript', 'python'] as PluginRuntimeLanguage[]).map((language) => {
                const allowedProviders = providers.filter((provider) =>
                  language === 'javascript'
                    ? provider.provider === 'deno' ||
                      provider.provider === 'node' ||
                      provider.provider === 'bun'
                    : provider.provider === 'python',
                );
                if (allowedProviders.length === 0) return null;
                return (
                  <div key={language} className="rounded-lg border border-border/60 px-3 py-3">
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <p className="text-xs font-medium">{LANGUAGE_LABELS[language]}</p>
                        <p className="text-[11px] text-muted-foreground">
                          {t('download.pluginRuntimeDefault')}
                        </p>
                      </div>
                      <Select
                        value={defaultProviders[language] ?? allowedProviders[0].provider}
                        onValueChange={(value) =>
                          handleSetDefaultProvider(language, value as PluginProvider)
                        }
                      >
                        <SelectTrigger className="h-8 w-[140px] text-xs">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {allowedProviders.map((provider) => (
                            <SelectItem
                              key={`${language}-${provider.provider}`}
                              value={provider.provider}
                              className="text-xs"
                            >
                              {PROVIDER_LABELS[provider.provider]}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="mt-3 flex flex-wrap gap-2">
              {providers.map((provider) => (
                <span
                  key={provider.provider}
                  className={cn(
                    'rounded px-2 py-1 text-[11px]',
                    provider.available
                      ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400'
                      : 'bg-amber-500/10 text-amber-600 dark:text-amber-400',
                  )}
                >
                  {PROVIDER_LABELS[provider.provider]}:{' '}
                  {provider.available
                    ? t('download.pluginRuntimeAvailable')
                    : t('download.pluginRuntimeMissing')}
                  {provider.resolvedSource ? ` (${provider.resolvedSource})` : ''}
                </span>
              ))}
            </div>
          </div>
        )}

        {error && <p className="text-xs text-destructive">{error}</p>}

        {inspection && installSource && (
          <div className="rounded-xl border border-dashed border-amber-500/40 bg-amber-500/5 p-4">
            <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="text-sm font-semibold">{inspection.manifest.name}</p>
                  <span className="rounded bg-muted px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">
                    v{inspection.manifest.version}
                  </span>
                  <span className="rounded bg-blue-500/10 px-2 py-0.5 text-[10px] uppercase tracking-wide text-blue-600 dark:text-blue-400">
                    {LANGUAGE_LABELS[inspection.manifest.runtime.language]}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground">
                  {inspection.manifest.description || t('download.pluginNoDescription')}
                </p>
                <div className="flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                  <span>{inspection.manifest.pluginId}</span>
                  <span>•</span>
                  <span>{inspection.source.kind}</span>
                  <span>•</span>
                  <span>{inspection.source.value}</span>
                </div>
                <div className="space-y-1 text-[11px] text-muted-foreground">
                  <p className="font-medium text-foreground/80">
                    {t('download.pluginCompatibilityTitle')}
                  </p>
                  {inspectionCompatibilityEntries.length > 0 ? (
                    inspectionCompatibilityEntries.map((entry) => <p key={entry}>{entry}</p>)
                  ) : (
                    <p>{t('download.pluginCompatibilityNone')}</p>
                  )}
                </div>
                {inspection.warnings.length > 0 && (
                  <div className="flex flex-wrap gap-2">
                    {inspection.warnings.map((warning) => (
                      <span
                        key={warning}
                        className="rounded bg-amber-500/10 px-2 py-1 text-[11px] text-amber-600 dark:text-amber-400"
                      >
                        {warning}
                      </span>
                    ))}
                  </div>
                )}
              </div>
              <div className="flex gap-2">
                <Button variant="outline" onClick={() => setInspection(null)} disabled={installing}>
                  {t('download.pluginDismiss')}
                </Button>
                <Button onClick={handleInstallInspection} disabled={installing}>
                  <Download className="h-4 w-4" />
                  {installing ? t('download.pluginInstalling') : t('download.pluginInstall')}
                </Button>
              </div>
            </div>
          </div>
        )}

        {loading ? (
          <p className="text-sm text-muted-foreground">{t('download.pluginLoading')}</p>
        ) : plugins.length === 0 ? (
          <div className="rounded-xl border border-dashed border-border/70 bg-background/40 px-4 py-6 text-center">
            <p className="text-sm font-medium">{t('download.pluginEmptyTitle')}</p>
            <p className="mt-1 text-xs text-muted-foreground">{t('download.pluginEmptyDesc')}</p>
            <div className="mt-3">
              <Button variant="outline" size="sm" onClick={loadPlugins}>
                <RefreshCw className="h-4 w-4" />
                {t('download.pluginReload')}
              </Button>
            </div>
          </div>
        ) : (
          <div className="space-y-3">
            {plugins.map((plugin) => {
              const requestedPermissions = summarizeRequestedPermissions(plugin, t);
              const compatibilityEntries = summarizeCompatibility(plugin.manifest.compatibility, t);
              const selectedProvider = currentProvider(plugin);
              const supportedProviders = plugin.manifest.runtime.supportedProviders;
              const runtimeStatus = runtimeStatuses[plugin.manifest.pluginId];
              return (
                <div
                  key={plugin.manifest.pluginId}
                  className="rounded-xl border border-border/60 bg-background/60 p-4"
                >
                  <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div className="space-y-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-semibold">{plugin.manifest.name}</p>
                        <span className="rounded bg-muted px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">
                          v{plugin.manifest.version}
                        </span>
                        <span className="rounded bg-blue-500/10 px-2 py-0.5 text-[10px] uppercase tracking-wide text-blue-600 dark:text-blue-400">
                          {LANGUAGE_LABELS[plugin.manifest.runtime.language]}
                        </span>
                        <span
                          className={cn(
                            'rounded px-2 py-0.5 text-[10px] uppercase tracking-wide',
                            plugin.installation.trusted
                              ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                              : 'bg-amber-500/10 text-amber-600 dark:text-amber-400',
                          )}
                        >
                          {plugin.installation.trusted
                            ? t('download.pluginTrusted')
                            : t('download.pluginUntrusted')}
                        </span>
                        {runtimeStatus?.status && (
                          <span
                            className={cn(
                              'rounded px-2 py-0.5 text-[10px] uppercase tracking-wide',
                              runtimeStatus.status === 'running' &&
                                'bg-sky-500/10 text-sky-600 dark:text-sky-400',
                              runtimeStatus.status === 'success' &&
                                'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
                              runtimeStatus.status === 'error' &&
                                'bg-red-500/10 text-red-600 dark:text-red-400',
                            )}
                          >
                            {runtimeStatus.status === 'running'
                              ? t('download.pluginStatusRunning')
                              : runtimeStatus.status === 'success'
                                ? t('download.pluginStatusSuccess')
                                : runtimeStatus.status === 'error'
                                  ? t('download.pluginStatusError')
                                  : runtimeStatus.status}
                          </span>
                        )}
                      </div>

                      <p className="text-xs text-muted-foreground">
                        {plugin.manifest.description || t('download.pluginNoDescription')}
                      </p>

                      <div className="flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                        <span>{plugin.manifest.pluginId}</span>
                        <span>•</span>
                        <span>{plugin.manifest.slug}</span>
                        <span>•</span>
                        <span>{plugin.manifest.runtime.entrypoint}</span>
                        <span>•</span>
                        <span>
                          {t('download.pluginTimeout', { seconds: plugin.manifest.timeoutSec })}
                        </span>
                      </div>

                      <div className="flex flex-wrap gap-2">
                        {supportedProviders.map((provider) => (
                          <span
                            key={`${plugin.manifest.pluginId}-${provider}`}
                            className="rounded bg-muted px-2 py-1 text-[11px] text-muted-foreground"
                          >
                            {PROVIDER_LABELS[provider]}
                          </span>
                        ))}
                        {plugin.warnings.map((warning) => (
                          <span
                            key={`${plugin.manifest.pluginId}-${warning}`}
                            className="rounded bg-amber-500/10 px-2 py-1 text-[11px] text-amber-600 dark:text-amber-400"
                          >
                            {warning}
                          </span>
                        ))}
                      </div>
                    </div>

                    <div className="flex flex-col gap-2 lg:items-end">
                      <div className="flex flex-wrap gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleOpenPluginLogs(plugin.manifest.pluginId)}
                        >
                          <TerminalSquare className="h-4 w-4" />
                          {t('download.pluginViewLogs')}
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleOpenPluginDirectory(plugin.manifest.pluginId)}
                        >
                          <FolderOpen className="h-4 w-4" />
                          {t('download.pluginOpenFolder')}
                        </Button>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">
                          {plugin.installation.enabled
                            ? t('download.pluginEnabled')
                            : t('download.pluginDisabled')}
                        </span>
                        <Switch
                          checked={plugin.installation.enabled}
                          onCheckedChange={(enabled) => handleTogglePlugin(plugin, enabled)}
                        />
                      </div>
                    </div>
                  </div>

                  <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                    <div className="rounded-xl bg-muted/30 p-3">
                      <div className="flex items-center gap-2 text-xs font-medium">
                        <ShieldCheck className="h-4 w-4 text-amber-500" />
                        <span>{t('download.pluginTrustTitle')}</span>
                      </div>
                      <p className="mt-2 text-[11px] text-muted-foreground">
                        {t('download.pluginTrustDesc')}
                      </p>
                      <div className="mt-3 flex items-center justify-between rounded-lg border border-border/60 px-3 py-2">
                        <span className="text-xs">{t('download.pluginTrustToggle')}</span>
                        <Switch
                          checked={plugin.installation.trusted}
                          onCheckedChange={(trusted) => handleSetTrust(plugin, trusted)}
                        />
                      </div>
                      <div className="mt-3 space-y-1 text-[11px] text-muted-foreground">
                        <p>
                          {t('download.pluginSourceLabel')}: {plugin.installation.source.kind}
                        </p>
                        <p className="break-all">
                          {t('download.pluginSourceValueLabel')}: {plugin.installation.source.value}
                        </p>
                        {plugin.installation.source.checksum && (
                          <p className="break-all">
                            {t('download.pluginChecksumLabel')}:{' '}
                            {plugin.installation.source.checksum}
                          </p>
                        )}
                      </div>
                    </div>

                    <div className="rounded-xl bg-muted/30 p-3">
                      <div className="flex items-center gap-2 text-xs font-medium">
                        <PackageOpen className="h-4 w-4 text-blue-500" />
                        <span>{t('download.pluginProviderTitle')}</span>
                      </div>
                      <p className="mt-2 text-[11px] text-muted-foreground">
                        {t('download.pluginProviderDesc')}
                      </p>
                      <div className="mt-3">
                        <Select
                          value={selectedProvider}
                          onValueChange={(value) =>
                            handleSetPluginProvider(plugin, value as PluginProvider)
                          }
                        >
                          <SelectTrigger className="h-8 text-xs">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {supportedProviders.map((provider) => (
                              <SelectItem key={provider} value={provider} className="text-xs">
                                {PROVIDER_LABELS[provider]}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                      <div className="mt-3 space-y-1 text-[11px] text-muted-foreground">
                        {plugin.installation.lastResolvedProvider && (
                          <p>
                            {t('download.pluginLastResolvedProvider')}:{' '}
                            {PROVIDER_LABELS[plugin.installation.lastResolvedProvider]}
                          </p>
                        )}
                        {plugin.installation.lastResolvedSource && (
                          <p>
                            {t('download.pluginLastResolvedSource')}:{' '}
                            {plugin.installation.lastResolvedSource}
                          </p>
                        )}
                        {plugin.installation.lastExecutionStatus && (
                          <p>
                            {t('download.pluginLastExecutionStatus')}:{' '}
                            {plugin.installation.lastExecutionStatus}
                          </p>
                        )}
                        {(runtimeStatus?.status === 'error' || plugin.installation.lastError) && (
                          <p className="text-destructive">
                            {t('download.pluginLastError')}:{' '}
                            {runtimeStatus?.status === 'error'
                              ? runtimeStatus.message
                              : plugin.installation.lastError}
                          </p>
                        )}
                        {runtimeStatus?.status === 'running' && (
                          <p className="text-sky-600 dark:text-sky-400">
                            {t('download.pluginRunningNow')}
                          </p>
                        )}
                      </div>
                    </div>
                  </div>

                  <div className="mt-4 rounded-xl bg-muted/30 p-3">
                    <div className="flex items-center gap-2 text-xs font-medium">
                      <Braces className="h-4 w-4 text-purple-500" />
                      <span>{t('download.pluginCompatibilityTitle')}</span>
                    </div>
                    <p className="mt-2 text-[11px] text-muted-foreground">
                      {t('download.pluginCompatibilityDesc')}
                    </p>
                    <div className="mt-3 space-y-1 text-[11px] text-muted-foreground">
                      {compatibilityEntries.length > 0 ? (
                        compatibilityEntries.map((entry) => <p key={entry}>{entry}</p>)
                      ) : (
                        <p>{t('download.pluginCompatibilityNone')}</p>
                      )}
                    </div>
                  </div>

                  <div className="mt-4 rounded-xl bg-muted/30 p-3">
                    <div className="flex items-center gap-2 text-xs font-medium">
                      <ShieldCheck className="h-4 w-4 text-amber-500" />
                      <span>{t('download.pluginPermissionsTitle')}</span>
                    </div>

                    {requestedPermissions.length === 0 ? (
                      <p className="mt-2 text-xs text-muted-foreground">
                        {t('download.pluginNoExtraPermissions')}
                      </p>
                    ) : (
                      <div className="mt-3 space-y-2">
                        <p className="text-xs text-muted-foreground">
                          {t('download.pluginRequestedPermissions')}
                        </p>
                        <div className="flex flex-wrap gap-2">
                          {requestedPermissions.map((permission) => (
                            <span
                              key={permission}
                              className="rounded bg-amber-500/10 px-2 py-1 text-[11px] text-amber-600 dark:text-amber-400"
                            >
                              {permission}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    <div className="mt-3 grid gap-2 md:grid-cols-2">
                      {[
                        {
                          key: 'network' as const,
                          label: t('download.pluginPermissionNetwork'),
                          enabled: plugin.manifest.permissions.network,
                          approved: plugin.installation.approvedPermissions.network,
                        },
                        {
                          key: 'readPaths' as const,
                          label: t('download.pluginPermissionReadPaths'),
                          enabled: plugin.manifest.permissions.readPaths.length > 0,
                          approved: plugin.installation.approvedPermissions.readPaths,
                        },
                        {
                          key: 'writePaths' as const,
                          label: t('download.pluginPermissionWritePaths'),
                          enabled: plugin.manifest.permissions.writePaths.length > 0,
                          approved: plugin.installation.approvedPermissions.writePaths,
                        },
                        {
                          key: 'env' as const,
                          label: t('download.pluginPermissionEnv'),
                          enabled: plugin.manifest.permissions.env.length > 0,
                          approved: plugin.installation.approvedPermissions.env,
                        },
                      ].map((permission) => (
                        <div
                          key={permission.key}
                          className={cn(
                            'flex items-center justify-between rounded-lg border border-border/60 px-3 py-2',
                            !permission.enabled && 'opacity-50',
                          )}
                        >
                          <span className="text-xs">{permission.label}</span>
                          <Switch
                            checked={permission.enabled && permission.approved}
                            disabled={!permission.enabled}
                            onCheckedChange={(checked) =>
                              handleApprovePermissions(plugin, {
                                ...plugin.installation.approvedPermissions,
                                [permission.key]: checked,
                              })
                            }
                          />
                        </div>
                      ))}
                    </div>

                    {(plugin.manifest.permissions.readPaths.length > 0 ||
                      plugin.manifest.permissions.writePaths.length > 0 ||
                      plugin.manifest.permissions.env.length > 0) && (
                      <div className="mt-3 space-y-2 text-[11px] text-muted-foreground">
                        {plugin.manifest.permissions.readPaths.length > 0 && (
                          <p>
                            {t('download.pluginPermissionReadPathsLabel')}:{' '}
                            {plugin.manifest.permissions.readPaths.join(', ')}
                          </p>
                        )}
                        {plugin.manifest.permissions.writePaths.length > 0 && (
                          <p>
                            {t('download.pluginPermissionWritePathsLabel')}:{' '}
                            {plugin.manifest.permissions.writePaths.join(', ')}
                          </p>
                        )}
                        {plugin.manifest.permissions.env.length > 0 && (
                          <p>
                            {t('download.pluginPermissionEnvLabel')}:{' '}
                            {plugin.manifest.permissions.env.join(', ')}
                          </p>
                        )}
                      </div>
                    )}

                    {plugin.manifest.permissions.env.length > 0 && (
                      <div className="mt-4 space-y-3 border-t border-border/50 pt-3">
                        <div className="space-y-1">
                          <p className="text-xs font-medium">{t('download.pluginEnvTitle')}</p>
                          <p className="text-[11px] text-muted-foreground">
                            {t('download.pluginEnvDesc')}
                          </p>
                        </div>

                        {plugin.manifest.permissions.env.map((envKey) => {
                          const isSet = plugin.installation.envValueStatus[envKey] ?? false;
                          const draftValue = getEnvDraftValue(plugin.manifest.pluginId, envKey);
                          const isSecret =
                            envKey.includes('TOKEN') ||
                            envKey.includes('SECRET') ||
                            envKey.includes('KEY') ||
                            envKey.includes('PASSWORD');

                          return (
                            <div
                              key={`${plugin.manifest.pluginId}-${envKey}`}
                              className="rounded-lg border border-border/60 px-3 py-3"
                            >
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <p className="text-xs font-medium">{envKey}</p>
                                <span
                                  className={cn(
                                    'rounded px-2 py-0.5 text-[10px]',
                                    isSet
                                      ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                                      : 'bg-amber-500/10 text-amber-600 dark:text-amber-400',
                                  )}
                                >
                                  {isSet
                                    ? t('download.pluginEnvValueSet')
                                    : t('download.pluginEnvValueMissing')}
                                </span>
                              </div>

                              <div className="mt-2 flex flex-col gap-2 sm:flex-row">
                                <Input
                                  type={isSecret ? 'password' : 'text'}
                                  value={draftValue}
                                  onChange={(event) =>
                                    setEnvDraftValue(
                                      plugin.manifest.pluginId,
                                      envKey,
                                      event.target.value,
                                    )
                                  }
                                  placeholder={
                                    isSet
                                      ? t('download.pluginEnvReplacePlaceholder')
                                      : t('download.pluginEnvValuePlaceholder')
                                  }
                                />
                                <div className="flex gap-2">
                                  <Button
                                    size="sm"
                                    onClick={() => handleSavePluginEnv(plugin, envKey)}
                                    disabled={!draftValue.trim()}
                                  >
                                    {t('download.pluginEnvSave')}
                                  </Button>
                                  <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={() => handleClearPluginEnv(plugin, envKey)}
                                    disabled={!isSet}
                                  >
                                    {t('download.pluginEnvClear')}
                                  </Button>
                                </div>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </SettingsCard>

      <PluginLogsDialog
        open={logsOpen}
        onOpenChange={(open) => {
          setLogsOpen(open);
          if (!open) {
            setSelectedPluginId(null);
            setPluginLogs([]);
            setPluginLogsError(null);
          }
        }}
        plugin={selectedPlugin}
        logs={pluginLogs}
        loading={logsLoading}
        error={pluginLogsError}
        onRefresh={() => (selectedPluginId ? loadPluginLogs(selectedPluginId) : undefined)}
      />
    </>
  );
}
