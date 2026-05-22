import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import {
  Braces,
  ChevronDown,
  Download,
  FolderOpen,
  Info,
  MoveDown,
  MoveUp,
  PackageOpen,
  Plus,
  RefreshCw,
  ShieldCheck,
  TerminalSquare,
  Trash2,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PluginLogsDialog } from '@/components/settings/PluginLogsDialog';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { localizeUnknownError } from '@/lib/backend-error';
import { buildWorkflowSnapshotMap, savePluginWorkflowSnapshots } from '@/lib/post-download-plugins';
import type {
  LogEntry,
  PluginCompatibilitySpec,
  PluginExecutionStatusEvent,
  PluginLogsPage,
  PluginPackageInspection,
  PluginPermissionApproval,
  PluginProvider,
  PluginRuntimeLanguage,
  PluginSummary,
  PluginTriggerWorkflow,
  PluginWorkflowFailurePolicy,
  PluginWorkspaceSummary,
  RuntimeProviderStatus,
} from '@/lib/types';
import { cn } from '@/lib/utils';
import { SettingsCard } from './SettingsSection';

type InstallPluginSourceInput = {
  kind: 'package-ywp';
  value: string;
};

type CreatePluginFormState = {
  name: string;
  destinationRoot: string;
  id: string;
  slug: string;
  version: string;
  description: string;
  author: string;
  homepage: string;
  repository: string;
  license: string;
  timeoutSec: string;
  supportedProviders: PluginProvider[];
  preferredProvider: PluginProvider;
  triggers: WorkflowTrigger[];
  permissionNetwork: boolean;
  permissionReadPaths: string;
  permissionWritePaths: string;
  permissionEnv: string;
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

function buildRequestedPermissionApproval(plugin: PluginSummary): PluginPermissionApproval {
  return {
    network: plugin.manifest.permissions.network,
    readPaths: plugin.manifest.permissions.readPaths.length > 0,
    writePaths: plugin.manifest.permissions.writePaths.length > 0,
    env: plugin.manifest.permissions.env.length > 0,
  };
}

function hasUnapprovedRequestedPermissions(plugin: PluginSummary) {
  const requested = buildRequestedPermissionApproval(plugin);
  return (
    (requested.network && !plugin.installation.approvedPermissions.network) ||
    (requested.readPaths && !plugin.installation.approvedPermissions.readPaths) ||
    (requested.writePaths && !plugin.installation.approvedPermissions.writePaths) ||
    (requested.env && !plugin.installation.approvedPermissions.env)
  );
}

function currentProvider(plugin: PluginSummary) {
  return (
    plugin.installation.selectedProvider ??
    plugin.manifest.runtime.preferredProvider ??
    plugin.manifest.runtime.supportedProviders[0]
  );
}

function currentTimeoutSec(plugin: PluginSummary) {
  return plugin.installation.timeoutSecOverride ?? plugin.manifest.timeoutSec;
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

function formatPluginIdentifier(pluginId: string, slug: string) {
  if (pluginId.endsWith(`.${slug}`)) {
    return pluginId;
  }
  return `${pluginId} • ${slug}`;
}

function formatChecksum(checksum: string) {
  if (checksum.length <= 20) return checksum;
  return `${checksum.slice(0, 8)}...${checksum.slice(-8)}`;
}

function formatSignerFingerprint(fingerprint: string) {
  if (fingerprint.length <= 24) return fingerprint;
  return `${fingerprint.slice(0, 12)}...${fingerprint.slice(-12)}`;
}

function formatSourceKind(
  kind: PluginSummary['installation']['source']['kind'] | PluginPackageInspection['source']['kind'],
  t: (key: string, opts?: Record<string, unknown>) => string,
) {
  switch (kind) {
    case 'workspace':
      return t('download.pluginSourceWorkspace');
    case 'package-ywp':
      return t('download.pluginSourcePackageYwp');
    default:
      return kind;
  }
}

function formatPackageFormat(
  format: string | null | undefined,
  version: number | null | undefined,
) {
  if (!format) return null;
  return version ? `${format.toUpperCase()} v${version}` : format.toUpperCase();
}

function formatSignatureStatus(
  status: string | null | undefined,
  t: (key: string, opts?: Record<string, unknown>) => string,
) {
  switch (status) {
    case 'signed':
      return t('download.pluginSignatureSigned');
    case 'invalid-signature':
      return t('download.pluginSignatureInvalid');
    case 'missing-signature':
      return t('download.pluginSignatureMissing');
    case 'signer-changed':
      return t('download.pluginSignatureSignerChanged');
    default:
      return t('download.pluginSignatureUnknown');
  }
}

function formatRuntimeStatusBadge(
  status: string | undefined,
  t: (key: string, opts?: Record<string, unknown>) => string,
) {
  if (status === 'running') return t('download.pluginStatusRunning');
  if (status === 'success') return t('download.pluginStatusSuccess');
  if (status === 'error') return t('download.pluginStatusError');
  return status;
}

const WORKFLOW_TRIGGERS = [
  'download.queued',
  'download.beforeStart',
  'download.completed',
  'download.failed',
] as const;
type WorkflowTrigger = (typeof WORKFLOW_TRIGGERS)[number];

const DEFAULT_CREATE_PLUGIN_FORM: CreatePluginFormState = {
  name: '',
  destinationRoot: '',
  id: '',
  slug: '',
  version: '0.1.0',
  description: '',
  author: '',
  homepage: '',
  repository: '',
  license: 'MIT',
  timeoutSec: '60',
  supportedProviders: ['node', 'bun'],
  preferredProvider: 'node',
  triggers: ['download.completed'],
  permissionNetwork: false,
  permissionReadPaths: '',
  permissionWritePaths: '',
  permissionEnv: '',
};

function parseMultilineList(value: string) {
  return value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function PostDownloadPluginsCard() {
  const { t } = useTranslation('settings');
  const [plugins, setPlugins] = useState<PluginSummary[]>([]);
  const [providers, setProviders] = useState<RuntimeProviderStatus[]>([]);
  const [workflows, setWorkflows] = useState<Record<string, PluginTriggerWorkflow>>({});
  const [workflowCandidates, setWorkflowCandidates] = useState<Record<string, string>>({});
  const [defaultProviders, setDefaultProviders] = useState<
    Partial<Record<PluginRuntimeLanguage, PluginProvider>>
  >({});
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [inspecting, setInspecting] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [runtimeGuideOpen, setRuntimeGuideOpen] = useState(false);
  const [expandedPluginId, setExpandedPluginId] = useState<string | null>(null);
  const [createPluginForm, setCreatePluginForm] = useState<CreatePluginFormState>(
    DEFAULT_CREATE_PLUGIN_FORM,
  );
  const [inspection, setInspection] = useState<PluginPackageInspection | null>(null);
  const [installSource, setInstallSource] = useState<InstallPluginSourceInput | null>(null);
  const [installAcknowledged, setInstallAcknowledged] = useState(false);
  const [envDrafts, setEnvDrafts] = useState<Record<string, string>>({});
  const [timeoutDrafts, setTimeoutDrafts] = useState<Record<string, string>>({});
  const [runtimeStatuses, setRuntimeStatuses] = useState<
    Record<string, { status: string; message?: string | null }>
  >({});
  const [logsOpen, setLogsOpen] = useState(false);
  const [logsLoading, setLogsLoading] = useState(false);
  const [logsLoadingMore, setLogsLoadingMore] = useState(false);
  const [logsClearing, setLogsClearing] = useState(false);
  const [pluginLogs, setPluginLogs] = useState<LogEntry[]>([]);
  const [pluginLogsTotal, setPluginLogsTotal] = useState(0);
  const [pluginLogsHasMore, setPluginLogsHasMore] = useState(false);
  const [pluginLogsOffset, setPluginLogsOffset] = useState(0);
  const [selectedPluginId, setSelectedPluginId] = useState<string | null>(null);
  const [pluginLogsError, setPluginLogsError] = useState<string | null>(null);
  const [clearLogsConfirmOpen, setClearLogsConfirmOpen] = useState(false);
  const [permissionDialogPlugin, setPermissionDialogPlugin] = useState<PluginSummary | null>(null);
  const [permissionDialogState, setPermissionDialogState] = useState<PluginPermissionApproval>({
    network: false,
    readPaths: false,
    writePaths: false,
    env: false,
  });
  const [createdWorkspace, setCreatedWorkspace] = useState<PluginWorkspaceSummary | null>(null);
  const [attachWorkspacePath, setAttachWorkspacePath] = useState<string | null>(null);
  const [uninstallTarget, setUninstallTarget] = useState<PluginSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPlugins = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [pluginResult, providerResult, workflowResults] = await Promise.all([
        invoke<PluginSummary[]>('list_plugins'),
        invoke<RuntimeProviderStatus[]>('list_runtime_providers'),
        Promise.all(
          WORKFLOW_TRIGGERS.map((trigger) =>
            invoke<PluginTriggerWorkflow>('get_plugin_trigger_workflow', { trigger }),
          ),
        ),
      ]);
      setPlugins(pluginResult);
      setProviders(providerResult);
      setWorkflows(
        Object.fromEntries(workflowResults.map((workflow) => [workflow.trigger, workflow])),
      );

      const defaults: Partial<Record<PluginRuntimeLanguage, PluginProvider>> = {};
      const statuses: Record<string, { status: string; message?: string | null }> = {};
      for (const plugin of pluginResult) {
        const language = plugin.manifest.runtime.language;
        if (!defaults[language]) {
          defaults[language] = currentProvider(plugin);
        }
        if (plugin.installation.lastExecutionStatus || plugin.installation.lastError) {
          statuses[plugin.manifest.id] = {
            status: plugin.installation.lastExecutionStatus ?? 'idle',
            message: plugin.installation.lastError,
          };
        }
      }
      setDefaultProviders(defaults);
      setRuntimeStatuses(statuses);
      savePluginWorkflowSnapshots(buildWorkflowSnapshotMap(pluginResult, workflowResults));
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
    savePluginWorkflowSnapshots(
      buildWorkflowSnapshotMap(
        plugins,
        WORKFLOW_TRIGGERS.map((trigger) => workflows[trigger] ?? { trigger, steps: [] }),
      ),
    );
  }, [workflows, plugins]);

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
      savePluginWorkflowSnapshots(
        buildWorkflowSnapshotMap(
          next,
          WORKFLOW_TRIGGERS.map((trigger) => workflows[trigger] ?? { trigger, steps: [] }),
        ),
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
      setInstallAcknowledged(false);
    } catch (err) {
      console.error('Failed to inspect plugin package:', err);
      setError(localizeUnknownError(err));
      setInspection(null);
      setInstallSource(null);
    } finally {
      setInspecting(false);
    }
  };

  const handleImportPackage = async () => {
    const selected = await open({
      directory: false,
      multiple: false,
      title: t('download.pluginImportPlugin'),
      filters: [{ name: 'Youwee Plugin File', extensions: ['ywp'] }],
    });
    if (typeof selected !== 'string') return;
    await inspectSource({ kind: 'package-ywp', value: selected }, 'inspect_plugin_package', 'path');
  };

  const handlePickWorkspaceRoot = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t('download.pluginCreatePickLocation'),
    });
    if (typeof selected !== 'string') return;
    updateCreatePluginForm('destinationRoot', selected);
  };

  const handleAttachWorkspace = async (workspacePath?: string) => {
    const selected =
      workspacePath ??
      (await open({
        directory: true,
        multiple: false,
        title: t('download.pluginAttachWorkspace'),
      }));

    if (typeof selected !== 'string' || !selected) return;
    setAttachWorkspacePath(selected);
  };

  const handleConfirmAttachWorkspace = async () => {
    if (!attachWorkspacePath) return;
    setError(null);
    try {
      await invoke<PluginSummary>('attach_plugin_workspace', {
        input: { value: attachWorkspacePath },
      });
      setAttachWorkspacePath(null);
      await loadPlugins();
    } catch (err) {
      console.error('Failed to attach plugin workspace:', err);
      setError(localizeUnknownError(err));
    }
  };

  const handleInstallInspection = async () => {
    if (!inspection || !installSource) return;
    setInstalling(true);
    setError(null);
    try {
      await invoke<PluginSummary>('install_plugin_package', {
        path: installSource.value,
        trusted: true,
      });
      setInspection(null);
      setInstallSource(null);
      setInstallAcknowledged(false);
      await loadPlugins();
    } catch (err) {
      console.error('Failed to install plugin:', err);
      setError(localizeUnknownError(err));
    } finally {
      setInstalling(false);
    }
  };

  const handleTogglePlugin = async (plugin: PluginSummary, enabled: boolean) => {
    if (enabled && hasUnapprovedRequestedPermissions(plugin)) {
      const requested = buildRequestedPermissionApproval(plugin);
      setPermissionDialogPlugin(plugin);
      setPermissionDialogState({
        network: requested.network ? plugin.installation.approvedPermissions.network : false,
        readPaths: requested.readPaths ? plugin.installation.approvedPermissions.readPaths : false,
        writePaths: requested.writePaths
          ? plugin.installation.approvedPermissions.writePaths
          : false,
        env: requested.env ? plugin.installation.approvedPermissions.env : false,
      });
      return;
    }

    try {
      await invoke('update_plugin_state', { pluginId: plugin.manifest.id, enabled });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
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

  const handleEnablePluginWithPermissions = async () => {
    if (!permissionDialogPlugin) return;

    try {
      await invoke('approve_plugin_permissions', {
        pluginId: permissionDialogPlugin.manifest.id,
        permissions: permissionDialogState,
      });
      await invoke('update_plugin_state', {
        pluginId: permissionDialogPlugin.manifest.id,
        enabled: true,
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === permissionDialogPlugin.manifest.id
            ? {
                ...item,
                installation: {
                  ...item.installation,
                  enabled: true,
                  approvedPermissions: permissionDialogState,
                },
              }
            : item,
        ),
      );
      setPermissionDialogPlugin(null);
    } catch (err) {
      console.error('Failed to enable plugin with permissions:', err);
      setError(t('download.pluginPermissionEnableError'));
    }
  };

  const handleApprovePermissions = async (
    plugin: PluginSummary,
    permissions: PluginPermissionApproval,
  ) => {
    try {
      await invoke('approve_plugin_permissions', {
        pluginId: plugin.manifest.id,
        permissions,
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
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
    const trimmedName = createPluginForm.name.trim();
    if (!trimmedName) return;

    const supportedProviders =
      createPluginForm.supportedProviders.length > 0
        ? createPluginForm.supportedProviders
        : DEFAULT_CREATE_PLUGIN_FORM.supportedProviders;
    const preferredProvider = supportedProviders.includes(createPluginForm.preferredProvider)
      ? createPluginForm.preferredProvider
      : supportedProviders[0];

    setCreating(true);
    setError(null);
    try {
      const result = await invoke<PluginWorkspaceSummary>('create_plugin_workspace', {
        input: {
          name: trimmedName,
          destinationRoot: createPluginForm.destinationRoot.trim(),
          id: createPluginForm.id.trim() || null,
          slug: createPluginForm.slug.trim() || null,
          version: createPluginForm.version.trim() || null,
          description: createPluginForm.description.trim() || null,
          author: createPluginForm.author.trim() || null,
          homepage: createPluginForm.homepage.trim() || null,
          repository: createPluginForm.repository.trim() || null,
          license: createPluginForm.license.trim() || null,
          timeoutSec: Number.parseInt(createPluginForm.timeoutSec, 10) || 60,
          triggers:
            createPluginForm.triggers.length > 0
              ? createPluginForm.triggers
              : DEFAULT_CREATE_PLUGIN_FORM.triggers,
          supportedProviders,
          preferredProvider,
          permissions: {
            network: createPluginForm.permissionNetwork,
            readPaths: parseMultilineList(createPluginForm.permissionReadPaths),
            writePaths: parseMultilineList(createPluginForm.permissionWritePaths),
            env: parseMultilineList(createPluginForm.permissionEnv),
          },
        },
      });
      setCreatedWorkspace(result);
      setCreatePluginForm(DEFAULT_CREATE_PLUGIN_FORM);
      setCreateOpen(false);
    } catch (err) {
      console.error('Failed to create plugin:', err);
      setError(localizeUnknownError(err));
    } finally {
      setCreating(false);
    }
  };

  const updateCreatePluginForm = <K extends keyof CreatePluginFormState>(
    key: K,
    value: CreatePluginFormState[K],
  ) => {
    setCreatePluginForm((current) => ({ ...current, [key]: value }));
  };

  const toggleCreatePluginProvider = (provider: PluginProvider) => {
    setCreatePluginForm((current) => {
      const exists = current.supportedProviders.includes(provider);
      const supportedProviders = exists
        ? current.supportedProviders.filter((item) => item !== provider)
        : [...current.supportedProviders, provider];

      if (supportedProviders.length === 0) {
        return {
          ...current,
          supportedProviders,
          preferredProvider: DEFAULT_CREATE_PLUGIN_FORM.preferredProvider,
        };
      }

      return {
        ...current,
        supportedProviders,
        preferredProvider: supportedProviders.includes(current.preferredProvider)
          ? current.preferredProvider
          : supportedProviders[0],
      };
    });
  };

  const toggleCreatePluginTrigger = (trigger: WorkflowTrigger) => {
    setCreatePluginForm((current) => ({
      ...current,
      triggers: current.triggers.includes(trigger)
        ? current.triggers.filter((item) => item !== trigger)
        : [...current.triggers, trigger],
    }));
  };

  const handleOpenPluginDirectory = async (pluginId: string) => {
    try {
      await invoke('open_plugin_directory', { pluginId });
    } catch (err) {
      console.error('Failed to open plugin directory:', err);
      setError(t('download.pluginOpenDirError'));
    }
  };

  const handleOpenWorkspacePath = async (path: string) => {
    try {
      await invoke('open_file_location', { filepath: path });
    } catch (err) {
      console.error('Failed to open workspace path:', err);
      setError(t('download.pluginWorkspaceOpenError'));
    }
  };

  const handleRefreshPlugin = async (pluginId: string) => {
    try {
      const refreshed = await invoke<PluginSummary>('get_plugin_details', { pluginId });
      updatePluginList((items) =>
        items.map((item) => (item.manifest.id === pluginId ? refreshed : item)),
      );
      setRuntimeStatuses((current) => ({
        ...current,
        [pluginId]: {
          status: refreshed.installation.lastExecutionStatus ?? current[pluginId]?.status ?? 'idle',
          message: refreshed.installation.lastError ?? current[pluginId]?.message ?? null,
        },
      }));
    } catch (err) {
      console.error('Failed to refresh plugin details:', err);
      setError(localizeUnknownError(err));
    }
  };

  const handleSetPluginProvider = async (plugin: PluginSummary, provider: PluginProvider) => {
    try {
      await invoke('set_plugin_provider', { pluginId: plugin.manifest.id, provider });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
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

  const setTimeoutDraftValue = (pluginId: string, value: string) => {
    setTimeoutDrafts((current) => ({
      ...current,
      [pluginId]: value,
    }));
  };

  const getTimeoutDraftValue = (plugin: PluginSummary) =>
    timeoutDrafts[plugin.manifest.id] ?? String(currentTimeoutSec(plugin));

  const handleSavePluginEnv = async (plugin: PluginSummary, key: string) => {
    const value = getEnvDraftValue(plugin.manifest.id, key);
    try {
      await invoke('update_plugin_env_values', {
        pluginId: plugin.manifest.id,
        input: {
          values: {
            [key]: value.trim() ? value : null,
          },
        },
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
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
      setEnvDraftValue(plugin.manifest.id, key, '');
    } catch (err) {
      console.error('Failed to update plugin env values:', err);
      setError(t('download.pluginEnvSaveError'));
    }
  };

  const handleClearPluginEnv = async (plugin: PluginSummary, key: string) => {
    try {
      await invoke('update_plugin_env_values', {
        pluginId: plugin.manifest.id,
        input: {
          values: {
            [key]: null,
          },
        },
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
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
      setEnvDraftValue(plugin.manifest.id, key, '');
    } catch (err) {
      console.error('Failed to clear plugin env value:', err);
      setError(t('download.pluginEnvSaveError'));
    }
  };

  const handleSavePluginTimeout = async (plugin: PluginSummary) => {
    const rawValue = getTimeoutDraftValue(plugin).trim();
    const parsed = Number.parseInt(rawValue, 10);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      setError(t('download.pluginTimeoutError'));
      return;
    }

    try {
      await invoke('set_plugin_timeout', {
        pluginId: plugin.manifest.id,
        timeoutSec: parsed,
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
            ? {
                ...item,
                installation: { ...item.installation, timeoutSecOverride: parsed },
              }
            : item,
        ),
      );
      setTimeoutDraftValue(plugin.manifest.id, String(parsed));
    } catch (err) {
      console.error('Failed to update plugin timeout:', err);
      setError(t('download.pluginTimeoutSaveError'));
    }
  };

  const handleResetPluginTimeout = async (plugin: PluginSummary) => {
    try {
      await invoke('set_plugin_timeout', {
        pluginId: plugin.manifest.id,
        timeoutSec: null,
      });
      updatePluginList((items) =>
        items.map((item) =>
          item.manifest.id === plugin.manifest.id
            ? {
                ...item,
                installation: { ...item.installation, timeoutSecOverride: null },
              }
            : item,
        ),
      );
      setTimeoutDraftValue(plugin.manifest.id, String(plugin.manifest.timeoutSec));
    } catch (err) {
      console.error('Failed to reset plugin timeout:', err);
      setError(t('download.pluginTimeoutSaveError'));
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

  const persistWorkflow = async (nextWorkflow: PluginTriggerWorkflow) => {
    try {
      const saved = await invoke<PluginTriggerWorkflow>('update_plugin_trigger_workflow', {
        workflow: nextWorkflow,
      });
      setWorkflows((current) => ({
        ...current,
        [saved.trigger]: saved,
      }));
    } catch (err) {
      console.error('Failed to update plugin workflow:', err);
      setError(localizeUnknownError(err));
    }
  };

  const handleAddWorkflowPlugin = async (trigger: WorkflowTrigger) => {
    const workflow = workflows[trigger] ?? { trigger, steps: [] };
    const pluginId = workflowCandidates[trigger] ?? '';
    if (!pluginId) return;
    await persistWorkflow({
      trigger,
      steps: [
        ...workflow.steps,
        {
          pluginId,
          failurePolicy: 'continue',
        },
      ],
    });
    setWorkflowCandidates((current) => ({ ...current, [trigger]: '' }));
  };

  const handleRemoveWorkflowStep = async (trigger: WorkflowTrigger, pluginId: string) => {
    const workflow = workflows[trigger] ?? { trigger, steps: [] };
    await persistWorkflow({
      trigger: workflow.trigger,
      steps: workflow.steps.filter((step) => step.pluginId !== pluginId),
    });
  };

  const handleMoveWorkflowStep = async (
    trigger: WorkflowTrigger,
    pluginId: string,
    direction: -1 | 1,
  ) => {
    const workflow = workflows[trigger] ?? { trigger, steps: [] };
    const index = workflow.steps.findIndex((step) => step.pluginId === pluginId);
    if (index < 0) return;
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= workflow.steps.length) return;
    const steps = [...workflow.steps];
    const [current] = steps.splice(index, 1);
    steps.splice(nextIndex, 0, current);
    await persistWorkflow({
      trigger: workflow.trigger,
      steps,
    });
  };

  const handleWorkflowFailurePolicy = async (
    trigger: WorkflowTrigger,
    pluginId: string,
    failurePolicy: PluginWorkflowFailurePolicy,
  ) => {
    const workflow = workflows[trigger] ?? { trigger, steps: [] };
    await persistWorkflow({
      trigger: workflow.trigger,
      steps: workflow.steps.map((step) =>
        step.pluginId === pluginId ? { ...step, failurePolicy } : step,
      ),
    });
  };

  const workflowPluginsByTrigger = useMemo(
    () =>
      Object.fromEntries(
        WORKFLOW_TRIGGERS.map((trigger) => {
          const workflow = workflows[trigger] ?? { trigger, steps: [] };
          return [
            trigger,
            workflow.steps
              .map((step) => ({
                step,
                plugin: plugins.find((plugin) => plugin.manifest.id === step.pluginId) ?? null,
              }))
              .filter((entry) => entry.plugin != null),
          ];
        }),
      ) as Record<
        WorkflowTrigger,
        Array<{ step: PluginTriggerWorkflow['steps'][number]; plugin: PluginSummary | null }>
      >,
    [workflows, plugins],
  );

  const availableWorkflowPluginsByTrigger = useMemo(
    () =>
      Object.fromEntries(
        WORKFLOW_TRIGGERS.map((trigger) => {
          const workflow = workflows[trigger] ?? { trigger, steps: [] };
          return [
            trigger,
            plugins.filter(
              (plugin) =>
                plugin.installation.enabled &&
                plugin.manifest.triggers.includes(trigger) &&
                !workflow.steps.some((step) => step.pluginId === plugin.manifest.id),
            ),
          ];
        }),
      ) as Record<WorkflowTrigger, PluginSummary[]>,
    [workflows, plugins],
  );

  const selectedPlugin =
    selectedPluginId != null
      ? (plugins.find((plugin) => plugin.manifest.id === selectedPluginId) ?? null)
      : null;

  const inspectionCompatibilityEntries = useMemo(
    () => (inspection ? summarizeCompatibility(inspection.manifest.compatibility, t) : []),
    [inspection, t],
  );
  const inspectionSigned = inspection?.signatureStatus === 'signed';

  const loadPluginLogs = useCallback(
    async (pluginId: string, mode: 'replace' | 'append' = 'replace') => {
      const limit = 60;
      const offset = mode === 'append' ? pluginLogsOffset : 0;

      if (mode === 'append') {
        setLogsLoadingMore(true);
      } else {
        setLogsLoading(true);
        setPluginLogs([]);
        setPluginLogsTotal(0);
        setPluginLogsHasMore(false);
        setPluginLogsOffset(0);
      }

      setPluginLogsError(null);
      try {
        const result = await invoke<PluginLogsPage>('get_plugin_logs', {
          pluginId,
          limit,
          offset,
        });
        setPluginLogs((current) =>
          mode === 'append' ? [...current, ...result.items] : result.items,
        );
        setPluginLogsTotal(result.total);
        setPluginLogsHasMore(result.has_more);
        setPluginLogsOffset(offset + result.items.length);
      } catch (err) {
        console.error('Failed to load plugin logs:', err);
        setPluginLogsError(t('download.pluginLogsLoadError'));
      } finally {
        if (mode === 'append') {
          setLogsLoadingMore(false);
        } else {
          setLogsLoading(false);
        }
      }
    },
    [pluginLogsOffset, t],
  );

  const handleOpenPluginLogs = async (pluginId: string) => {
    setSelectedPluginId(pluginId);
    setLogsOpen(true);
    await loadPluginLogs(pluginId, 'replace');
  };

  const handleLoadMorePluginLogs = async () => {
    if (!selectedPluginId || logsLoadingMore || !pluginLogsHasMore) {
      return;
    }

    await loadPluginLogs(selectedPluginId, 'append');
  };

  const handleClearPluginLogs = async () => {
    if (!selectedPluginId) {
      return;
    }
    setClearLogsConfirmOpen(true);
  };

  const handleConfirmClearPluginLogs = async () => {
    if (!selectedPluginId) {
      return;
    }
    setLogsClearing(true);
    setPluginLogsError(null);
    try {
      await invoke('clear_plugin_logs', {
        pluginId: selectedPluginId,
      });
      setPluginLogs([]);
      setPluginLogsTotal(0);
      setPluginLogsHasMore(false);
      setPluginLogsOffset(0);
      setClearLogsConfirmOpen(false);
    } catch (err) {
      console.error('Failed to clear plugin logs:', err);
      setPluginLogsError(t('download.pluginLogsClearError'));
    } finally {
      setLogsClearing(false);
    }
  };

  const handleUninstallPlugin = async (plugin: PluginSummary) => {
    setUninstallTarget(plugin);
  };

  const handleConfirmUninstallPlugin = async () => {
    if (!uninstallTarget) {
      return;
    }
    try {
      await invoke('uninstall_plugin', { pluginId: uninstallTarget.manifest.id });
      setPlugins((current) =>
        current.filter((item) => item.manifest.id !== uninstallTarget.manifest.id),
      );
      setRuntimeStatuses((current) => {
        const next = { ...current };
        delete next[uninstallTarget.manifest.id];
        return next;
      });
      setWorkflows((current) =>
        Object.fromEntries(
          Object.entries(current).map(([trigger, workflow]) => [
            trigger,
            {
              ...workflow,
              steps: workflow.steps.filter((step) => step.pluginId !== uninstallTarget.manifest.id),
            },
          ]),
        ),
      );
      if (selectedPluginId === uninstallTarget.manifest.id) {
        setLogsOpen(false);
        setSelectedPluginId(null);
      }
      setUninstallTarget(null);
    } catch (err) {
      console.error('Failed to uninstall plugin:', err);
      setError(t('download.pluginUninstallError'));
    }
  };

  return (
    <>
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          variant="outline"
          size="sm"
          className="border-dashed"
          onClick={() => setRuntimeGuideOpen(true)}
        >
          <Info className="h-4 w-4" />
          {t('download.pluginRuntimeGuideButton')}
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="border-dashed"
          onClick={() => handleAttachWorkspace()}
        >
          <Braces className="h-4 w-4" />
          {t('download.pluginAttachWorkspace')}
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="border-dashed"
          onClick={handleImportPackage}
          disabled={inspecting}
        >
          <PackageOpen className="h-4 w-4" />
          {t('download.pluginImportPlugin')}
        </Button>
        <Button size="sm" onClick={() => setCreateOpen(true)}>
          <Plus className="h-4 w-4" />
          {t('download.pluginCreateWorkspace')}
        </Button>
      </div>

      <div className="rounded-xl border border-dashed border-border/70 bg-muted/20 p-4">
        <div className="space-y-2">
          <p className="text-sm font-medium">{t('download.pluginWorkspaceFlowTitle')}</p>
          <p className="text-xs text-muted-foreground">{t('download.pluginWorkspaceFlowDesc')}</p>
        </div>
        <div className="mt-3 grid gap-3 md:grid-cols-3">
          <div className="rounded-lg border border-border/60 bg-background/70 p-3">
            <p className="text-xs font-medium">{t('download.pluginWorkspaceStep1Title')}</p>
            <p className="mt-1 text-[11px] text-muted-foreground">
              {t('download.pluginWorkspaceStep1Desc')}
            </p>
          </div>
          <div className="rounded-lg border border-border/60 bg-background/70 p-3">
            <p className="text-xs font-medium">{t('download.pluginWorkspaceStep2Title')}</p>
            <p className="mt-1 text-[11px] text-muted-foreground">
              {t('download.pluginWorkspaceStep2Desc')}
            </p>
          </div>
          <div className="rounded-lg border border-border/60 bg-background/70 p-3">
            <p className="text-xs font-medium">{t('download.pluginWorkspaceStep3Title')}</p>
            <p className="mt-1 text-[11px] text-muted-foreground">
              {t('download.pluginWorkspaceStep3Desc')}
            </p>
          </div>
        </div>
      </div>

      {createdWorkspace && (
        <div className="rounded-xl border border-dashed border-emerald-500/40 bg-emerald-500/5 p-4">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
            <div className="space-y-2">
              <p className="text-sm font-semibold">{t('download.pluginWorkspaceCreatedTitle')}</p>
              <p className="text-xs text-muted-foreground">
                {t('download.pluginWorkspaceCreatedDesc', { name: createdWorkspace.name })}
              </p>
              <div className="space-y-1 text-[11px] text-muted-foreground">
                <p className="break-all">
                  {t('download.pluginWorkspacePathLabel')}: {createdWorkspace.path}
                </p>
                <p className="break-all">
                  {t('download.pluginManifestPathLabel')}: {createdWorkspace.manifestPath}
                </p>
              </div>
              <div className="rounded-lg border border-border/60 bg-background/70 p-3 text-[11px] text-muted-foreground">
                <p>{t('download.pluginWorkspaceNextStep1')}</p>
                <p>{t('download.pluginWorkspaceNextStep2')}</p>
                <p>{t('download.pluginWorkspaceNextStep3')}</p>
              </div>
            </div>

            <div className="flex gap-2">
              <Button
                variant="outline"
                onClick={() => handleOpenWorkspacePath(createdWorkspace.path)}
              >
                <FolderOpen className="h-4 w-4" />
                {t('download.pluginWorkspaceOpen')}
              </Button>
              <Button
                variant="outline"
                onClick={() => handleAttachWorkspace(createdWorkspace.path)}
              >
                <Braces className="h-4 w-4" />
                {t('download.pluginAttachWorkspace')}
              </Button>
              <Button variant="outline" onClick={() => setCreatedWorkspace(null)}>
                {t('download.pluginDismiss')}
              </Button>
            </div>
          </div>
        </div>
      )}

      <SettingsCard className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm font-medium">{t('download.pluginsTitle')}</p>
          <Button variant="outline" size="sm" onClick={loadPlugins} disabled={loading}>
            <RefreshCw className={cn('h-4 w-4', loading && 'animate-spin')} />
            {t('download.pluginReload')}
          </Button>
        </div>

        {error && <p className="text-xs text-destructive">{error}</p>}

        {inspection && installSource && (
          <div className="rounded-xl border border-dashed border-amber-500/40 bg-amber-500/5 p-4">
            <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="text-sm font-semibold">{inspection.manifest.name}</p>
                  <span className="rounded bg-muted px-2 py-0.5 text-[10px] tracking-wide text-muted-foreground">
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
                  <span>{inspection.manifest.id}</span>
                  <span>•</span>
                  <span>{formatSourceKind(inspection.source.kind, t)}</span>
                </div>
                <div className="space-y-1 text-[11px] text-muted-foreground">
                  <p className="font-medium text-foreground/80">
                    {t('download.pluginCompatibilityTitle')}
                  </p>
                  <p>
                    {t('download.pluginSignatureTitle')}:{' '}
                    {formatSignatureStatus(inspection.signatureStatus, t)}
                  </p>
                  {inspection.signerFingerprint && (
                    <p>
                      {t('download.pluginSignerFingerprintLabel')}:{' '}
                      {formatSignerFingerprint(inspection.signerFingerprint)}
                    </p>
                  )}
                  {inspection.signedAt && (
                    <p>
                      {t('download.pluginSignedAtLabel')}: {inspection.signedAt}
                    </p>
                  )}
                  {inspection.packageFormat && (
                    <p>
                      {t('download.pluginPackageFormatLabel')}:{' '}
                      {formatPackageFormat(
                        inspection.packageFormat,
                        inspection.packageFormatVersion,
                      )}
                    </p>
                  )}
                  {inspection.builderSdkVersion && (
                    <p>
                      {t('download.pluginBuilderSdkVersionLabel')}: v{inspection.builderSdkVersion}
                    </p>
                  )}
                  {inspection.packageChecksum && (
                    <p className="break-all">
                      {t('download.pluginPackageChecksumLabel')}: {inspection.packageChecksum}
                    </p>
                  )}
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
                <div className="rounded-lg border border-border/60 bg-background/60 px-3 py-3">
                  <label className="flex items-start gap-3">
                    <input
                      type="checkbox"
                      checked={installAcknowledged}
                      onChange={(event) => setInstallAcknowledged(event.target.checked)}
                      className="mt-0.5 h-4 w-4 rounded border-border"
                    />
                    <div className="space-y-1">
                      <p className="text-xs font-medium text-foreground">
                        {t('download.pluginInstallConfirmLabel')}
                      </p>
                      <p className="text-[11px] text-muted-foreground">
                        {t('download.pluginInstallConfirmHelp')}
                      </p>
                    </div>
                  </label>
                </div>
              </div>

              <div className="flex gap-2">
                <Button
                  variant="outline"
                  onClick={() => {
                    setInspection(null);
                    setInstallAcknowledged(false);
                  }}
                  disabled={installing}
                >
                  {t('download.pluginDismiss')}
                </Button>
                <Button
                  onClick={handleInstallInspection}
                  disabled={installing || !installAcknowledged || !inspectionSigned}
                >
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
          </div>
        ) : (
          <div className="space-y-3">
            {plugins.map((plugin) => {
              const requestedPermissions = summarizeRequestedPermissions(plugin, t);
              const compatibilityEntries = summarizeCompatibility(plugin.manifest.compatibility, t);
              const selectedProvider = currentProvider(plugin);
              const supportedProviders = plugin.manifest.runtime.supportedProviders;
              const runtimeStatus = runtimeStatuses[plugin.manifest.id];
              const isExpanded = expandedPluginId === plugin.manifest.id;
              return (
                <Collapsible
                  key={plugin.manifest.id}
                  open={isExpanded}
                  onOpenChange={(open) => setExpandedPluginId(open ? plugin.manifest.id : null)}
                >
                  <div className="rounded-xl border border-border/60 bg-background/60">
                    <div className="flex items-center gap-3 px-4 py-3">
                      <CollapsibleTrigger asChild>
                        <button
                          type="button"
                          className="flex min-w-0 flex-1 items-center gap-3 text-left"
                        >
                          <div className="rounded-xl bg-purple-500/10 p-2 text-purple-500">
                            <Braces className="h-4 w-4" />
                          </div>

                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <p className="truncate text-sm font-semibold">
                                {plugin.manifest.name}
                              </p>
                              <span className="rounded bg-muted px-2 py-0.5 text-[10px] tracking-wide text-muted-foreground">
                                v{plugin.manifest.version}
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
                                  {formatRuntimeStatusBadge(runtimeStatus.status, t)}
                                </span>
                              )}
                              {plugin.warnings.length > 0 && (
                                <span className="rounded bg-amber-500/10 px-2 py-0.5 text-[10px] uppercase tracking-wide text-amber-600 dark:text-amber-400">
                                  {t('download.pluginWarningCount', {
                                    count: plugin.warnings.length,
                                  })}
                                </span>
                              )}
                            </div>

                            <p className="mt-1 line-clamp-2 text-[11px] text-muted-foreground">
                              {plugin.manifest.description || t('download.pluginNoDescription')}
                            </p>
                          </div>

                          <ChevronDown
                            className={cn(
                              'h-4 w-4 shrink-0 text-muted-foreground transition-transform',
                              isExpanded && 'rotate-180',
                            )}
                          />
                        </button>
                      </CollapsibleTrigger>

                      <div className="flex items-center gap-3">
                        <span className="hidden text-xs text-muted-foreground sm:inline">
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

                    <CollapsibleContent className="border-t border-border/60 px-4 py-4">
                      <div className="space-y-4">
                        <div className="flex flex-wrap gap-2">
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleRefreshPlugin(plugin.manifest.id)}
                          >
                            <RefreshCw className="h-4 w-4" />
                            {t('download.pluginRefreshInfo')}
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleOpenPluginLogs(plugin.manifest.id)}
                          >
                            <TerminalSquare className="h-4 w-4" />
                            {t('download.pluginViewLogs')}
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleOpenPluginDirectory(plugin.manifest.id)}
                          >
                            <FolderOpen className="h-4 w-4" />
                            {t('download.pluginOpenFolder')}
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            className="border-dashed text-destructive hover:text-destructive"
                            onClick={() => handleUninstallPlugin(plugin)}
                          >
                            <Trash2 className="h-4 w-4" />
                            {t(
                              plugin.installation.source.kind === 'workspace'
                                ? 'download.pluginDetachWorkspace'
                                : 'download.pluginUninstall',
                            )}
                          </Button>
                        </div>

                        <div className="grid gap-3 lg:grid-cols-2">
                          <div className="rounded-xl bg-muted/30 p-3">
                            <div className="flex items-center gap-2 text-xs font-medium">
                              <Info className="h-4 w-4 text-purple-500" />
                              <span>{t('download.pluginPackageTitle')}</span>
                            </div>
                            <p className="mt-2 text-[11px] text-muted-foreground">
                              {t('download.pluginPackageDesc')}
                            </p>
                            <div className="mt-3 grid gap-2 text-[11px] text-muted-foreground sm:grid-cols-2">
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginIdentifierLabel')}
                                </p>
                                <p>
                                  {formatPluginIdentifier(plugin.manifest.id, plugin.manifest.slug)}
                                </p>
                              </div>
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginVersionLabel')}
                                </p>
                                <p>v{plugin.manifest.version}</p>
                              </div>
                              {plugin.manifest.author && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginAuthorLabel')}
                                  </p>
                                  <p>{plugin.manifest.author}</p>
                                </div>
                              )}
                              {plugin.manifest.license && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginLicenseLabel')}
                                  </p>
                                  <p>{plugin.manifest.license}</p>
                                </div>
                              )}
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginSourceLabel')}
                                </p>
                                <p>{formatSourceKind(plugin.installation.source.kind, t)}</p>
                              </div>
                              {plugin.installation.source.packageFormat && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginPackageFormatLabel')}
                                  </p>
                                  <p>
                                    {formatPackageFormat(
                                      plugin.installation.source.packageFormat,
                                      plugin.installation.source.packageFormatVersion,
                                    )}
                                  </p>
                                </div>
                              )}
                              {plugin.installation.source.builderSdkVersion && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginBuilderSdkVersionLabel')}
                                  </p>
                                  <p>v{plugin.installation.source.builderSdkVersion}</p>
                                </div>
                              )}
                              {plugin.installation.signatureStatus && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginSignatureTitle')}
                                  </p>
                                  <p>
                                    {formatSignatureStatus(plugin.installation.signatureStatus, t)}
                                  </p>
                                </div>
                              )}
                              {plugin.installation.signerFingerprint && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginSignerFingerprintLabel')}
                                  </p>
                                  <p>
                                    {formatSignerFingerprint(plugin.installation.signerFingerprint)}
                                  </p>
                                </div>
                              )}
                              {plugin.installation.signatureAlgorithm && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginSignatureAlgorithmLabel')}
                                  </p>
                                  <p>{plugin.installation.signatureAlgorithm}</p>
                                </div>
                              )}
                              {plugin.installation.signedAt && (
                                <div>
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginSignedAtLabel')}
                                  </p>
                                  <p>{plugin.installation.signedAt}</p>
                                </div>
                              )}
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginLanguageLabel')}
                                </p>
                                <p>{LANGUAGE_LABELS[plugin.manifest.runtime.language]}</p>
                              </div>
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginTimeoutLabel')}
                                </p>
                                <div className="mt-1 space-y-2">
                                  <div className="flex items-center gap-2">
                                    <Input
                                      type="number"
                                      min={1}
                                      value={getTimeoutDraftValue(plugin)}
                                      onChange={(event) =>
                                        setTimeoutDraftValue(plugin.manifest.id, event.target.value)
                                      }
                                      className="h-8 text-xs"
                                    />
                                    <Button
                                      variant="outline"
                                      size="sm"
                                      onClick={() => handleSavePluginTimeout(plugin)}
                                    >
                                      {t('download.pluginTimeoutSave')}
                                    </Button>
                                  </div>
                                  <p>
                                    {plugin.installation.timeoutSecOverride
                                      ? t('download.pluginTimeoutOverrideHelp', {
                                          seconds: plugin.manifest.timeoutSec,
                                        })
                                      : t('download.pluginTimeoutDefaultHelp')}
                                  </p>
                                  {plugin.installation.timeoutSecOverride && (
                                    <Button
                                      variant="outline"
                                      size="sm"
                                      onClick={() => handleResetPluginTimeout(plugin)}
                                    >
                                      {t('download.pluginTimeoutUseDefault')}
                                    </Button>
                                  )}
                                </div>
                              </div>
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginSupportedProvidersLabel')}
                                </p>
                                <p>
                                  {plugin.manifest.runtime.supportedProviders
                                    .map((provider) => PROVIDER_LABELS[provider])
                                    .join(', ')}
                                </p>
                              </div>
                              <div className="sm:col-span-2">
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginTriggersLabel')}
                                </p>
                                <p>{plugin.manifest.triggers.join(', ')}</p>
                              </div>
                              {plugin.manifest.homepage && (
                                <div className="sm:col-span-2">
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginHomepageLabel')}
                                  </p>
                                  <a
                                    href={plugin.manifest.homepage}
                                    target="_blank"
                                    rel="noreferrer"
                                    className="break-all text-primary hover:underline"
                                  >
                                    {plugin.manifest.homepage}
                                  </a>
                                </div>
                              )}
                              {plugin.manifest.repository && (
                                <div className="sm:col-span-2">
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginRepositoryLabel')}
                                  </p>
                                  <a
                                    href={plugin.manifest.repository}
                                    target="_blank"
                                    rel="noreferrer"
                                    className="break-all text-primary hover:underline"
                                  >
                                    {plugin.manifest.repository}
                                  </a>
                                </div>
                              )}
                              {plugin.manifest.publishedAt && (
                                <div className="sm:col-span-2">
                                  <p className="font-medium text-foreground/80">
                                    {t('download.pluginPublishedAtLabel')}
                                  </p>
                                  <p>{plugin.manifest.publishedAt}</p>
                                </div>
                              )}
                              <div className="sm:col-span-2">
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginLocationLabel')}
                                </p>
                                <p className="break-all">{plugin.installation.source.value}</p>
                              </div>
                              {plugin.installation.source.checksum && (
                                <div className="sm:col-span-2">
                                  <p className="font-medium text-foreground/80">
                                    {plugin.installation.source.packageFormat
                                      ? t('download.pluginPackageChecksumLabel')
                                      : t('download.pluginChecksumLabel')}
                                  </p>
                                  <p className="break-all">
                                    {plugin.installation.source.packageFormat
                                      ? plugin.installation.source.checksum
                                      : formatChecksum(plugin.installation.source.checksum)}
                                  </p>
                                </div>
                              )}
                            </div>
                          </div>

                          <div className="rounded-xl bg-muted/30 p-3">
                            <div className="flex items-center gap-2 text-xs font-medium">
                              <PackageOpen className="h-4 w-4 text-blue-500" />
                              <span>{t('download.pluginCompatibilityTitle')}</span>
                            </div>
                            <p className="mt-2 text-[11px] text-muted-foreground">
                              {t('download.pluginCompatibilityDesc')}
                            </p>
                            <div className="mt-3 space-y-3 text-[11px] text-muted-foreground">
                              <div>
                                <p className="font-medium text-foreground/80">
                                  {t('download.pluginProviderTitle')}
                                </p>
                                <p className="mt-1">{t('download.pluginProviderDesc')}</p>
                                <div className="mt-2">
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
                                        <SelectItem
                                          key={provider}
                                          value={provider}
                                          className="text-xs"
                                        >
                                          {PROVIDER_LABELS[provider]}
                                        </SelectItem>
                                      ))}
                                    </SelectContent>
                                  </Select>
                                </div>
                              </div>

                              <div className="space-y-1">
                                {compatibilityEntries.length > 0 ? (
                                  compatibilityEntries.map((entry) => <p key={entry}>{entry}</p>)
                                ) : (
                                  <p>{t('download.pluginCompatibilityNone')}</p>
                                )}
                              </div>

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
                              {(runtimeStatus?.status === 'error' ||
                                plugin.installation.lastError) && (
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
                              {plugin.warnings.length > 0 && (
                                <div className="flex flex-wrap gap-2 pt-1">
                                  {plugin.warnings.map((warning) => (
                                    <span
                                      key={`${plugin.manifest.id}-${warning}`}
                                      className="rounded bg-amber-500/10 px-2 py-1 text-[11px] text-amber-600 dark:text-amber-400"
                                    >
                                      {warning}
                                    </span>
                                  ))}
                                </div>
                              )}
                            </div>
                          </div>
                        </div>

                        <div className="rounded-xl bg-muted/30 p-3">
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
                            </div>
                          )}

                          {plugin.manifest.permissions.env.length > 0 && (
                            <div className="mt-4 space-y-3 border-t border-border/50 pt-3">
                              <div className="space-y-1">
                                <p className="text-xs font-medium">
                                  {t('download.pluginEnvTitle')}
                                </p>
                                <p className="text-[11px] text-muted-foreground">
                                  {t('download.pluginEnvDesc')}
                                </p>
                              </div>

                              {plugin.manifest.permissions.env.map((envKey) => {
                                const isSet = plugin.installation.envValueStatus[envKey] ?? false;
                                const draftValue = getEnvDraftValue(plugin.manifest.id, envKey);
                                const isSecret =
                                  envKey.includes('TOKEN') ||
                                  envKey.includes('SECRET') ||
                                  envKey.includes('KEY') ||
                                  envKey.includes('PASSWORD');

                                return (
                                  <div
                                    key={`${plugin.manifest.id}-${envKey}`}
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
                                            plugin.manifest.id,
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
                    </CollapsibleContent>
                  </div>
                </Collapsible>
              );
            })}
          </div>
        )}
      </SettingsCard>

      <div className="space-y-4">
        {WORKFLOW_TRIGGERS.map((trigger) => {
          const workflowPlugins = workflowPluginsByTrigger[trigger] ?? [];
          const availableWorkflowPlugins = availableWorkflowPluginsByTrigger[trigger] ?? [];
          const candidateValue = workflowCandidates[trigger] ?? '';

          return (
            <SettingsCard key={trigger} className="space-y-4">
              <div className="space-y-1">
                <p className="text-sm font-medium">
                  {t(`download.pluginWorkflowTrigger.${trigger}.title`)}
                </p>
                <p className="text-xs text-muted-foreground">
                  {t(`download.pluginWorkflowTrigger.${trigger}.desc`)}
                </p>
              </div>

              <div className="rounded-xl border border-dashed border-border/70 bg-background/40 p-4">
                <div className="flex flex-col gap-3 lg:flex-row lg:items-end">
                  <div className="min-w-0 flex-1 space-y-2">
                    <p className="text-xs font-medium">{t('download.pluginWorkflowAddLabel')}</p>
                    <Select
                      value={candidateValue}
                      onValueChange={(value) =>
                        setWorkflowCandidates((current) => ({ ...current, [trigger]: value }))
                      }
                    >
                      <SelectTrigger className="h-9 text-xs">
                        <SelectValue placeholder={t('download.pluginWorkflowAddPlaceholder')} />
                      </SelectTrigger>
                      <SelectContent>
                        {availableWorkflowPlugins.map((plugin) => (
                          <SelectItem
                            key={`${trigger}-${plugin.manifest.id}`}
                            value={plugin.manifest.id}
                            className="text-xs"
                          >
                            {plugin.manifest.name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <Button
                    variant="outline"
                    className="border-dashed"
                    onClick={() => handleAddWorkflowPlugin(trigger)}
                    disabled={!candidateValue}
                  >
                    <Plus className="h-4 w-4" />
                    {t('download.pluginWorkflowAddButton')}
                  </Button>
                </div>
              </div>

              {workflowPlugins.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border/70 bg-background/40 px-4 py-6 text-center">
                  <p className="text-sm font-medium">{t('download.pluginWorkflowEmptyTitle')}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {t('download.pluginWorkflowEmptyDesc')}
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {workflowPlugins.map(({ step, plugin }, index) => {
                    if (!plugin) return null;
                    return (
                      <div
                        key={`${trigger}-${plugin.manifest.id}`}
                        className="rounded-xl border border-border/60 bg-background/60 p-4"
                      >
                        <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                          <div className="min-w-0 flex-1 space-y-2">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="rounded bg-blue-500/10 px-2 py-0.5 text-[10px] uppercase tracking-wide text-blue-600 dark:text-blue-400">
                                {t('download.pluginWorkflowStepNumber', { index: index + 1 })}
                              </span>
                              <p className="truncate text-sm font-semibold">
                                {plugin.manifest.name}
                              </p>
                              <span className="rounded bg-muted px-2 py-0.5 text-[10px] tracking-wide text-muted-foreground">
                                v{plugin.manifest.version}
                              </span>
                            </div>
                            <p className="text-xs text-muted-foreground">
                              {plugin.manifest.description || t('download.pluginNoDescription')}
                            </p>
                          </div>

                          <div className="flex flex-wrap gap-2">
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() =>
                                handleMoveWorkflowStep(trigger, plugin.manifest.id, -1)
                              }
                              disabled={index === 0}
                            >
                              <MoveUp className="h-4 w-4" />
                              {t('download.pluginWorkflowMoveUp')}
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => handleMoveWorkflowStep(trigger, plugin.manifest.id, 1)}
                              disabled={index === workflowPlugins.length - 1}
                            >
                              <MoveDown className="h-4 w-4" />
                              {t('download.pluginWorkflowMoveDown')}
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => handleRemoveWorkflowStep(trigger, plugin.manifest.id)}
                            >
                              <Trash2 className="h-4 w-4" />
                              {t('download.pluginWorkflowRemove')}
                            </Button>
                          </div>
                        </div>

                        <div className="mt-4 grid gap-3 md:grid-cols-2">
                          <div className="space-y-2">
                            <p className="text-xs font-medium">
                              {t('download.pluginWorkflowStepOrder')}
                            </p>
                            <p className="text-xs text-muted-foreground">
                              {t('download.pluginWorkflowStepOrderHelp')}
                            </p>
                          </div>

                          <div className="space-y-2">
                            <p className="text-xs font-medium">
                              {t('download.pluginWorkflowFailureTitle')}
                            </p>
                            <Select
                              value={step.failurePolicy}
                              onValueChange={(value) =>
                                handleWorkflowFailurePolicy(
                                  trigger,
                                  plugin.manifest.id,
                                  value as PluginWorkflowFailurePolicy,
                                )
                              }
                            >
                              <SelectTrigger className="h-9 text-xs">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="continue" className="text-xs">
                                  {t('download.pluginWorkflowFailureContinue')}
                                </SelectItem>
                                <SelectItem value="stop-chain" className="text-xs">
                                  {t('download.pluginWorkflowFailureStopChain')}
                                </SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </SettingsCard>
          );
        })}
      </div>

      <Dialog
        open={createOpen}
        onOpenChange={(open) => {
          setCreateOpen(open);
          if (!open) {
            setCreatePluginForm(DEFAULT_CREATE_PLUGIN_FORM);
          }
        }}
      >
        <DialogContent className="sm:max-w-[760px]">
          <DialogHeader>
            <DialogTitle>{t('download.pluginCreateDialogTitle')}</DialogTitle>
          </DialogHeader>

          <div className="max-h-[80vh] space-y-4 overflow-y-auto pr-1">
            <p className="text-sm text-muted-foreground">{t('download.pluginCreateDialogDesc')}</p>

            <div className="space-y-4 rounded-xl border border-border/60 bg-muted/20 p-4">
              <p className="text-sm font-medium">{t('download.pluginCreateDetailsTitle')}</p>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginCreateNameLabel')}</p>
                  <Input
                    value={createPluginForm.name}
                    onChange={(event) => updateCreatePluginForm('name', event.target.value)}
                    placeholder={t('download.pluginNamePlaceholder')}
                  />
                </div>

                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginWorkspacePathLabel')}</p>
                  <div className="flex gap-2">
                    <Input
                      value={createPluginForm.destinationRoot}
                      onChange={(event) =>
                        updateCreatePluginForm('destinationRoot', event.target.value)
                      }
                      placeholder={t('download.pluginCreateLocationPlaceholder')}
                    />
                    <Button variant="outline" type="button" onClick={handlePickWorkspaceRoot}>
                      {t('download.pluginCreateBrowse')}
                    </Button>
                  </div>
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginCreateIdLabel')}</p>
                  <Input
                    value={createPluginForm.id}
                    onChange={(event) => updateCreatePluginForm('id', event.target.value)}
                    placeholder={t('download.pluginCreateIdPlaceholder')}
                  />
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginCreateSlugLabel')}</p>
                  <Input
                    value={createPluginForm.slug}
                    onChange={(event) => updateCreatePluginForm('slug', event.target.value)}
                    placeholder={t('download.pluginSlugPlaceholder')}
                  />
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginVersionLabel')}</p>
                  <Input
                    value={createPluginForm.version}
                    onChange={(event) => updateCreatePluginForm('version', event.target.value)}
                    placeholder="0.1.0"
                  />
                </div>

                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginLicenseLabel')}</p>
                  <Input
                    value={createPluginForm.license}
                    onChange={(event) => updateCreatePluginForm('license', event.target.value)}
                    placeholder="MIT"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <p className="text-sm font-medium">{t('download.pluginDescriptionLabel')}</p>
                <Textarea
                  value={createPluginForm.description}
                  onChange={(event) => updateCreatePluginForm('description', event.target.value)}
                  placeholder={t('download.pluginCreateDescriptionPlaceholder')}
                  rows={3}
                />
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginAuthorLabel')}</p>
                  <Input
                    value={createPluginForm.author}
                    onChange={(event) => updateCreatePluginForm('author', event.target.value)}
                    placeholder={t('download.pluginCreateAuthorPlaceholder')}
                  />
                </div>

                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginTimeoutLabel')}</p>
                  <Input
                    type="number"
                    min={1}
                    value={createPluginForm.timeoutSec}
                    onChange={(event) => updateCreatePluginForm('timeoutSec', event.target.value)}
                    placeholder="60"
                  />
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginHomepageLabel')}</p>
                  <Input
                    value={createPluginForm.homepage}
                    onChange={(event) => updateCreatePluginForm('homepage', event.target.value)}
                    placeholder="https://example.com"
                  />
                </div>

                <div className="space-y-2">
                  <p className="text-sm font-medium">{t('download.pluginRepositoryLabel')}</p>
                  <Input
                    value={createPluginForm.repository}
                    onChange={(event) => updateCreatePluginForm('repository', event.target.value)}
                    placeholder="https://github.com/example/plugin"
                  />
                </div>
              </div>
            </div>

            <div className="space-y-4 rounded-xl border border-border/60 bg-muted/20 p-4">
              <div className="space-y-1">
                <p className="text-sm font-medium">{t('download.pluginCreateRuntimeTitle')}</p>
                <p className="text-xs text-muted-foreground">
                  {t('download.pluginCreateRuntimeHelp')}
                </p>
              </div>

              <div className="space-y-2">
                <p className="text-sm font-medium">{t('download.pluginSupportedProvidersLabel')}</p>
                <div className="flex flex-wrap gap-2">
                  {(['deno', 'node', 'bun'] as PluginProvider[]).map((provider) => {
                    const selected = createPluginForm.supportedProviders.includes(provider);
                    return (
                      <button
                        key={provider}
                        type="button"
                        onClick={() => toggleCreatePluginProvider(provider)}
                        className={cn(
                          'rounded-md border border-dashed px-3 py-1.5 text-xs transition-colors',
                          selected
                            ? 'border-primary bg-primary/10 text-primary'
                            : 'border-border text-muted-foreground hover:bg-muted/60',
                        )}
                      >
                        {PROVIDER_LABELS[provider]}
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="space-y-2">
                <p className="text-sm font-medium">{t('download.pluginProviderTitle')}</p>
                <Select
                  value={createPluginForm.preferredProvider}
                  onValueChange={(value) =>
                    updateCreatePluginForm('preferredProvider', value as PluginProvider)
                  }
                >
                  <SelectTrigger className="h-10 text-sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {(createPluginForm.supportedProviders.length > 0
                      ? createPluginForm.supportedProviders
                      : DEFAULT_CREATE_PLUGIN_FORM.supportedProviders
                    ).map((provider) => (
                      <SelectItem key={provider} value={provider}>
                        {PROVIDER_LABELS[provider]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <p className="text-sm font-medium">{t('download.pluginTriggersLabel')}</p>
                <div className="flex flex-wrap gap-2">
                  {WORKFLOW_TRIGGERS.map((trigger) => {
                    const selected = createPluginForm.triggers.includes(trigger);
                    return (
                      <button
                        key={trigger}
                        type="button"
                        onClick={() => toggleCreatePluginTrigger(trigger)}
                        className={cn(
                          'rounded-md border border-dashed px-3 py-1.5 text-xs transition-colors',
                          selected
                            ? 'border-primary bg-primary/10 text-primary'
                            : 'border-border text-muted-foreground hover:bg-muted/60',
                        )}
                      >
                        {t(`download.pluginWorkflowTrigger.${trigger}.title`)}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>

            <div className="space-y-4 rounded-xl border border-border/60 bg-muted/20 p-4">
              <div className="space-y-1">
                <p className="text-sm font-medium">{t('download.pluginPermissionsTitle')}</p>
                <p className="text-xs text-muted-foreground">
                  {t('download.pluginCreatePermissionsHelp')}
                </p>
              </div>

              <div className="flex items-center justify-between rounded-lg border border-border/60 px-3 py-2">
                <span className="text-sm">{t('download.pluginPermissionNetwork')}</span>
                <Switch
                  checked={createPluginForm.permissionNetwork}
                  onCheckedChange={(checked) =>
                    updateCreatePluginForm('permissionNetwork', checked)
                  }
                />
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-sm font-medium">
                    {t('download.pluginPermissionReadPathsLabel')}
                  </p>
                  <Textarea
                    value={createPluginForm.permissionReadPaths}
                    onChange={(event) =>
                      updateCreatePluginForm('permissionReadPaths', event.target.value)
                    }
                    placeholder={t('download.pluginCreateListPlaceholder')}
                    rows={4}
                  />
                </div>

                <div className="space-y-2">
                  <p className="text-sm font-medium">
                    {t('download.pluginPermissionWritePathsLabel')}
                  </p>
                  <Textarea
                    value={createPluginForm.permissionWritePaths}
                    onChange={(event) =>
                      updateCreatePluginForm('permissionWritePaths', event.target.value)
                    }
                    placeholder={t('download.pluginCreateListPlaceholder')}
                    rows={4}
                  />
                </div>
              </div>

              <div className="space-y-2">
                <p className="text-sm font-medium">{t('download.pluginPermissionEnvLabel')}</p>
                <Textarea
                  value={createPluginForm.permissionEnv}
                  onChange={(event) => updateCreatePluginForm('permissionEnv', event.target.value)}
                  placeholder={t('download.pluginCreateListPlaceholder')}
                  rows={4}
                />
              </div>
            </div>

            <p className="text-xs text-muted-foreground">{t('download.pluginCreateHelp')}</p>

            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => {
                  setCreateOpen(false);
                  setCreatePluginForm(DEFAULT_CREATE_PLUGIN_FORM);
                }}
                disabled={creating}
              >
                {t('download.pluginDismiss')}
              </Button>
              <Button
                onClick={handleCreatePlugin}
                disabled={
                  creating ||
                  !createPluginForm.name.trim() ||
                  !createPluginForm.destinationRoot.trim()
                }
              >
                <Plus className="h-4 w-4" />
                {creating ? t('download.pluginCreating') : t('download.pluginCreateWorkspace')}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog
        open={attachWorkspacePath != null}
        onOpenChange={(open) => {
          if (!open) {
            setAttachWorkspacePath(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t('download.pluginAttachWorkspace')}</DialogTitle>
          </DialogHeader>

          <div className="space-y-3">
            <p className="text-sm text-muted-foreground">
              {t('download.pluginAttachWorkspaceConfirm')}
            </p>
            {attachWorkspacePath && (
              <div className="rounded-lg border border-border/60 bg-muted/30 p-3 text-xs text-muted-foreground break-all">
                {attachWorkspacePath}
              </div>
            )}
          </div>

          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setAttachWorkspacePath(null)}>
              {t('download.pluginDismiss')}
            </Button>
            <Button onClick={handleConfirmAttachWorkspace}>
              <Braces className="h-4 w-4" />
              {t('download.pluginAttachWorkspace')}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog
        open={uninstallTarget != null}
        onOpenChange={(open) => {
          if (!open) {
            setUninstallTarget(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>
              {t(
                uninstallTarget?.installation.source.kind === 'workspace'
                  ? 'download.pluginDetachWorkspace'
                  : 'download.pluginUninstall',
              )}
            </DialogTitle>
          </DialogHeader>

          {uninstallTarget && (
            <div className="space-y-3">
              <p className="text-sm text-muted-foreground">
                {t(
                  uninstallTarget.installation.source.kind === 'workspace'
                    ? 'download.pluginDetachWorkspaceConfirm'
                    : 'download.pluginUninstallConfirm',
                  { name: uninstallTarget.manifest.name },
                )}
              </p>
              <div className="rounded-lg border border-border/60 bg-muted/30 p-3 text-xs text-muted-foreground">
                {uninstallTarget.manifest.name}
              </div>
            </div>
          )}

          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setUninstallTarget(null)}>
              {t('download.pluginDismiss')}
            </Button>
            <Button
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={handleConfirmUninstallPlugin}
            >
              <Trash2 className="h-4 w-4" />
              {t(
                uninstallTarget?.installation.source.kind === 'workspace'
                  ? 'download.pluginDetachWorkspace'
                  : 'download.pluginUninstall',
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={runtimeGuideOpen} onOpenChange={setRuntimeGuideOpen}>
        <DialogContent className="sm:max-w-[640px]">
          <DialogHeader>
            <DialogTitle>{t('download.pluginRuntimeGuideTitle')}</DialogTitle>
          </DialogHeader>

          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">{t('download.pluginRuntimeGuideDesc')}</p>

            <div className="grid gap-3 lg:grid-cols-2">
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
                  <div key={language} className="rounded-xl border border-border/60 p-3">
                    <div className="space-y-1">
                      <p className="text-sm font-medium">{LANGUAGE_LABELS[language]}</p>
                      <p className="text-xs text-muted-foreground">
                        {t('download.pluginRuntimeDefault')}
                      </p>
                    </div>

                    <div className="mt-3">
                      <Select
                        value={defaultProviders[language] ?? allowedProviders[0].provider}
                        onValueChange={(value) =>
                          handleSetDefaultProvider(language, value as PluginProvider)
                        }
                      >
                        <SelectTrigger className="h-9 text-xs">
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

            <div className="flex flex-wrap gap-2">
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

            <div className="rounded-xl bg-muted/30 p-3 text-[11px] text-muted-foreground">
              <p>{t('download.pluginRuntimeGuideNotePrimary')}</p>
              <p className="mt-1">{t('download.pluginRuntimeGuideNoteSecondary')}</p>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog
        open={permissionDialogPlugin != null}
        onOpenChange={(open) => {
          if (!open) {
            setPermissionDialogPlugin(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t('download.pluginEnablePermissionsTitle')}</DialogTitle>
          </DialogHeader>

          {permissionDialogPlugin && (
            <div className="space-y-4">
              <p className="text-sm text-muted-foreground">
                {t('download.pluginEnablePermissionsDesc', {
                  name: permissionDialogPlugin.manifest.name,
                })}
              </p>

              <div className="space-y-2">
                <p className="text-sm font-medium">{t('download.pluginRequestedPermissions')}</p>
                <div className="grid gap-2 md:grid-cols-2">
                  {[
                    {
                      key: 'network' as const,
                      label: t('download.pluginPermissionNetwork'),
                      enabled: permissionDialogPlugin.manifest.permissions.network,
                    },
                    {
                      key: 'readPaths' as const,
                      label: t('download.pluginPermissionReadPaths'),
                      enabled: permissionDialogPlugin.manifest.permissions.readPaths.length > 0,
                    },
                    {
                      key: 'writePaths' as const,
                      label: t('download.pluginPermissionWritePaths'),
                      enabled: permissionDialogPlugin.manifest.permissions.writePaths.length > 0,
                    },
                    {
                      key: 'env' as const,
                      label: t('download.pluginPermissionEnv'),
                      enabled: permissionDialogPlugin.manifest.permissions.env.length > 0,
                    },
                  ]
                    .filter((permission) => permission.enabled)
                    .map((permission) => (
                      <div
                        key={permission.key}
                        className="flex items-center justify-between rounded-lg border border-border/60 px-3 py-2"
                      >
                        <span className="text-sm">{permission.label}</span>
                        <Switch
                          checked={permissionDialogState[permission.key]}
                          onCheckedChange={(checked) =>
                            setPermissionDialogState((current) => ({
                              ...current,
                              [permission.key]: checked,
                            }))
                          }
                        />
                      </div>
                    ))}
                </div>
              </div>

              <p className="text-xs text-muted-foreground">
                {t('download.pluginEnablePermissionsHelp')}
              </p>

              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setPermissionDialogPlugin(null)}>
                  {t('download.pluginDismiss')}
                </Button>
                <Button onClick={handleEnablePluginWithPermissions}>
                  {t('download.pluginEnableWithPermissions')}
                </Button>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <PluginLogsDialog
        open={logsOpen}
        onOpenChange={(open) => {
          setLogsOpen(open);
          if (!open) {
            setSelectedPluginId(null);
            setPluginLogs([]);
            setPluginLogsTotal(0);
            setPluginLogsHasMore(false);
            setPluginLogsOffset(0);
            setPluginLogsError(null);
          }
        }}
        plugin={selectedPlugin}
        logs={pluginLogs}
        total={pluginLogsTotal}
        loading={logsLoading}
        loadingMore={logsLoadingMore}
        clearing={logsClearing}
        hasMore={pluginLogsHasMore}
        error={pluginLogsError}
        onRefresh={() =>
          selectedPluginId ? loadPluginLogs(selectedPluginId, 'replace') : undefined
        }
        onLoadMore={handleLoadMorePluginLogs}
        onClear={handleClearPluginLogs}
      />

      <Dialog
        open={clearLogsConfirmOpen}
        onOpenChange={(open) => {
          if (!open) {
            setClearLogsConfirmOpen(false);
          }
        }}
      >
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>{t('download.pluginLogsClear')}</DialogTitle>
          </DialogHeader>

          <p className="text-sm text-muted-foreground">{t('download.pluginLogsClearConfirm')}</p>

          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setClearLogsConfirmOpen(false)}>
              {t('download.pluginDismiss')}
            </Button>
            <Button
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={handleConfirmClearPluginLogs}
              disabled={logsClearing}
            >
              <Trash2 className="h-4 w-4" />
              {t('download.pluginLogsClear')}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
