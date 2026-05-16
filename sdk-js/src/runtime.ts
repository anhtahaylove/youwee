import { spawn } from 'node:child_process';
import { access, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createAIBridge } from './ai';
import {
  assertCompatibleAppVersion,
  checkAppVersionCompatibility,
  SDK_VERSION,
} from './compatibility';
import type {
  CommandResult,
  CompatibilityCheckResult,
  PluginContext,
  PluginDefinition,
  PluginFileSystemBridge,
  PluginHttpBridge,
  PluginHttpRequestOptions,
  PluginHttpResponse,
  PluginLogger,
  PluginPayload,
  PluginResult,
  ToolRunner,
  YouweeBridge,
} from './types';

function writeStderr(level: string, message: string, metadata?: unknown): void {
  const suffix = metadata ? ` ${JSON.stringify(metadata)}` : '';
  process.stderr.write(`[${level}] ${message}${suffix}\n`);
}

export function createLogger(): PluginLogger {
  return {
    debug(message, metadata) {
      writeStderr('debug', message, metadata);
    },
    info(message, metadata) {
      writeStderr('info', message, metadata);
    },
    warn(message, metadata) {
      writeStderr('warn', message, metadata);
    },
    error(message, metadata) {
      writeStderr('error', message, metadata);
    },
  };
}

function parseNumber(value: string | undefined): number | null {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function resolveAvailableTool(pathEnvName: string): Pick<ToolRunner, 'available' | 'path'> {
  const path = process.env[pathEnvName] || null;
  return {
    available: Boolean(path),
    path,
  };
}

function createCommandRunner(toolName: string, pathEnvName: string): ToolRunner {
  const tool = resolveAvailableTool(pathEnvName);

  return {
    ...tool,
    async run(args = [], options = {}) {
      if (!tool.path) {
        throw new Error(`${toolName} is not available in this Youwee runtime.`);
      }

      return await spawnCommand(tool.path, args, options);
    },
  };
}

export function spawnCommand(
  command: string,
  args: string[],
  options: { cwd?: string; env?: Record<string, string> } = {},
): Promise<CommandResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd || process.cwd(),
      env: {
        ...process.env,
        ...(options.env || {}),
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];

    child.stdout.on('data', (chunk: Buffer | string) => {
      stdoutChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });

    child.stderr.on('data', (chunk: Buffer | string) => {
      stderrChunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });

    child.on('error', (error) => {
      reject(error);
    });

    child.on('close', (code, signal) => {
      resolve({
        code: typeof code === 'number' ? code : null,
        signal,
        stdout: Buffer.concat(stdoutChunks).toString('utf8'),
        stderr: Buffer.concat(stderrChunks).toString('utf8'),
      });
    });
  });
}

function createFileSystemBridge(): PluginFileSystemBridge {
  return {
    async exists(path) {
      try {
        await access(path);
        return true;
      } catch {
        return false;
      }
    },
    async readText(path) {
      return await readFile(path, 'utf8');
    },
    async writeText(path, content) {
      await writeFile(path, content, 'utf8');
    },
    async ensureDir(path) {
      await mkdir(path, { recursive: true });
    },
    async tempDir(prefix = 'youwee-plugin-') {
      return await mkdtemp(join(tmpdir(), prefix));
    },
  };
}

function createHttpBridge(): PluginHttpBridge {
  return {
    async request(url, options = {}) {
      return await requestText(url, options);
    },
    async get(url, headers) {
      return await requestText(url, {
        method: 'GET',
        headers,
      });
    },
    async getJson(url, headers) {
      return await requestJson(url, {
        method: 'GET',
        headers,
      });
    },
    async postJson(url, body, headers) {
      return await requestJson(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(headers || {}),
        },
        body: JSON.stringify(body),
      });
    },
  };
}

function createSdkBridge(currentAppVersion: string | null): {
  version: string;
  checkAppVersion(range: string): CompatibilityCheckResult;
  assertAppVersion(range: string): void;
} {
  return {
    version: SDK_VERSION,
    checkAppVersion(range) {
      return checkAppVersionCompatibility(currentAppVersion, range);
    },
    assertAppVersion(range) {
      assertCompatibleAppVersion(currentAppVersion, range);
    },
  };
}

async function requestText(
  url: string,
  options: PluginHttpRequestOptions = {},
): Promise<PluginHttpResponse<string>> {
  const response = await fetchWithTimeout(url, options);
  const body = await response.text();
  return {
    ok: response.ok,
    status: response.status,
    statusText: response.statusText,
    headers: normalizeHeaders(response.headers),
    body,
  };
}

async function requestJson<T>(
  url: string,
  options: PluginHttpRequestOptions = {},
): Promise<PluginHttpResponse<T>> {
  const response = await fetchWithTimeout(url, options);
  const text = await response.text();
  let body: T;

  try {
    body = JSON.parse(text) as T;
  } catch {
    throw new Error(`HTTP response from ${url} was not valid JSON.`);
  }

  return {
    ok: response.ok,
    status: response.status,
    statusText: response.statusText,
    headers: normalizeHeaders(response.headers),
    body,
  };
}

async function fetchWithTimeout(url: string, options: PluginHttpRequestOptions): Promise<Response> {
  const controller = new AbortController();
  const timeoutMs = Math.max(1, options.timeoutMs ?? 30000);
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    return await fetch(url, {
      method: options.method,
      headers: options.headers,
      body: options.body,
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timer);
  }
}

function normalizeHeaders(headers: Headers): Record<string, string> {
  const output: Record<string, string> = {};
  headers.forEach((value, key) => {
    output[key] = value;
  });
  return output;
}

function createYouweeBridge(logger: PluginLogger): YouweeBridge {
  const appVersion = process.env.YOUWEE_APP_VERSION || null;
  return {
    app: {
      version: appVersion,
    },
    sdk: createSdkBridge(appVersion),
    plugin: {
      id: process.env.YOUWEE_PLUGIN_ID || null,
      slug: process.env.YOUWEE_PLUGIN_SLUG || null,
      name: process.env.YOUWEE_PLUGIN_NAME || null,
      version: process.env.YOUWEE_PLUGIN_VERSION || null,
    },
    runtime: {
      language: process.env.YOUWEE_PLUGIN_LANGUAGE || null,
      provider: process.env.YOUWEE_PLUGIN_PROVIDER || null,
      providerSource: process.env.YOUWEE_PLUGIN_PROVIDER_SOURCE || null,
      timeoutMs: parseNumber(process.env.YOUWEE_PLUGIN_TIMEOUT_MS),
    },
    tools: {
      ffmpeg: createCommandRunner('FFmpeg', 'YOUWEE_FFMPEG_PATH'),
      ytdlp: createCommandRunner('yt-dlp', 'YOUWEE_YTDLP_PATH'),
    },
    fs: createFileSystemBridge(),
    http: createHttpBridge(),
    ai: createAIBridge(logger),
  };
}

export function createContext(payload: PluginPayload): PluginContext {
  const logger = createLogger();

  return {
    payload,
    trigger: payload.trigger,
    download: {
      jobId: payload.jobId,
      kind: payload.downloadKind,
      source: payload.source ?? null,
      historyId: payload.historyId ?? null,
      timeRange: payload.timeRange ?? null,
    },
    file: {
      path: payload.filepath,
      name: payload.filename,
      directory: payload.directory,
      size: payload.filesize ?? null,
      format: payload.format ?? null,
      quality: payload.quality ?? null,
    },
    media: {
      url: payload.url,
      title: payload.title ?? null,
      thumbnail: payload.thumbnail ?? null,
    },
    env: {
      get(name) {
        return process.env[name];
      },
      require(name) {
        const value = process.env[name];
        if (!value) {
          throw new Error(`Missing required environment variable: ${name}`);
        }
        return value;
      },
      has(name) {
        return Boolean(process.env[name]);
      },
    },
    log: logger,
    youwee: createYouweeBridge(logger),
    ok(message, metadata = null, artifacts = null): PluginResult {
      return {
        success: true,
        message,
        metadata,
        artifacts,
      };
    },
    fail(message, metadata = null, artifacts = null): PluginResult {
      return {
        success: false,
        message,
        metadata,
        artifacts,
      };
    },
  };
}

async function readInput(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString('utf8');
}

function normalizePluginModule(
  pluginModule: PluginDefinition | { default?: PluginDefinition },
): PluginDefinition {
  return pluginModule && 'default' in pluginModule && pluginModule.default
    ? pluginModule.default
    : (pluginModule as PluginDefinition);
}

function validatePlugin(plugin: PluginDefinition): void {
  if (!plugin || typeof plugin !== 'object') {
    throw new Error('Plugin module must export an object from definePlugin(...)');
  }

  if (!plugin.hooks || typeof plugin.hooks !== 'object') {
    throw new Error('Plugin module is missing hooks.');
  }
}

export async function runPluginModule(
  pluginModule: PluginDefinition | { default?: PluginDefinition },
): Promise<void> {
  const plugin = normalizePluginModule(pluginModule);
  validatePlugin(plugin);

  const input = await readInput();
  const payload = JSON.parse(input) as PluginPayload;
  const hook = plugin.hooks?.[payload.trigger];

  if (typeof hook !== 'function') {
    throw new Error(`No hook registered for trigger: ${payload.trigger}`);
  }

  const ctx = createContext(payload);
  const result = await hook(ctx);
  process.stdout.write(
    `${JSON.stringify(result ?? ctx.ok('Plugin completed without explicit result.'))}\n`,
  );
}

export { writeStderr };
