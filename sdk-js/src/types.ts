export type PluginTrigger =
  | 'download.queued'
  | 'download.beforeStart'
  | 'download.completed'
  | 'download.failed'
  | 'processing.beforeStart'
  | 'processing.completed';

export type PluginRuntimeLanguage = 'javascript' | 'python';
export type PluginProvider = 'deno' | 'node' | 'bun' | 'python';

export interface ParsedSemver {
  major: number;
  minor: number;
  patch: number;
}

export type JsonShapeLeaf =
  | 'string'
  | 'number'
  | 'boolean'
  | 'object'
  | 'array'
  | 'null'
  | 'unknown';

export interface JsonShapeArrayDescriptor {
  type: 'array';
  items?: JsonShapeDescriptor;
}

export interface JsonShapeObjectDescriptor {
  type: 'object';
  properties?: Record<string, JsonShapeDescriptor>;
  required?: string[];
}

export type JsonShapeDescriptor =
  | JsonShapeLeaf
  | JsonShapeArrayDescriptor
  | JsonShapeObjectDescriptor;

export interface PluginMeta {
  name: string;
  version: string;
  description?: string;
}

export interface BasePluginPayload {
  jobId: string;
  source?: string | null;
  trigger: string;
  filepath: string;
  filename: string;
  directory: string;
  filesize?: number | null;
  format?: string | null;
  quality?: string | null;
  url: string;
  title?: string | null;
  thumbnail?: string | null;
  historyId?: string | null;
  timeRange?: string | null;
  downloadKind: string;
}

export interface DownloadQueuedPayload extends BasePluginPayload {
  trigger: 'download.queued';
}

export interface DownloadBeforeStartPayload extends BasePluginPayload {
  trigger: 'download.beforeStart';
}

export interface DownloadCompletedPayload extends BasePluginPayload {
  trigger: 'download.completed';
}

export interface DownloadFailedPayload extends BasePluginPayload {
  trigger: 'download.failed';
}

export interface ProcessingBeforeStartPayload extends BasePluginPayload {
  trigger: 'processing.beforeStart';
}

export interface ProcessingCompletedPayload extends BasePluginPayload {
  trigger: 'processing.completed';
}

export type PluginPayload =
  | DownloadQueuedPayload
  | DownloadBeforeStartPayload
  | DownloadCompletedPayload
  | DownloadFailedPayload
  | ProcessingBeforeStartPayload
  | ProcessingCompletedPayload
  | BasePluginPayload;

export interface TriggerPayloadMap {
  'download.queued': DownloadQueuedPayload;
  'download.beforeStart': DownloadBeforeStartPayload;
  'download.completed': DownloadCompletedPayload;
  'download.failed': DownloadFailedPayload;
  'processing.beforeStart': ProcessingBeforeStartPayload;
  'processing.completed': ProcessingCompletedPayload;
}

export interface PluginResult {
  success: boolean;
  message?: string | null;
  artifacts?: unknown;
  metadata?: unknown;
}

export interface CommandResult {
  code: number | null;
  signal: string | null;
  stdout: string;
  stderr: string;
}

export interface ToolRunner {
  available: boolean;
  path: string | null;
  run(
    args?: string[],
    options?: { cwd?: string; env?: Record<string, string> },
  ): Promise<CommandResult>;
}

export interface AIConfigSnapshot {
  enabled: boolean;
  provider: string | null;
  model: string | null;
  timeoutSeconds: number;
  summaryStyle: string;
  summaryLanguage: string;
  whisperEnabled: boolean;
  hasApiKey: boolean;
  hasWhisperApiKey: boolean;
}

export interface AITextOptions {
  prompt: string;
  systemPrompt?: string;
  temperature?: number;
}

export interface AISummarizeOptions {
  text: string;
  instructions?: string;
  title?: string;
  maxSentences?: number;
}

export interface AIExtractJsonOptions {
  prompt: string;
  schemaDescription?: string;
  systemPrompt?: string;
  temperature?: number;
  validate?: (value: unknown) => boolean;
}

export interface AIBridge {
  available(): boolean;
  getConfig(): AIConfigSnapshot;
  generateText(options: string | AITextOptions): Promise<string>;
  summarize(options: string | AISummarizeOptions): Promise<string>;
  extractJson<T = unknown>(options: string | AIExtractJsonOptions): Promise<T>;
}

export interface PluginFileSystemBridge {
  exists(path: string): Promise<boolean>;
  readText(path: string): Promise<string>;
  writeText(path: string, content: string): Promise<void>;
  ensureDir(path: string): Promise<void>;
  tempDir(prefix?: string): Promise<string>;
}

export interface PluginHttpRequestOptions {
  method?: string;
  headers?: Record<string, string>;
  body?: string;
  timeoutMs?: number;
}

export interface PluginHttpResponse<TBody = string> {
  ok: boolean;
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: TBody;
}

export interface PluginHttpBridge {
  request(url: string, options?: PluginHttpRequestOptions): Promise<PluginHttpResponse<string>>;
  get(url: string, headers?: Record<string, string>): Promise<PluginHttpResponse<string>>;
  getJson<T = unknown>(
    url: string,
    headers?: Record<string, string>,
  ): Promise<PluginHttpResponse<T>>;
  postJson<T = unknown>(
    url: string,
    body: unknown,
    headers?: Record<string, string>,
  ): Promise<PluginHttpResponse<T>>;
}

export interface CompatibilityCheckResult {
  compatible: boolean;
  currentVersion: string | null;
  requiredRange: string;
  reason?: string;
}

export interface PluginSdkBridge {
  version: string;
  checkAppVersion(range: string): CompatibilityCheckResult;
  assertAppVersion(range: string): void;
}

export interface YouweeBridge {
  app: {
    version: string | null;
  };
  sdk: PluginSdkBridge;
  plugin: {
    id: string | null;
    slug: string | null;
    name: string | null;
    version: string | null;
  };
  runtime: {
    language: string | null;
    provider: string | null;
    providerSource: string | null;
    timeoutMs: number | null;
  };
  tools: {
    ffmpeg: ToolRunner;
    ytdlp: ToolRunner;
  };
  fs: PluginFileSystemBridge;
  http: PluginHttpBridge;
  ai: AIBridge;
}

export interface PluginLogger {
  debug(message: string, metadata?: unknown): void;
  info(message: string, metadata?: unknown): void;
  warn(message: string, metadata?: unknown): void;
  error(message: string, metadata?: unknown): void;
}

export interface PluginContext<TPayload extends PluginPayload = PluginPayload> {
  payload: TPayload;
  trigger: TPayload['trigger'];
  download: {
    jobId: string;
    kind: string;
    source: string | null;
    historyId: string | null;
    timeRange: string | null;
  };
  file: {
    path: string;
    name: string;
    directory: string;
    size: number | null;
    format: string | null;
    quality: string | null;
  };
  media: {
    url: string;
    title: string | null;
    thumbnail: string | null;
  };
  env: {
    get(name: string): string | undefined;
    require(name: string): string;
    has(name: string): boolean;
  };
  log: PluginLogger;
  youwee: YouweeBridge;
  ok(message: string, metadata?: unknown, artifacts?: unknown): PluginResult;
  fail(message: string, metadata?: unknown, artifacts?: unknown): PluginResult;
}

export type DownloadQueuedContext = PluginContext<DownloadQueuedPayload>;
export type DownloadBeforeStartContext = PluginContext<DownloadBeforeStartPayload>;
export type DownloadCompletedContext = PluginContext<DownloadCompletedPayload>;
export type DownloadFailedContext = PluginContext<DownloadFailedPayload>;
export type ProcessingBeforeStartContext = PluginContext<ProcessingBeforeStartPayload>;
export type ProcessingCompletedContext = PluginContext<ProcessingCompletedPayload>;

export type PluginHookHandler<
  TPayload extends PluginPayload = PluginPayload,
  TResult extends PluginResult | undefined = PluginResult | undefined,
> = (ctx: PluginContext<TPayload>) => Promise<TResult> | TResult;

export type KnownPluginHooks = {
  [K in keyof TriggerPayloadMap]?: PluginHookHandler<TriggerPayloadMap[K]>;
};

export type PluginHooks = KnownPluginHooks &
  Partial<Record<string, PluginHookHandler<PluginPayload>>>;

export interface PluginDefinition<THooks extends PluginHooks = PluginHooks> {
  meta: PluginMeta;
  hooks: THooks;
}

export interface PluginPermissionRequest {
  network?: boolean;
  readPaths?: string[];
  writePaths?: string[];
  env?: string[];
}

export interface PluginRuntimeSpec {
  language: PluginRuntimeLanguage;
  supportedProviders: PluginProvider[];
  preferredProvider?: PluginProvider;
  entrypoint: string;
}

export interface PluginManifest {
  pluginId: string;
  slug: string;
  name: string;
  version: string;
  description?: string;
  author?: string;
  homepage?: string;
  repository?: string;
  license?: string;
  runtime: PluginRuntimeSpec;
  triggers?: string[];
  permissions?: PluginPermissionRequest;
  timeoutSec?: number;
  readme?: string;
  checksum?: string;
  publishedAt?: string;
  compatibility?: {
    appVersion?: string;
    sdkVersion?: string;
  };
}

export interface ManifestValidationResult {
  valid: boolean;
  errors: string[];
}

export interface PluginPackageDefinitionInput {
  name: string;
  version: string;
  description?: string;
  main?: string;
}
