import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { SDK_VERSION } from './compatibility';
import { validatePluginManifest } from './manifest';
import type {
  BuildPluginPackageInput,
  BuildPluginPackageResult,
  PackagedPluginBuildInfo,
  PackagedPluginChecksums,
  PackPluginPackageInput,
  PackPluginPackageResult,
  PluginManifest,
} from './types';

const PACKAGE_FORMAT = 'ywp' as const;
const PACKAGE_FORMAT_VERSION = 1 as const;
const PACKAGED_ENTRYPOINT = 'dist/plugin.cjs';

type ZipEntry = {
  path: string;
  bytes: Uint8Array;
};

function getBunExecutable(): string {
  const execPath = process.execPath;
  if (!execPath) {
    throw new Error('youwee-sdk build/pack commands require Bun runtime.');
  }
  return execPath;
}

function readJsonFile<T>(path: string): T {
  return JSON.parse(readFileSync(path, 'utf8')) as T;
}

function sha256(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}

function normalizeArchivePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/^\.?\//, '');
}

function collectFiles(rootDir: string, relativeDir: string): string[] {
  const absoluteDir = join(rootDir, relativeDir);
  if (!existsSync(absoluteDir) || !statSync(absoluteDir).isDirectory()) {
    return [];
  }

  const files: string[] = [];
  const stack = [absoluteDir];

  while (stack.length > 0) {
    const current = stack.pop();
    if (!current) {
      continue;
    }
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const absolutePath = join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolutePath);
      } else if (entry.isFile()) {
        files.push(normalizeArchivePath(relative(rootDir, absolutePath)));
      }
    }
  }

  files.sort();
  return files;
}

function loadSourceManifest(rootDir: string): { path: string; manifest: PluginManifest } {
  const manifestPath = join(rootDir, 'plugin.json');
  if (!existsSync(manifestPath)) {
    throw new Error(`plugin.json not found in ${rootDir}`);
  }

  const manifest = readJsonFile<PluginManifest>(manifestPath);
  const validation = validatePluginManifest(manifest);
  if (!validation.valid) {
    throw new Error(validation.errors.join('\n'));
  }

  return {
    path: manifestPath,
    manifest,
  };
}

function validateBuildInputs(rootDir: string, manifest: PluginManifest) {
  if (manifest.runtime.language !== 'javascript') {
    throw new Error('The .ywp packager currently supports JavaScript plugins only.');
  }

  const entrySourcePath = resolve(rootDir, manifest.runtime.entrypoint);
  if (!existsSync(entrySourcePath) || !statSync(entrySourcePath).isFile()) {
    throw new Error(`Plugin entrypoint not found: ${manifest.runtime.entrypoint}`);
  }

  const i18nDirectory = manifest.i18n?.directory || 'locales';
  if (manifest.i18n) {
    for (const locale of manifest.i18n.supportedLocales || []) {
      const localePath = resolve(rootDir, i18nDirectory, `${locale}.json`);
      if (!existsSync(localePath) || !statSync(localePath).isFile()) {
        throw new Error(
          `Missing locale file for ${locale}: ${normalizeArchivePath(relative(rootDir, localePath))}`,
        );
      }
    }

    const defaultLocale = manifest.i18n.defaultLocale;
    if (defaultLocale) {
      const defaultLocalePath = resolve(rootDir, i18nDirectory, `${defaultLocale}.json`);
      if (!existsSync(defaultLocalePath) || !statSync(defaultLocalePath).isFile()) {
        throw new Error(
          `Missing default locale file for ${defaultLocale}: ${normalizeArchivePath(relative(rootDir, defaultLocalePath))}`,
        );
      }
    }
  }
}

function buildRuntimeManifest(sourceManifest: PluginManifest): PluginManifest {
  return {
    ...sourceManifest,
    runtime: {
      ...sourceManifest.runtime,
      entrypoint: PACKAGED_ENTRYPOINT,
    },
  };
}

export function validatePackagedManifest(manifest: PluginManifest) {
  const validation = validatePluginManifest(manifest);
  if (!validation.valid) {
    throw new Error(validation.errors.join('\n'));
  }

  if (manifest.runtime.language !== 'javascript') {
    throw new Error('Packaged .ywp plugins currently support JavaScript runtime only.');
  }

  if (manifest.runtime.entrypoint !== PACKAGED_ENTRYPOINT) {
    throw new Error(`Packaged manifest runtime.entrypoint must be ${PACKAGED_ENTRYPOINT}.`);
  }
}

export async function buildPluginPackage(
  input: BuildPluginPackageInput = {},
): Promise<BuildPluginPackageResult> {
  const bunExecutable = getBunExecutable();
  const rootDir = resolve(input.cwd || process.cwd());
  const { path: sourceManifestPath, manifest: sourceManifest } = loadSourceManifest(rootDir);
  validateBuildInputs(rootDir, sourceManifest);

  const sourceEntrypoint = resolve(rootDir, sourceManifest.runtime.entrypoint);
  const distDir = join(rootDir, 'dist');
  const distEntrypoint = join(distDir, 'plugin.cjs');

  rmSync(distDir, { recursive: true, force: true });
  mkdirSync(dirname(distEntrypoint), { recursive: true });

  const result = spawnSync(
    bunExecutable,
    [
      'build',
      `--outfile=${distEntrypoint}`,
      '--target=node',
      '--format=cjs',
      '--sourcemap=none',
      sourceEntrypoint,
    ],
    {
      cwd: rootDir,
      encoding: 'utf8',
    },
  );

  if (result.status !== 0) {
    const output = [result.stdout, result.stderr]
      .filter((value) => typeof value === 'string' && value.trim().length > 0)
      .join('\n')
      .trim();
    throw new Error(output || 'Build failed');
  }

  if (!existsSync(distEntrypoint) || !statSync(distEntrypoint).isFile()) {
    throw new Error(`Bundled plugin output was not written to ${distEntrypoint}`);
  }

  const copiedFiles: string[] = [];
  const i18nDirectory = sourceManifest.i18n?.directory || 'locales';
  copiedFiles.push(...collectFiles(rootDir, i18nDirectory));
  copiedFiles.push(...collectFiles(rootDir, 'assets'));
  for (const file of ['README.md', 'CHANGELOG.md']) {
    const absolute = join(rootDir, file);
    if (existsSync(absolute) && statSync(absolute).isFile()) {
      copiedFiles.push(file);
    }
  }

  const runtimeManifest = buildRuntimeManifest(sourceManifest);
  validatePackagedManifest(runtimeManifest);

  return {
    rootDir,
    sourceManifestPath,
    sourceManifest,
    runtimeManifest,
    distEntrypoint,
    copiedFiles,
  };
}

function buildPackageInfo(): PackagedPluginBuildInfo {
  return {
    packageFormat: PACKAGE_FORMAT,
    packageFormatVersion: PACKAGE_FORMAT_VERSION,
    packagedAt: new Date().toISOString(),
    builder: {
      tool: 'youwee-sdk',
      version: SDK_VERSION,
    },
    bundle: {
      entrypoint: PACKAGED_ENTRYPOINT,
      bundled: true,
      includesDependencies: true,
      moduleFormat: 'cjs',
    },
  };
}

function toBytes(value: string | Uint8Array): Uint8Array {
  return typeof value === 'string' ? new TextEncoder().encode(value) : value;
}

function readEntryBytes(rootDir: string, relativePath: string): Uint8Array {
  return new Uint8Array(readFileSync(join(rootDir, relativePath)));
}

function buildChecksums(entries: ZipEntry[]): PackagedPluginChecksums {
  const files: Record<string, string> = {};
  for (const entry of entries) {
    files[entry.path] = sha256(entry.bytes);
  }
  return {
    algorithm: 'sha256',
    files,
  };
}

function makeDosDateTime(date: Date) {
  const year = Math.max(1980, date.getFullYear());
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const hours = date.getHours();
  const minutes = date.getMinutes();
  const seconds = Math.floor(date.getSeconds() / 2);

  return {
    time: (hours << 11) | (minutes << 5) | seconds,
    date: ((year - 1980) << 9) | (month << 5) | day,
  };
}

function createCrc32Table() {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i += 1) {
    let c = i;
    for (let j = 0; j < 8; j += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[i] = c >>> 0;
  }
  return table;
}

const CRC32_TABLE = createCrc32Table();

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function concatArrays(chunks: Uint8Array[]): Uint8Array {
  const size = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const result = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }
  return result;
}

function writeU16(view: DataView, offset: number, value: number) {
  view.setUint16(offset, value, true);
}

function writeU32(view: DataView, offset: number, value: number) {
  view.setUint32(offset, value >>> 0, true);
}

function createStoredZip(entries: ZipEntry[]): Uint8Array {
  const localParts: Uint8Array[] = [];
  const centralParts: Uint8Array[] = [];
  let offset = 0;
  const now = makeDosDateTime(new Date());

  for (const entry of entries) {
    const nameBytes = toBytes(normalizeArchivePath(entry.path));
    const dataBytes = entry.bytes;
    const checksum = crc32(dataBytes);

    const localHeader = new Uint8Array(30 + nameBytes.length);
    const localView = new DataView(localHeader.buffer);
    writeU32(localView, 0, 0x04034b50);
    writeU16(localView, 4, 20);
    writeU16(localView, 6, 0);
    writeU16(localView, 8, 0);
    writeU16(localView, 10, now.time);
    writeU16(localView, 12, now.date);
    writeU32(localView, 14, checksum);
    writeU32(localView, 18, dataBytes.length);
    writeU32(localView, 22, dataBytes.length);
    writeU16(localView, 26, nameBytes.length);
    writeU16(localView, 28, 0);
    localHeader.set(nameBytes, 30);

    const centralHeader = new Uint8Array(46 + nameBytes.length);
    const centralView = new DataView(centralHeader.buffer);
    writeU32(centralView, 0, 0x02014b50);
    writeU16(centralView, 4, 20);
    writeU16(centralView, 6, 20);
    writeU16(centralView, 8, 0);
    writeU16(centralView, 10, 0);
    writeU16(centralView, 12, now.time);
    writeU16(centralView, 14, now.date);
    writeU32(centralView, 16, checksum);
    writeU32(centralView, 20, dataBytes.length);
    writeU32(centralView, 24, dataBytes.length);
    writeU16(centralView, 28, nameBytes.length);
    writeU16(centralView, 30, 0);
    writeU16(centralView, 32, 0);
    writeU16(centralView, 34, 0);
    writeU16(centralView, 36, 0);
    writeU32(centralView, 38, 0);
    writeU32(centralView, 42, offset);
    centralHeader.set(nameBytes, 46);

    localParts.push(localHeader, dataBytes);
    centralParts.push(centralHeader);
    offset += localHeader.length + dataBytes.length;
  }

  const centralDirectory = concatArrays(centralParts);
  const localData = concatArrays(localParts);
  const endRecord = new Uint8Array(22);
  const endView = new DataView(endRecord.buffer);
  writeU32(endView, 0, 0x06054b50);
  writeU16(endView, 4, 0);
  writeU16(endView, 6, 0);
  writeU16(endView, 8, entries.length);
  writeU16(endView, 10, entries.length);
  writeU32(endView, 12, centralDirectory.length);
  writeU32(endView, 16, localData.length);
  writeU16(endView, 20, 0);

  return concatArrays([localData, centralDirectory, endRecord]);
}

export async function packPluginPackage(
  input: PackPluginPackageInput = {},
): Promise<PackPluginPackageResult> {
  const buildResult = await buildPluginPackage({ cwd: input.cwd });
  const packageInfo = buildPackageInfo();

  const entries: ZipEntry[] = [
    {
      path: 'manifest.json',
      bytes: toBytes(`${JSON.stringify(buildResult.runtimeManifest, null, 2)}\n`),
    },
    {
      path: 'build.json',
      bytes: toBytes(`${JSON.stringify(packageInfo, null, 2)}\n`),
    },
    {
      path: PACKAGED_ENTRYPOINT,
      bytes: new Uint8Array(readFileSync(buildResult.distEntrypoint)),
    },
  ];

  for (const file of buildResult.copiedFiles) {
    entries.push({
      path: normalizeArchivePath(file),
      bytes: readEntryBytes(buildResult.rootDir, file),
    });
  }

  const checksums = buildChecksums(entries);
  entries.push({
    path: 'checksums.json',
    bytes: toBytes(`${JSON.stringify(checksums, null, 2)}\n`),
  });

  const packageBytes = createStoredZip(entries);
  const outDir = resolve(input.outDir || join(buildResult.rootDir, 'release'));
  mkdirSync(outDir, { recursive: true });
  const packagePath = join(
    outDir,
    `${buildResult.runtimeManifest.slug}-${buildResult.runtimeManifest.version}.${PACKAGE_FORMAT}`,
  );
  writeFileSync(packagePath, packageBytes);

  return {
    packagePath,
    packageChecksum: sha256(packageBytes),
    manifest: buildResult.runtimeManifest,
    buildInfo: packageInfo,
  };
}

export function readPackagedBuildInfo(path: string): PackagedPluginBuildInfo {
  return readJsonFile<PackagedPluginBuildInfo>(path);
}
