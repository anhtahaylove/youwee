import { invoke } from '@tauri-apps/api/core';
import type { PluginSummary } from '@/lib/types';

const STORAGE_KEY = 'youwee-post-download-plugins';

export function loadEnabledPostDownloadPlugins(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((value): value is string => typeof value === 'string' && value.length > 0);
  } catch (error) {
    console.error('Failed to load post-download plugins:', error);
    return [];
  }
}

export function saveEnabledPostDownloadPlugins(pluginIds: string[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...new Set(pluginIds)]));
  } catch (error) {
    console.error('Failed to save post-download plugins:', error);
  }
}

export async function refreshEnabledPostDownloadPlugins(): Promise<string[]> {
  try {
    const plugins = await invoke<PluginSummary[]>('list_plugins');
    const enabledIds = plugins
      .filter((plugin) => plugin.installation.enabled)
      .map((plugin) => plugin.manifest.pluginId);
    saveEnabledPostDownloadPlugins(enabledIds);
    return enabledIds;
  } catch (error) {
    console.error('Failed to refresh post-download plugins:', error);
    return loadEnabledPostDownloadPlugins();
  }
}
