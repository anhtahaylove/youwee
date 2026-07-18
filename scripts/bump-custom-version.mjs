import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const APP_VERSION_FILES = ['package.json', 'src-tauri/Cargo.toml', 'src-tauri/tauri.conf.json'];
const CHANGELOG_FILES = ['CHANGELOG.md', 'docs/CHANGELOG.vi.md', 'docs/CHANGELOG.zh-CN.md'];
const EXTENSION_MANIFEST_FILES = [
  'extensions/youwee-webext/manifest.chromium.json',
  'extensions/youwee-webext/manifest.firefox.json',
];
const CUSTOM_VERSION_PATTERN = /^(\d+)\.(\d+)\.(\d+)-custom\.(\d+)$/;

function usage() {
  return `Usage: bun run version:bump -- <version> [--date YYYY-MM-DD] [--write]

Examples:
  bun run version:bump -- 0.19.1-custom.42
  bun run version:bump -- 0.19.1-custom.42 --date 2026-07-19 --write

The command is a dry run unless --write is provided. It updates the three app
version files and promotes the current Unreleased section in all three changelogs.
The browser extension store version remains independent.`;
}

function parseCustomVersion(version) {
  const match = CUSTOM_VERSION_PATTERN.exec(version);
  if (!match) {
    throw new Error(`Expected a custom version such as 0.19.1-custom.42, got '${version}'.`);
  }
  return match.slice(1).map(Number);
}

function assertValidDate(date) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) {
    throw new Error(`Expected --date in YYYY-MM-DD format, got '${date}'.`);
  }
  const parsed = new Date(`${date}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== date) {
    throw new Error(`Invalid release date '${date}'.`);
  }
}

function compareVersions(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return left[index] - right[index];
    }
  }
  return 0;
}

export function deriveWindowsInstallerVersion(version) {
  const parts = parseCustomVersion(version);
  const limits = [255, 255, 65535, 65535];
  if (parts.some((part, index) => part > limits[index])) {
    throw new Error(`Version '${version}' exceeds Windows Installer field limits.`);
  }
  return parts.join('.');
}

function replaceExactlyOnce(content, search, replacement, file) {
  if (content.split(search).length !== 2) {
    throw new Error(`Expected exactly one '${search}' in ${file}.`);
  }
  return content.replace(search, replacement);
}

export function updateAppVersionFile(file, content, currentVersion, nextVersion) {
  const search = file.endsWith('Cargo.toml')
    ? `version = "${currentVersion}"`
    : `"version": "${currentVersion}"`;
  const replacement = file.endsWith('Cargo.toml')
    ? `version = "${nextVersion}"`
    : `"version": "${nextVersion}"`;
  return replaceExactlyOnce(content, search, replacement, file);
}

export function promoteUnreleased(content, nextVersion, date, file = 'CHANGELOG.md') {
  const heading = '## [Unreleased]';
  if (content.includes(`## [${nextVersion}]`)) {
    throw new Error(`${file} already contains version ${nextVersion}.`);
  }
  const eol = content.includes('\r\n') ? '\r\n' : '\n';
  return replaceExactlyOnce(
    content,
    heading,
    `${heading}${eol}${eol}## [${nextVersion}] - ${date}`,
    file,
  );
}

export function parseArgs(args) {
  if (args.includes('--help') || args.includes('-h')) {
    return { help: true };
  }

  let version;
  let date = new Date().toISOString().slice(0, 10);
  let write = false;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--write') {
      write = true;
      continue;
    }
    if (arg === '--date') {
      date = args[index + 1];
      if (!date || date.startsWith('--')) {
        throw new Error('--date requires a YYYY-MM-DD value.');
      }
      index += 1;
      continue;
    }
    if (arg.startsWith('--')) {
      throw new Error(`Unknown option '${arg}'.`);
    }
    if (version) {
      throw new Error(`Unexpected positional argument '${arg}'.`);
    }
    version = arg;
  }

  if (!version) {
    throw new Error('Missing target version.');
  }

  return { date, help: false, version, write };
}

async function readJson(root, file) {
  return JSON.parse(await readFile(path.join(root, file), 'utf8'));
}

export async function createVersionBumpPlan(root, nextVersion, date) {
  const nextParts = parseCustomVersion(nextVersion);
  assertValidDate(date);

  const packageJson = await readJson(root, 'package.json');
  const cargoToml = await readFile(path.join(root, 'src-tauri/Cargo.toml'), 'utf8');
  const tauriConfig = await readJson(root, 'src-tauri/tauri.conf.json');
  const cargoVersion = /^version\s*=\s*"([^"]+)"/m.exec(cargoToml)?.[1];
  const currentVersions = [packageJson.version, cargoVersion, tauriConfig.version];
  if (currentVersions.some((version) => typeof version !== 'string')) {
    throw new Error('Could not read all three app versions.');
  }
  if (new Set(currentVersions).size !== 1) {
    throw new Error(`App versions are out of sync: ${currentVersions.join(', ')}.`);
  }

  const currentVersion = currentVersions[0];
  const currentParts = parseCustomVersion(currentVersion);
  if (compareVersions(nextParts, currentParts) <= 0) {
    throw new Error(`Target version ${nextVersion} must be newer than ${currentVersion}.`);
  }

  const extensionManifests = await Promise.all(
    EXTENSION_MANIFEST_FILES.map((file) => readJson(root, file)),
  );
  const extensionVersions = extensionManifests.map((manifest) => manifest.version);
  if (extensionVersions.some((version) => typeof version !== 'string')) {
    throw new Error('Could not read both extension versions.');
  }
  if (new Set(extensionVersions).size !== 1) {
    throw new Error(`Extension versions are out of sync: ${extensionVersions.join(', ')}.`);
  }

  const changes = [];
  for (const file of APP_VERSION_FILES) {
    const before = await readFile(path.join(root, file), 'utf8');
    changes.push({
      after: updateAppVersionFile(file, before, currentVersion, nextVersion),
      before,
      file,
    });
  }
  for (const file of CHANGELOG_FILES) {
    const before = await readFile(path.join(root, file), 'utf8');
    changes.push({ after: promoteUnreleased(before, nextVersion, date, file), before, file });
  }

  return {
    changes,
    currentVersion,
    extensionVersion: extensionVersions[0],
    nextVersion,
    windowsInstallerVersion: deriveWindowsInstallerVersion(nextVersion),
  };
}

async function run() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(usage());
    return;
  }

  const root = process.cwd();
  const plan = await createVersionBumpPlan(root, options.version, options.date);
  const changedFiles = plan.changes.filter(({ after, before }) => after !== before);

  console.log(`${options.write ? 'Applying' : 'Previewing'} Youwee custom version bump:`);
  console.log(`- App: ${plan.currentVersion} -> ${plan.nextVersion}`);
  console.log(
    `- Windows Installer: ${plan.windowsInstallerVersion} (generated during deps:prepare:windows)`,
  );
  console.log(`- Extension store: ${plan.extensionVersion} (unchanged)`);
  for (const { file } of changedFiles) {
    console.log(`- ${options.write ? 'Updated' : 'Would update'} ${file}`);
  }

  if (!options.write) {
    console.log('Dry run only. Add --write after reviewing this plan.');
    return;
  }

  await Promise.all(
    changedFiles.map(({ after, file }) => writeFile(path.join(root, file), after, 'utf8')),
  );
  console.log('Run cargo check to refresh Cargo.lock, then review all three changelogs.');
}

if (import.meta.main) {
  run().catch((error) => {
    console.error(error.message);
    console.error(usage());
    process.exit(1);
  });
}
