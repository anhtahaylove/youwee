import { satisfiesVersionRange } from './compatibility';
import type {
  ManifestValidationResult,
  PluginManifest,
  PluginPackageDefinitionInput,
  PluginProvider,
  PluginRuntimeLanguage,
} from './types';

const PROVIDERS_BY_LANGUAGE: Record<PluginRuntimeLanguage, PluginProvider[]> = {
  javascript: ['deno', 'node', 'bun'],
  python: ['python'],
};

export function slugifyPluginName(input: string): string {
  let slug = '';
  let previousDash = false;

  for (const char of input.trim()) {
    if (/^[a-z0-9]$/i.test(char)) {
      slug += char.toLowerCase();
      previousDash = false;
      continue;
    }

    if (!previousDash) {
      slug += '-';
      previousDash = true;
    }
  }

  const normalized = slug.replace(/^-+|-+$/g, '');
  return normalized || 'plugin';
}

export function getAllowedProviders(language: PluginRuntimeLanguage): PluginProvider[] {
  return [...PROVIDERS_BY_LANGUAGE[language]];
}

export function getManifestValidationErrors(manifest: PluginManifest): string[] {
  const errors: string[] = [];

  if (!manifest.pluginId?.trim()) {
    errors.push('pluginId is required.');
  }

  if (!manifest.slug?.trim()) {
    errors.push('slug is required.');
  }

  if (!manifest.name?.trim()) {
    errors.push('name is required.');
  }

  if (!manifest.version?.trim()) {
    errors.push('version is required.');
  }

  if (!manifest.runtime) {
    errors.push('runtime is required.');
    return errors;
  }

  if (!manifest.runtime.entrypoint?.trim()) {
    errors.push('runtime.entrypoint is required.');
  }

  if (!manifest.runtime.supportedProviders?.length) {
    errors.push('runtime.supportedProviders must contain at least one provider.');
  } else {
    const allowedProviders = new Set(getAllowedProviders(manifest.runtime.language));
    for (const provider of manifest.runtime.supportedProviders) {
      if (!allowedProviders.has(provider)) {
        errors.push(
          `runtime.supportedProviders contains unsupported provider "${provider}" for language "${manifest.runtime.language}".`,
        );
      }
    }
  }

  if (
    manifest.runtime.preferredProvider &&
    !manifest.runtime.supportedProviders?.includes(manifest.runtime.preferredProvider)
  ) {
    errors.push('runtime.preferredProvider must be included in runtime.supportedProviders.');
  }

  if (typeof manifest.timeoutSec === 'number' && manifest.timeoutSec <= 0) {
    errors.push('timeoutSec must be greater than 0.');
  }

  if (manifest.compatibility?.appVersion) {
    try {
      satisfiesVersionRange('0.0.0', manifest.compatibility.appVersion);
    } catch (error) {
      errors.push(
        `compatibility.appVersion is invalid: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  if (manifest.compatibility?.sdkVersion) {
    try {
      satisfiesVersionRange('0.0.0', manifest.compatibility.sdkVersion);
    } catch (error) {
      errors.push(
        `compatibility.sdkVersion is invalid: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  return errors;
}

export function validatePluginManifest(manifest: PluginManifest): ManifestValidationResult {
  const errors = getManifestValidationErrors(manifest);
  return {
    valid: errors.length === 0,
    errors,
  };
}

export function createPluginPackageDefinition(
  input: PluginPackageDefinitionInput,
): Record<string, unknown> {
  const main = input.main || 'src/plugin.js';

  return {
    name: input.name,
    version: input.version,
    private: true,
    description: input.description || 'Youwee plugin package',
    type: 'commonjs',
    main,
    scripts: {
      'test:node':
        'NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js node vendor/youwee-sdk/dist/runtime-cli.js',
      'test:bun':
        'NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js bun vendor/youwee-sdk/dist/runtime-cli.js',
    },
    dependencies: {
      'youwee-sdk': 'file:vendor/youwee-sdk',
    },
  };
}

export function createPluginPackageJson(input: PluginPackageDefinitionInput): string {
  return `${JSON.stringify(createPluginPackageDefinition(input), null, 2)}\n`;
}
