export interface UpdaterManifestPlatform {
  signature: string;
  url: string;
}

export interface UpdaterManifest {
  version: string;
  notes?: string;
  notes_vi?: string;
  'notes_zh-CN'?: string;
  pub_date: string;
  platforms: Record<string, UpdaterManifestPlatform>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function requireNonEmptyString(value: unknown, path: string): string {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`Invalid updater manifest: ${path} must be a non-empty string`);
  }
  return value;
}

function optionalString(value: unknown, path: string): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== 'string') {
    throw new Error(`Invalid updater manifest: ${path} must be a string`);
  }
  return value;
}

export function parseUpdaterManifest(value: unknown): UpdaterManifest {
  if (!isRecord(value)) {
    throw new Error('Invalid updater manifest: root must be an object');
  }

  const version = requireNonEmptyString(value.version, 'version');
  const pubDate = requireNonEmptyString(value.pub_date, 'pub_date');
  if (Number.isNaN(Date.parse(pubDate))) {
    throw new Error('Invalid updater manifest: pub_date must be an ISO date');
  }
  if (!isRecord(value.platforms) || Object.keys(value.platforms).length === 0) {
    throw new Error('Invalid updater manifest: platforms must be a non-empty object');
  }

  const platforms: Record<string, UpdaterManifestPlatform> = {};
  for (const [platformName, platformValue] of Object.entries(value.platforms)) {
    if (!isRecord(platformValue)) {
      throw new Error(`Invalid updater manifest: platforms.${platformName} must be an object`);
    }

    const signature = requireNonEmptyString(
      platformValue.signature,
      `platforms.${platformName}.signature`,
    );
    const url = requireNonEmptyString(platformValue.url, `platforms.${platformName}.url`);
    try {
      if (new URL(url).protocol !== 'https:') throw new Error('not https');
    } catch {
      throw new Error(`Invalid updater manifest: platforms.${platformName}.url must use HTTPS`);
    }
    platforms[platformName] = { signature, url };
  }

  return {
    version,
    notes: optionalString(value.notes, 'notes'),
    notes_vi: optionalString(value.notes_vi, 'notes_vi'),
    'notes_zh-CN': optionalString(value['notes_zh-CN'], 'notes_zh-CN'),
    pub_date: pubDate,
    platforms,
  };
}
