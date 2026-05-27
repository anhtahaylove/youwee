import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { runPluginModule } from './runtime';
import type { PluginDefinition } from './types';

const STRIP_RUNTIME_ENV_KEYS = [
  'DYLD_FALLBACK_LIBRARY_PATH',
  'DYLD_LIBRARY_PATH',
  'LD_LIBRARY_PATH',
] as const;

const nativeDynamicImport = new Function('specifier', 'return import(specifier)') as (
  specifier: string,
) => Promise<unknown>;

function sanitizeRuntimeEnvironment(): void {
  for (const key of STRIP_RUNTIME_ENV_KEYS) {
    try {
      delete process.env[key];
    } catch {
      // Ignore environment mutation failures and continue.
    }
  }

  const denoEnv = (
    globalThis as typeof globalThis & {
      Deno?: {
        env?: {
          delete?: (key: string) => void;
        };
      };
    }
  ).Deno?.env;

  if (!denoEnv?.delete) {
    return;
  }

  for (const key of STRIP_RUNTIME_ENV_KEYS) {
    try {
      denoEnv.delete(key);
    } catch {
      // Ignore environment mutation failures and continue.
    }
  }
}

async function loadPluginModule(): Promise<PluginDefinition | { default?: PluginDefinition }> {
  const pluginMain = process.argv[2] || process.env.YOUWEE_PLUGIN_MAIN;

  if (!pluginMain) {
    throw new Error(
      'Plugin entrypoint is not set. Pass it as the first argument or set YOUWEE_PLUGIN_MAIN.',
    );
  }

  const resolvedPluginMain = resolve(pluginMain);
  if (/\.(mjs|mts|ts)$/i.test(resolvedPluginMain)) {
    return (await nativeDynamicImport(pathToFileURL(resolvedPluginMain).href)) as
      | PluginDefinition
      | { default?: PluginDefinition };
  }

  return require(resolvedPluginMain) as PluginDefinition | { default?: PluginDefinition };
}

async function main(): Promise<void> {
  sanitizeRuntimeEnvironment();
  const plugin = await loadPluginModule();
  await runPluginModule(plugin);
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${message}\n`);
  process.exit(1);
});
