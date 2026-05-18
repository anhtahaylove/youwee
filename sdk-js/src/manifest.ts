import { SDK_VERSION, satisfiesVersionRange } from './compatibility';
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

const ALLOWED_TRIGGERS = new Set([
  'download.queued',
  'download.beforeStart',
  'download.completed',
  'download.failed',
]);

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

  if (!manifest.id?.trim()) {
    errors.push('id is required.');
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

  if (!manifest.triggers?.length) {
    errors.push('triggers must contain at least one runtime trigger string.');
  } else {
    for (const trigger of manifest.triggers) {
      if (!ALLOWED_TRIGGERS.has(trigger)) {
        if (trigger.startsWith('triggers.')) {
          errors.push(
            `triggers contains "${trigger}", but plugin.json must use raw runtime names like "download.completed", not SDK identifiers like "triggers.downloadCompleted".`,
          );
        } else {
          errors.push(`triggers contains unsupported runtime trigger "${trigger}".`);
        }
      }
    }
  }

  if (manifest.i18n?.defaultLocale) {
    if (
      manifest.i18n.supportedLocales?.length &&
      !manifest.i18n.supportedLocales.includes(manifest.i18n.defaultLocale)
    ) {
      errors.push('i18n.defaultLocale must be included in i18n.supportedLocales.');
    }
  }

  if (manifest.i18n?.directory) {
    const directory = manifest.i18n.directory.trim();
    if (!directory) {
      errors.push('i18n.directory cannot be empty.');
    } else if (directory.startsWith('/') || directory.split(/[\\/]/).includes('..')) {
      errors.push('i18n.directory must stay inside the plugin package.');
    }
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
  const sdkVersion = input.sdkVersion || `^${SDK_VERSION}`;

  return {
    name: input.name,
    version: input.version,
    private: true,
    description: input.description || 'Youwee plugin package',
    type: 'commonjs',
    main,
    scripts: {
      'test:node':
        'YOUWEE_PLUGIN_MAIN=src/plugin.js node node_modules/youwee-sdk/dist/runtime-cli.js',
      'test:bun':
        'YOUWEE_PLUGIN_MAIN=src/plugin.js bun node_modules/youwee-sdk/dist/runtime-cli.js',
    },
    dependencies: {
      'youwee-sdk': sdkVersion,
    },
  };
}

export function createPluginPackageJson(input: PluginPackageDefinitionInput): string {
  return `${JSON.stringify(createPluginPackageDefinition(input), null, 2)}\n`;
}
