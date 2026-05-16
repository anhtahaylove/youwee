# youwee-sdk

`youwee-sdk` is the JavaScript and TypeScript plugin SDK for Youwee.

This document is intentionally written as technical documentation rather than a lightweight getting-started guide. It is meant to serve as:

- a runtime contract reference for plugin authors
- an implementation reference for AI agents generating plugin code
- a specification for how JavaScript plugins communicate with Youwee

---

## Table of Contents

1. [Scope](#scope)
2. [Architectural Model](#architectural-model)
3. [Package Layout](#package-layout)
4. [Runtime Execution Model](#runtime-execution-model)
5. [Plugin Module Contract](#plugin-module-contract)
6. [SDK Surface](#sdk-surface)
7. [Context Model](#context-model)
8. [Result Contract](#result-contract)
9. [Logging Contract](#logging-contract)
10. [Youwee Capabilities Bridge](#youwee-capabilities-bridge)
11. [AI Bridge](#ai-bridge)
12. [Trigger Catalog](#trigger-catalog)
13. [Local Execution](#local-execution)
14. [Packaging and Distribution](#packaging-and-distribution)
15. [Release and Versioning Policy](#release-and-versioning-policy)
16. [Design Notes](#design-notes)
17. [Troubleshooting](#troubleshooting)
18. [AI Agent Notes](#ai-agent-notes)
19. [Appendix A: Minimal Plugin](#appendix-a-minimal-plugin)
20. [Appendix B: Payload Mapping](#appendix-b-payload-mapping)
21. [Appendix C: Capability Reference](#appendix-c-capability-reference)

---

## Scope

`youwee-sdk` exists to formalize the JavaScript plugin contract for Youwee.

The SDK is responsible for:

- exposing a stable plugin module shape
- converting raw process input into a typed execution context
- providing result helpers and logging helpers
- bridging selected Youwee capabilities into plugin runtime space

The SDK is not responsible for:

- plugin installation
- manifest approval or permission UX
- runtime selection policy
- queue scheduling or download orchestration

Those concerns remain part of the Youwee application runtime.

---

## Architectural Model

Youwee plugins are hook-based modules.

At runtime:

1. Youwee resolves a plugin installation and selects a runtime provider.
2. Youwee launches a shared SDK bootstrap.
3. The bootstrap loads the plugin entry module.
4. The bootstrap reads the JSON payload from `stdin`.
5. The bootstrap constructs `ctx`.
6. The bootstrap resolves the hook for the incoming trigger.
7. The hook executes.
8. The final result is written as JSON to `stdout`.

This design deliberately removes per-plugin runner boilerplate.

A plugin author should only need to maintain:

- the manifest
- the package metadata
- the plugin hook module

They should not need to create a custom execution bootstrap per plugin.

---

## Package Layout

The SDK package is authored in TypeScript and emitted as CommonJS runtime artifacts.

```text
sdk-js/
  package.json
  README.md
  src/
    index.ts
    runtime.ts
    runtime-cli.ts
    ai.ts
    types.ts
  dist/
    index.js
    index.d.ts
    runtime.js
    runtime.d.ts
    runtime-cli.js
    runtime-cli.d.ts
    ai.js
    ai.d.ts
    types.d.ts
```

Semantics:

- `src/`: source of truth
- `dist/`: runtime artifacts consumed by plugin scaffolds and the Youwee application
- `runtime-cli`: shared bootstrap entry used by Youwee and local test commands

---

## Runtime Execution Model

### Shared bootstrap

The SDK provides a shared runtime bootstrap at:

- `youwee-sdk/dist/runtime-cli.js`

This bootstrap is launched by the application for Node and Bun-based JavaScript plugins.

The bootstrap expects:

- `stdin`: a JSON payload emitted by Youwee
- `YOUWEE_PLUGIN_MAIN`: absolute or runtime-resolvable path to the plugin entry module

This means plugin packages do not need a custom `run.js` file.

### Why there is no per-plugin runner

Per-plugin runner files are mechanically identical in almost every plugin:

- import the SDK bootstrap
- import the plugin module
- call the bootstrap
- map thrown errors to process exit

That pattern adds noise without adding plugin-specific value.

For that reason, the recommended architecture is:

- one shared runtime bootstrap in the SDK
- one plugin-specific hook module per plugin

---

## Plugin Module Contract

A JavaScript plugin entry module should export a plugin definition via `definePlugin(...)`.

Example:

```js
const { definePlugin, triggers } = require("youwee-sdk");

module.exports = definePlugin({
  meta: {
    name: "Example plugin",
    version: "0.1.0",
    description: "Example plugin description",
  },
  hooks: {
    [triggers.downloadCompleted]: async (ctx) => {
      return ctx.ok("Completed");
    },
  },
});
```

Contract requirements:

- the module must export an object
- the object must contain `meta`
- the object must contain `hooks`
- hook values must be callable functions

The SDK validates this shape at runtime.

---

## SDK Surface

### Main package

```js
const { definePlugin, defineHooks, triggers, TRIGGERS } = require("youwee-sdk");
```

Exports:

- `definePlugin(config)`
- `defineHooks(hooks)`
- `triggers`
- `TRIGGERS`
- `SDK_VERSION`
- `parseSemver(version)`
- `compareSemver(a, b)`
- `satisfiesVersionRange(version, range)`
- `checkAppVersionCompatibility(currentVersion, range)`
- `assertCompatibleAppVersion(currentVersion, range)`
- `createJsonShapeValidator(shape)`
- `matchesJsonShape(value, shape)`
- `slugifyPluginName(input)`
- `getAllowedProviders(language)`
- `getManifestValidationErrors(manifest)`
- `validatePluginManifest(manifest)`
- `createPluginPackageDefinition(input)`
- `createPluginPackageJson(input)`

### Runtime package

```js
const { runPluginModule, createContext, createLogger, spawnCommand } = require("youwee-sdk/runtime");
```

Exports:

- `runPluginModule(pluginModule)`
- `createContext(payload)`
- `createLogger()`
- `spawnCommand(command, args, options?)`

### AI package

```js
const { createAIBridge, readAIConfigFromEnv } = require("youwee-sdk/ai");
```

Exports:

- `createAIBridge(logger?)`
- `readAIConfigFromEnv()`

In typical plugin authoring flows, only the main package is required directly.

### Authoring helpers

The main package also exposes manifest and package authoring helpers. These are intended for:

- plugin template generators
- local scaffolding tools
- AI agents generating or refactoring plugin packages

Example:

```js
const {
  createPluginPackageJson,
  slugifyPluginName,
  validatePluginManifest,
} = require("youwee-sdk");
```

---

## Context Model

Each hook receives a `ctx` object constructed by the SDK.

### `ctx.trigger`

The resolved trigger string for the current execution.

Example:

```js
ctx.trigger === "download.completed";
```

### `ctx.payload`

The raw payload object after JSON parsing.

This exists for completeness, but plugin authors should prefer the normalized sub-objects below.

### `ctx.download`

Normalized download/job metadata:

- `jobId`
- `kind`
- `source`
- `historyId`
- `timeRange`

### `ctx.file`

Normalized file metadata:

- `path`
- `name`
- `directory`
- `size`
- `format`
- `quality`

### `ctx.media`

Normalized media metadata:

- `url`
- `title`
- `thumbnail`

### `ctx.env`

Environment variable helpers:

- `ctx.env.get(name)`
- `ctx.env.require(name)`
- `ctx.env.has(name)`

Recommended use:

```js
const token = ctx.env.require("GOOGLE_DRIVE_ACCESS_TOKEN");
```

### `ctx.log`

Structured plugin logger:

- `debug(message, metadata?)`
- `info(message, metadata?)`
- `warn(message, metadata?)`
- `error(message, metadata?)`

### `ctx.youwee`

Bridge to application-managed capabilities:

- `ctx.youwee.plugin`
- `ctx.youwee.runtime`
- `ctx.youwee.tools`
- `ctx.youwee.ai`

### `ctx.ok(...)` and `ctx.fail(...)`

Result helpers:

```js
return ctx.ok("Uploaded successfully");
return ctx.fail("Missing token");
```

---

## Result Contract

The final result written to `stdout` must be JSON-serializable and should follow this shape:

```json
{
  "success": true,
  "message": "Human readable summary",
  "artifacts": null,
  "metadata": {}
}
```

Field semantics:

- `success`: required boolean
- `message`: short human-readable status summary
- `artifacts`: optional structured outputs intended for downstream use
- `metadata`: optional diagnostic or contextual payload

Examples:

```js
return ctx.ok("Uploaded to Google Drive", {
  driveFileId: "abc123",
});
```

```js
return ctx.fail("Upload failed", {
  reason: "HTTP 401",
});
```

If a hook returns `undefined`, the SDK emits a success fallback result. Explicit returns are strongly preferred.

---

## Logging Contract

The SDK separates logging from final structured results.

- runtime logs go to `stderr`
- final execution result goes to `stdout`

This separation is critical because Youwee parses the structured result channel.

Correct:

```js
ctx.log.info("Starting request", { filename: ctx.file.name });
return ctx.ok("Finished");
```

Incorrect:

```js
console.log("Starting request");
console.log(JSON.stringify({ debug: true }));
```

Plugin authors should treat `stdout` as reserved for the final result.

---

## Youwee Capabilities Bridge

### `ctx.youwee.plugin`

Execution-time plugin identity:

- `id`
- `slug`
- `name`
- `version`

### `ctx.youwee.runtime`

Resolved runtime information:

- `language`
- `provider`
- `providerSource`
- `timeoutMs`

### `ctx.youwee.app`

Application runtime metadata:

- `version`

### `ctx.youwee.sdk`

SDK runtime metadata and compatibility helpers:

- `version`
- `checkAppVersion(range)`
- `assertAppVersion(range)`

Example:

```js
const compatibility = ctx.youwee.sdk.checkAppVersion(">=0.13.0 <0.14.0");

if (!compatibility.compatible) {
  return ctx.fail("Unsupported Youwee version", compatibility);
}
```

Or fail immediately:

```js
ctx.youwee.sdk.assertAppVersion(">=0.13.0 <0.14.0");
```

### `ctx.youwee.tools`

Runtime-managed tool bridge currently includes:

- `ffmpeg`
- `ytdlp`

Each tool exposes:

- `available`
- `path`
- `run(args, options?)`

Example:

```js
if (!ctx.youwee.tools.ffmpeg.available) {
  return ctx.fail("FFmpeg is not available");
}

const result = await ctx.youwee.tools.ffmpeg.run([
  "-i",
  ctx.file.path,
  "-f",
  "null",
  "-",
]);
```

This allows plugins to use the application-managed tool resolution logic instead of assuming a global system path.

### `ctx.youwee.fs`

Filesystem helpers exposed by the SDK:

- `exists(path)`
- `readText(path)`
- `writeText(path, content)`
- `ensureDir(path)`
- `tempDir(prefix?)`

These helpers are convenience APIs for common plugin-side filesystem work. They do not replace the app's permission model. Plugin authors are still expected to declare and receive the required path permissions in the plugin manifest.

### `ctx.youwee.http`

HTTP helpers exposed by the SDK:

- `request(url, options?)`
- `get(url, headers?)`
- `getJson(url, headers?)`
- `postJson(url, body, headers?)`

Example:

```js
const response = await ctx.youwee.http.getJson("https://example.com/api");

if (!response.ok) {
  return ctx.fail("HTTP request failed", {
    status: response.status,
    body: response.body,
  });
}
```

The bridge returns normalized response objects with:

- `ok`
- `status`
- `statusText`
- `headers`
- `body`

---

## AI Bridge

The SDK exposes application AI configuration through:

- `ctx.youwee.ai.available()`
- `ctx.youwee.ai.getConfig()`
- `ctx.youwee.ai.generateText(options)`

### Availability

```js
if (!ctx.youwee.ai.available()) {
  return ctx.fail("AI is not enabled in Youwee");
}
```

### Configuration snapshot

`getConfig()` returns a non-secret capability snapshot such as:

```js
{
  enabled: true,
  provider: "openai",
  model: "gpt-4.1-mini",
  timeoutSeconds: 120,
  summaryStyle: "concise",
  summaryLanguage: "auto",
  whisperEnabled: false,
  hasApiKey: true,
  hasWhisperApiKey: false
}
```

### Text generation

```js
const summary = await ctx.youwee.ai.generateText({
  prompt: `Summarize the media file ${ctx.file.name}`,
  systemPrompt: "Return a short Vietnamese summary.",
  temperature: 0.2,
});
```

The bridge uses the provider configured by the Youwee user, not a provider chosen by the plugin.

### Higher-level AI helpers

The AI bridge also exposes:

- `summarize(options)`
- `extractJson(options)`

Example:

```js
const summary = await ctx.youwee.ai.summarize({
  text: "Long input text",
  title: "Optional title",
  maxSentences: 3,
});
```

```js
const data = await ctx.youwee.ai.extractJson({
  prompt: "Convert this content into a JSON object",
  schemaDescription: '{ "title": "string", "score": "number" }',
});
```

`extractJson(...)` attempts to normalize model output into valid JSON and can recover from common responses such as fenced code blocks.
It also supports an optional `validate(value)` function to reject structurally invalid results after parsing.

---

## Compatibility Utilities

The main package exposes lightweight semver-based compatibility helpers:

- `parseSemver(version)`
- `compareSemver(a, b)`
- `satisfiesVersionRange(version, range)`
- `checkAppVersionCompatibility(currentVersion, range)`
- `assertCompatibleAppVersion(currentVersion, range)`

Supported range syntax is intentionally simple and explicit. Examples:

- `>=0.13.0`
- `>=0.13.0 <0.14.0`
- `=0.13.3`
- `0.13.3`

This is meant for plugin-side runtime guards, not as a full npm-style semver engine.

---

## JSON Shape Validation

The SDK also exposes lightweight JSON shape validation helpers for use with `extractJson(...)`.

Exports:

- `createJsonShapeValidator(shape)`
- `matchesJsonShape(value, shape)`

Example:

```js
const validator = createJsonShapeValidator({
  type: "object",
  required: ["title", "score"],
  properties: {
    title: "string",
    score: "number",
    tags: { type: "array", items: "string" },
  },
});

const data = await ctx.youwee.ai.extractJson({
  prompt: "Return a JSON object with title, score, and tags",
  validate: validator,
});
```

Supported descriptors:

- `"string"`
- `"number"`
- `"boolean"`
- `"object"`
- `"array"`
- `"null"`
- `"unknown"`
- `{ type: "array", items?: descriptor }`
- `{ type: "object", properties?: { ... }, required?: [...] }`

---

## Trigger Catalog

The SDK exposes a forward-looking trigger catalog:

- `triggers.downloadQueued`
- `triggers.downloadBeforeStart`
- `triggers.downloadCompleted`
- `triggers.downloadFailed`
- `triggers.processingBeforeStart`
- `triggers.processingCompleted`

Important:

- the SDK may expose more trigger names than the application dispatches today
- plugin code may be structured against stable trigger names before every trigger is live in the app

Authors should still confirm that the installed application version dispatches the trigger they depend on.

---

## Local Execution

Recommended Node test:

```bash
cat examples/payload.download.completed.json | NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js node vendor/youwee-sdk/dist/runtime-cli.js
```

Recommended Bun test:

```bash
cat examples/payload.download.completed.json | NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js bun vendor/youwee-sdk/dist/runtime-cli.js
```

If the plugin requires environment variables:

```bash
GOOGLE_DRIVE_ACCESS_TOKEN=xxx \
GOOGLE_DRIVE_FOLDER_ID=yyy \
cat examples/payload.download.completed.json | NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js node vendor/youwee-sdk/dist/runtime-cli.js
```

Validation checklist:

1. `stdout` contains exactly one final JSON result.
2. `stderr` contains the expected runtime logs.
3. required environment variables fail clearly when missing.
4. tool bridge availability is checked before tool execution.

---

## Packaging and Distribution

A shareable plugin package should typically include:

- `plugin.json`
- `package.json`
- `README.md`
- `src/`
- `vendor/youwee-sdk/`
- any plugin-specific assets or example payloads

Distribution model:

1. keep `plugin.json` at the package root
2. ensure manifest metadata is correct
3. ensure runtime compatibility is declared correctly
4. optionally declare `compatibility.appVersion` and `compatibility.sdkVersion`
5. package the plugin root as a folder or ZIP
6. import into Youwee from folder, ZIP, or URL

Example:

```json
{
  "compatibility": {
    "appVersion": ">=0.13.0 <0.14.0",
    "sdkVersion": ">=0.1.0 <0.2.0"
  }
}
```

`appVersion` and `sdkVersion` are intended to fail fast when a shared plugin is installed or executed against an incompatible environment.

## Release and Versioning Policy

`youwee-sdk` is versioned as a package, but it is currently maintained inside the main Youwee repository.

The operational release policy lives in:

- `sdk-js/CHANGELOG.md`
- `sdk-js/RELEASING.md`

High-level rules:

- use semantic versioning
- keep TypeScript source in `sdk-js/src/`
- treat `sdk-js/dist/` as build output, not hand-edited source
- keep plugin scaffold vendoring and backend compatibility enforcement aligned with the current SDK version

---

## Design Notes

### Why the SDK remains inside the main repository

At the current stage, keeping the SDK inside the Youwee repository is the least risky option because:

- the plugin contract is still evolving
- runtime provider behavior is still evolving
- scaffolding and runtime bridge logic must stay synchronized

The package is still structured as an actual package so it can be published or moved to a separate repository later without changing plugin authoring semantics.

### Why the SDK is authored in TypeScript

The SDK is authored in TypeScript because:

- the plugin contract benefits from strong type documentation
- generated declarations help editor tooling and AI agent code generation
- it supports eventual package publication more cleanly

Runtime output remains CommonJS JavaScript for compatibility with supported providers.

### Why per-plugin runners are removed

Per-plugin runner files are repetitive and do not encode domain-specific logic.

Eliminating them:

- reduces scaffold noise
- reduces plugin author confusion
- centralizes bootstrap behavior in one audited location

---

## Troubleshooting

### The plugin does not execute

Check:

- the plugin is enabled
- the trigger is actually dispatched by the app version in use
- the selected provider is listed in `supportedProviders`
- the runtime provider exists on the machine

### The plugin times out

Typical causes:

- a network request never completes
- a child process never exits
- asynchronous work keeps the event loop alive unintentionally

Recommended debugging steps:

- add `ctx.log.info(...)` around long-running operations
- verify external tool invocations terminate
- confirm final control flow returns or throws

### The plugin cannot use AI

Check:

- the user enabled AI in Youwee
- a valid provider is configured
- the corresponding API key is available
- `ctx.youwee.ai.available()` returns `true`

### FFmpeg or yt-dlp is unavailable

Check:

```js
ctx.youwee.tools.ffmpeg.available;
ctx.youwee.tools.ytdlp.available;
```

If unavailable, fail explicitly instead of assuming the tool exists.

---

## AI Agent Notes

If you are an AI agent generating or editing a plugin based on this SDK, follow these rules:

1. Place business logic in the plugin entry module, not in the runtime bootstrap.
2. Use `definePlugin(...)` for the exported module shape.
3. Use `triggers.*` instead of hard-coded trigger strings when possible.
4. Use `ctx.env.require(...)` for required secrets.
5. Use `ctx.ok(...)` or `ctx.fail(...)` for final results.
6. Use `ctx.log.info(...)` for major execution milestones.
7. Do not write arbitrary text to `stdout`.
8. Do not assume AI, FFmpeg, yt-dlp, or any specific runtime is always available.
9. Treat `ctx.youwee.runtime.provider` as runtime metadata, not as a guarantee of feature parity.

Recommended internal instruction for AI plugin generation:

> Implement hook logic in `src/plugin.js` for the required trigger. Use `ctx.file`, `ctx.media`, `ctx.env`, `ctx.youwee.tools`, and `ctx.youwee.ai` as needed. Do not create a custom runner.

---

## Appendix A: Minimal Plugin

```js
const { definePlugin, triggers } = require("youwee-sdk");

module.exports = definePlugin({
  meta: {
    name: "Minimal plugin",
    version: "0.1.0",
    description: "Minimal reference plugin",
  },
  hooks: {
    [triggers.downloadCompleted]: async (ctx) => {
      ctx.log.info("Hook entered", {
        filename: ctx.file.name,
      });

      return ctx.ok("Completed", {
        filename: ctx.file.name,
      });
    },
  },
});
```

---

## Appendix B: Payload Mapping

Representative payload from Youwee:

```json
{
  "jobId": "sample-job",
  "source": "youtube",
  "trigger": "download.completed",
  "filepath": "/tmp/sample.mp4",
  "filename": "sample.mp4",
  "directory": "/tmp",
  "filesize": 12345678,
  "format": "mp4",
  "quality": "1080p",
  "url": "https://example.com/video",
  "title": "Sample video",
  "thumbnail": "https://example.com/thumb.jpg",
  "historyId": "sample-history-id",
  "timeRange": null,
  "downloadKind": "download"
}
```

Normalized mapping:

- `payload.jobId` -> `ctx.download.jobId`
- `payload.filepath` -> `ctx.file.path`
- `payload.filename` -> `ctx.file.name`
- `payload.url` -> `ctx.media.url`
- `payload.historyId` -> `ctx.download.historyId`

The SDK exposes the raw payload as `ctx.payload`, but plugins should usually prefer the normalized objects.

---

## Appendix C: Capability Reference

### `ctx.youwee.plugin`

- `id`
- `slug`
- `name`
- `version`

### `ctx.youwee.runtime`

- `language`
- `provider`
- `providerSource`
- `timeoutMs`

### `ctx.youwee.tools.ffmpeg`

- `available`
- `path`
- `run(args, options?)`

### `ctx.youwee.tools.ytdlp`

- `available`
- `path`
- `run(args, options?)`

### `ctx.youwee.ai`

- `available()`
- `getConfig()`
- `generateText({ prompt, systemPrompt?, temperature? })`

---

## Status

The SDK is intentionally designed as a real package with a shared bootstrap and a TypeScript source model, even while it is still housed inside the Youwee repository.

That design keeps the plugin authoring experience stable while allowing the app runtime and the package to evolve together.
