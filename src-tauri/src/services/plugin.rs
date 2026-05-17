use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsStr;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use uuid::Uuid;
use zip::ZipArchive;

use crate::database::add_log_internal;
use crate::types::{
    PluginChainMutation, PluginChainState, PluginCompatibilitySpec, PluginExecutionOutputEvent,
    PluginExecutionResult, PluginExecutionStatusEvent, PluginInstallation, PluginManifest,
    PluginPackageInspection, PluginPackageSource, PluginPackageSourceKind,
    PluginPermissionApproval, PluginPermissionRequest, PluginProvider, PluginRuntimeLanguage,
    PluginRuntimeSpec, PluginSummary, PluginTriggerWorkflow, PluginWorkflowFailurePolicy,
    PluginWorkflowRun, PluginWorkflowRunStatus, PluginWorkflowStepConfig,
    PluginWorkflowStepSnapshot, PostDownloadPluginPayload, RuntimeProviderStatus,
};
use crate::utils::CommandExt;

const PLUGINS_DIR_NAME: &str = "plugins";
const REGISTRY_FILE_NAME: &str = "registry.json";
const SDK_JS_PACKAGE_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/package.json"));
const SDK_JS_INDEX: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/index.js"));
const SDK_JS_RUNTIME: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/runtime.js"));
const SDK_JS_RUNTIME_CLI: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/runtime-cli.js"));
const SDK_JS_AI: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/ai.js"));
const SDK_JS_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/index.d.ts"));
const SDK_JS_RUNTIME_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/runtime.d.ts"));
const SDK_JS_RUNTIME_CLI_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/runtime-cli.d.ts"));
const SDK_JS_AI_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/ai.d.ts"));
const SDK_JS_MANIFEST: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/manifest.js"));
const SDK_JS_MANIFEST_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/manifest.d.ts"));
const SDK_JS_COMPATIBILITY: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/compatibility.js"));
const SDK_JS_COMPATIBILITY_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/compatibility.d.ts"));
const SDK_JS_SCHEMA: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/schema.js"));
const SDK_JS_SCHEMA_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/schema.d.ts"));
const SDK_JS_SHARED_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/types.d.ts"));
const SDK_JS_SHARED_RUNTIME_TYPES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/dist/types.js"));
const SDK_JS_README: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../sdk-js/README.md"));

static PLUGIN_WORKFLOW_QUEUE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissionApprovalInput {
    pub network: bool,
    pub read_paths: bool,
    pub write_paths: bool,
    pub env: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginSourceInput {
    pub kind: PluginPackageSourceKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePluginScaffoldInput {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginEnvValuesInput {
    #[serde(default)]
    pub values: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PluginRegistryEntry {
    enabled: bool,
    trusted: bool,
    #[serde(default)]
    approved_permissions: PluginPermissionApproval,
    #[serde(default)]
    env_values: BTreeMap<String, String>,
    #[serde(default)]
    selected_provider: Option<PluginProvider>,
    source: Option<PluginPackageSource>,
    #[serde(default)]
    last_resolved_provider: Option<PluginProvider>,
    #[serde(default)]
    last_resolved_source: Option<String>,
    #[serde(default)]
    last_execution_status: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PluginTriggerWorkflowRegistry {
    #[serde(default)]
    steps: Vec<PluginWorkflowStepConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PluginRegistry {
    #[serde(default)]
    installations: BTreeMap<String, PluginRegistryEntry>,
    #[serde(default)]
    default_providers: BTreeMap<String, PluginProvider>,
    #[serde(default)]
    trigger_workflows: BTreeMap<String, PluginTriggerWorkflowRegistry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginScriptOutput {
    success: Option<bool>,
    message: Option<String>,
    artifacts: Option<Value>,
    metadata: Option<Value>,
    mutations: Option<PluginChainMutation>,
}

#[derive(Debug, Clone)]
struct PreparedPackage {
    manifest: PluginManifest,
    package_root: PathBuf,
    source: PluginPackageSource,
    warnings: Vec<String>,
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_len).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn combine_plugin_event_details(
    message: Option<&String>,
    stdout: Option<&String>,
    stderr: Option<&String>,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(value) = message {
        if !value.trim().is_empty() {
            lines.push(format!("message: {value}"));
        }
    }
    if let Some(value) = stdout {
        if !value.trim().is_empty() {
            lines.push(format!("stdout: {value}"));
        }
    }
    if let Some(value) = stderr {
        if !value.trim().is_empty() {
            lines.push(format!("stderr: {value}"));
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn shorten_for_event(text: Option<String>) -> Option<String> {
    text.map(|value| truncate_text(&value, 1500))
}

pub fn plugins_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {}", e))?;
    Ok(app_data_dir.join(PLUGINS_DIR_NAME))
}

fn registry_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(plugins_root(app)?.join(REGISTRY_FILE_NAME))
}

fn ensure_plugins_root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = plugins_root(app)?;
    std::fs::create_dir_all(&root)
        .map_err(|e| format!("Failed to create plugins directory {}: {}", root.display(), e))?;
    Ok(root)
}

fn read_registry(app: &AppHandle) -> Result<PluginRegistry, String> {
    let path = registry_path(app)?;
    if !path.exists() {
        return Ok(PluginRegistry::default());
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read plugin registry {}: {}", path.display(), e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse plugin registry {}: {}", path.display(), e))
}

fn write_registry(app: &AppHandle, registry: &PluginRegistry) -> Result<(), String> {
    let path = registry_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create plugin registry directory {}: {}",
                parent.display(),
                e
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Failed to serialize plugin registry: {}", e))?;
    std::fs::write(&path, raw)
        .map_err(|e| format!("Failed to write plugin registry {}: {}", path.display(), e))
}

fn workflow_queue_lock() -> &'static tokio::sync::Mutex<()> {
    PLUGIN_WORKFLOW_QUEUE.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn workflow_registry_for_trigger<'a>(
    registry: &'a PluginRegistry,
    trigger: &str,
) -> Option<&'a PluginTriggerWorkflowRegistry> {
    registry.trigger_workflows.get(trigger)
}

fn sanitize_slug(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in input.trim().chars() {
        let normalized = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'A'..='Z' => Some(ch.to_ascii_lowercase()),
            _ => None,
        };

        if let Some(value) = normalized {
            slug.push(value);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "plugin".to_string()
    } else {
        slug
    }
}

fn install_dir_name(plugin_id: &str, slug: &str) -> String {
    format!("{}-{}", plugin_id, slug)
}

fn installation_path(root: &Path, manifest: &PluginManifest) -> PathBuf {
    root.join(install_dir_name(&manifest.plugin_id, &manifest.slug))
}

fn default_supported_providers(language: &PluginRuntimeLanguage) -> Vec<PluginProvider> {
    match language {
        PluginRuntimeLanguage::Javascript => {
            vec![PluginProvider::Deno, PluginProvider::Node, PluginProvider::Bun]
        }
        PluginRuntimeLanguage::Python => vec![PluginProvider::Python],
    }
}

fn allowed_manifest_triggers() -> &'static [&'static str] {
    &[
        "download.queued",
        "download.beforeStart",
        "download.completed",
        "download.failed",
    ]
}

fn validate_manifest(manifest: &PluginManifest, manifest_path: &Path) -> Result<(), String> {
    if manifest.plugin_id.trim().is_empty() {
        return Err(format!(
            "Plugin manifest {} is missing pluginId",
            manifest_path.display()
        ));
    }
    if manifest.slug.trim().is_empty() {
        return Err(format!(
            "Plugin manifest {} is missing slug",
            manifest_path.display()
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err(format!(
            "Plugin manifest {} is missing name",
            manifest_path.display()
        ));
    }
    if manifest.runtime.entrypoint.trim().is_empty() {
        return Err(format!(
            "Plugin manifest {} is missing runtime.entrypoint",
            manifest_path.display()
        ));
    }
    if manifest.runtime.supported_providers.is_empty() {
        return Err(format!(
            "Plugin manifest {} is missing runtime.supportedProviders",
            manifest_path.display()
        ));
    }
    let allowed = default_supported_providers(&manifest.runtime.language);
    for provider in &manifest.runtime.supported_providers {
        if !allowed.iter().any(|candidate| candidate == provider) {
            return Err(format!(
                "Plugin {} declares unsupported provider {} for language {}",
                manifest.plugin_id,
                provider.as_str(),
                manifest.runtime.language.as_str()
            ));
        }
    }
    if let Some(preferred) = manifest.runtime.preferred_provider.as_ref() {
        if !manifest
            .runtime
            .supported_providers
            .iter()
            .any(|provider| provider == preferred)
        {
            return Err(format!(
                "Plugin {} preferredProvider is not listed in supportedProviders",
                manifest.plugin_id
            ));
        }
    }
    if let Some(compatibility) = manifest.compatibility.as_ref() {
        if let Some(range) = compatibility.app_version.as_ref() {
            if range.trim().is_empty() {
                return Err(format!(
                    "Plugin manifest {} has an empty compatibility.appVersion",
                    manifest_path.display()
                ));
            }
        }
        if let Some(range) = compatibility.sdk_version.as_ref() {
            if range.trim().is_empty() {
                return Err(format!(
                    "Plugin manifest {} has an empty compatibility.sdkVersion",
                    manifest_path.display()
                ));
            }
        }
    }
    if manifest.triggers.is_empty() {
        return Err(format!(
            "Plugin manifest {} must declare at least one trigger",
            manifest_path.display()
        ));
    }
    for trigger in &manifest.triggers {
        if allowed_manifest_triggers().iter().any(|allowed| allowed == trigger) {
            continue;
        }
        if trigger.starts_with("triggers.") {
            return Err(format!(
                "Plugin manifest {} contains invalid trigger {}. plugin.json must use raw runtime names like download.completed, not SDK identifiers like triggers.downloadCompleted",
                manifest_path.display(),
                trigger
            ));
        }
        return Err(format!(
            "Plugin manifest {} contains unsupported trigger {}",
            manifest_path.display(),
            trigger
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SimpleSemver {
    major: u64,
    minor: u64,
    patch: u64,
}

fn parse_simple_semver(version: &str) -> Option<SimpleSemver> {
    let trimmed = version.trim().trim_start_matches('v');
    let mut parts = trimmed.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(SimpleSemver { major, minor, patch })
}

fn compare_simple_semver(a: &str, b: &str) -> Result<std::cmp::Ordering, String> {
    let left = parse_simple_semver(a).ok_or_else(|| format!("Invalid semver: {}", a))?;
    let right = parse_simple_semver(b).ok_or_else(|| format!("Invalid semver: {}", b))?;
    Ok(left.cmp(&right))
}

fn satisfies_version_range(version: &str, range: &str) -> Result<bool, String> {
    let clauses = range
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if clauses.is_empty() {
        return Err("Version range cannot be empty".to_string());
    }

    for clause in clauses {
        let (operator, raw_version) = if let Some(rest) = clause.strip_prefix(">=") {
            (">=", rest)
        } else if let Some(rest) = clause.strip_prefix("<=") {
            ("<=", rest)
        } else if let Some(rest) = clause.strip_prefix('>') {
            (">", rest)
        } else if let Some(rest) = clause.strip_prefix('<') {
            ("<", rest)
        } else if let Some(rest) = clause.strip_prefix('=') {
            ("=", rest)
        } else {
            ("=", clause)
        };

        let ordering = compare_simple_semver(version, raw_version)?;
        let satisfied = match operator {
            ">=" => ordering != std::cmp::Ordering::Less,
            "<=" => ordering != std::cmp::Ordering::Greater,
            ">" => ordering == std::cmp::Ordering::Greater,
            "<" => ordering == std::cmp::Ordering::Less,
            "=" => ordering == std::cmp::Ordering::Equal,
            _ => return Err(format!("Unsupported version operator in clause: {}", clause)),
        };

        if !satisfied {
            return Ok(false);
        }
    }

    Ok(true)
}

fn current_sdk_version() -> String {
    serde_json::from_str::<serde_json::Value>(SDK_JS_PACKAGE_JSON)
        .ok()
        .and_then(|value| value.get("version").and_then(|value| value.as_str()).map(str::to_string))
        .unwrap_or_else(|| "0.1.0".to_string())
}

fn build_scaffold_compatibility_range(version: &str) -> String {
    if let Some(parsed) = parse_simple_semver(version) {
        format!(">={}.{}.{} <{}.{}.0", parsed.major, parsed.minor, parsed.patch, parsed.major, parsed.minor + 1)
    } else {
        format!("={}", version)
    }
}

fn validate_execution_compatibility(manifest: &PluginManifest) -> Result<(), String> {
    validate_install_compatibility(manifest)
}

fn load_manifest_from_dir(plugin_root: &Path) -> Result<PluginManifest, String> {
    let manifest_path = plugin_root.join("plugin.json");
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read {}: {}", manifest_path.display(), e))?;

    let manifest: PluginManifest = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse {}: {}", manifest_path.display(), e))?;

    validate_manifest(&manifest, &manifest_path)?;
    Ok(manifest)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create directory {}: {}", dst.display(), e))?;

    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {}: {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create directory {}: {}", parent.display(), e)
                })?;
            }
            std::fs::copy(&path, &target).map_err(|e| {
                format!(
                    "Failed to copy file {} to {}: {}",
                    path.display(),
                    target.display(),
                    e
                )
            })?;
        }
    }

    Ok(())
}

fn compute_sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn compute_dir_checksum(root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut files = Vec::new();
    while let Some(path) = queue.pop_front() {
        for entry in std::fs::read_dir(&path)
            .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                queue.push_back(entry_path);
            } else if entry_path.is_file() {
                files.push(entry_path);
            }
        }
    }
    files.sort();

    let mut hasher = Sha256::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .to_string();
        hasher.update(relative.as_bytes());
        let mut bytes = Vec::new();
        std::fs::File::open(&file)
            .map_err(|e| format!("Failed to open {}: {}", file.display(), e))?
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read {}: {}", file.display(), e))?;
        hasher.update(&bytes);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn ensure_unique_manifest_file(root: &Path) -> Result<PathBuf, String> {
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut matches = Vec::new();

    while let Some(path) = queue.pop_front() {
        for entry in std::fs::read_dir(&path)
            .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                queue.push_back(entry_path);
            } else if entry_path.file_name() == Some(OsStr::new("plugin.json")) {
                matches.push(entry_path);
            }
        }
    }

    match matches.len() {
        0 => Err(format!(
            "No plugin.json found inside package root {}",
            root.display()
        )),
        1 => Ok(matches.remove(0)),
        _ => Err(format!(
            "Multiple plugin.json files found inside package root {}",
            root.display()
        )),
    }
}

fn extract_zip_to_temp(bytes: &[u8], label: &str) -> Result<PathBuf, String> {
    let temp_root = std::env::temp_dir().join(format!("youwee-plugin-{}-{}", label, Uuid::new_v4()));
    std::fs::create_dir_all(&temp_root).map_err(|e| {
        format!(
            "Failed to create temporary plugin extraction directory {}: {}",
            temp_root.display(),
            e
        )
    })?;

    let cursor = Cursor::new(bytes.to_vec());
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Failed to open plugin zip archive: {}", e))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read zip entry {}: {}", index, e))?;
        let Some(safe_name) = file.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        let outpath = temp_root.join(safe_name);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).map_err(|e| {
                format!(
                    "Failed to create extracted directory {}: {}",
                    outpath.display(),
                    e
                )
            })?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create extracted directory {}: {}", parent.display(), e)
                })?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create extracted file {}: {}", outpath.display(), e))?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| {
                format!("Failed to extract zip entry to {}: {}", outpath.display(), e)
            })?;
        }
    }

    Ok(temp_root)
}

fn prepared_from_folder(path: &Path, kind: PluginPackageSourceKind) -> Result<PreparedPackage, String> {
    if !path.exists() || !path.is_dir() {
        return Err(format!("Plugin folder not found: {}", path.display()));
    }

    let manifest_file = ensure_unique_manifest_file(path)?;
    let package_root = manifest_file
        .parent()
        .ok_or_else(|| "Failed to resolve plugin package root".to_string())?
        .to_path_buf();
    let manifest = load_manifest_from_dir(&package_root)?;
    let checksum = compute_dir_checksum(&package_root).ok();

    Ok(PreparedPackage {
        manifest,
        package_root,
        source: PluginPackageSource {
            kind,
            value: path.to_string_lossy().to_string(),
            checksum,
        },
        warnings: Vec::new(),
    })
}

async fn prepared_from_url(url: &str) -> Result<PreparedPackage, String> {
    let response = Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download plugin package: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Plugin package download failed with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read plugin package response: {}", e))?;
    prepared_from_zip_bytes(&bytes, PluginPackageSourceKind::RemoteUrl, url.to_string())
}

fn prepared_from_zip_file(path: &Path) -> Result<PreparedPackage, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read plugin zip {}: {}", path.display(), e))?;
    prepared_from_zip_bytes(
        &bytes,
        PluginPackageSourceKind::LocalZip,
        path.to_string_lossy().to_string(),
    )
}

fn prepared_from_zip_bytes(
    bytes: &[u8],
    kind: PluginPackageSourceKind,
    value: String,
) -> Result<PreparedPackage, String> {
    let temp_root = extract_zip_to_temp(bytes, "import")?;
    let manifest_file = ensure_unique_manifest_file(&temp_root)?;
    let package_root = manifest_file
        .parent()
        .ok_or_else(|| "Failed to resolve extracted plugin package root".to_string())?
        .to_path_buf();
    let manifest = load_manifest_from_dir(&package_root)?;

    Ok(PreparedPackage {
        manifest,
        package_root,
        source: PluginPackageSource {
            kind,
            value,
            checksum: Some(compute_sha256_bytes(bytes)),
        },
        warnings: vec!["Unsigned plugin package".to_string()],
    })
}

async fn prepare_package(source: &InstallPluginSourceInput) -> Result<PreparedPackage, String> {
    match source.kind {
        PluginPackageSourceKind::LocalFolder => prepared_from_folder(Path::new(&source.value), source.kind.clone()),
        PluginPackageSourceKind::LocalZip => prepared_from_zip_file(Path::new(&source.value)),
        PluginPackageSourceKind::RemoteUrl => prepared_from_url(&source.value).await,
        PluginPackageSourceKind::AppScaffold => {
            prepared_from_folder(Path::new(&source.value), PluginPackageSourceKind::AppScaffold)
        }
    }
}

fn manifest_summary(
    manifest: PluginManifest,
    installation: PluginInstallation,
    warnings: Vec<String>,
) -> PluginSummary {
    PluginSummary {
        manifest,
        installation,
        warnings,
    }
}

fn default_provider_for_language(language: &PluginRuntimeLanguage) -> PluginProvider {
    match language {
        PluginRuntimeLanguage::Javascript => PluginProvider::Deno,
        PluginRuntimeLanguage::Python => PluginProvider::Python,
    }
}

fn build_installation_from_registry(
    registry: &PluginRegistry,
    manifest: &PluginManifest,
    source: PluginPackageSource,
    installed_path: String,
) -> PluginInstallation {
    let entry = registry.installations.get(&manifest.plugin_id);
    let env_value_status = manifest
        .permissions
        .env
        .iter()
        .map(|key| {
            let is_set = entry
                .and_then(|value| value.env_values.get(key))
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            (key.clone(), is_set)
        })
        .collect();
    PluginInstallation {
        plugin_id: manifest.plugin_id.clone(),
        enabled: entry.map(|value| value.enabled).unwrap_or(false),
        trusted: entry.map(|value| value.trusted).unwrap_or(false),
        approved_permissions: entry
            .map(|value| value.approved_permissions.clone())
            .unwrap_or_default(),
        selected_provider: entry
            .and_then(|value| value.selected_provider.clone())
            .or_else(|| manifest.runtime.preferred_provider.clone()),
        installed_path,
        source: entry
            .and_then(|value| value.source.clone())
            .unwrap_or(source),
        last_resolved_provider: entry.and_then(|value| value.last_resolved_provider.clone()),
        last_resolved_source: entry.and_then(|value| value.last_resolved_source.clone()),
        last_execution_status: entry.and_then(|value| value.last_execution_status.clone()),
        last_error: entry.and_then(|value| value.last_error.clone()),
        env_value_status,
    }
}

fn get_trigger_workflow_internal(app: &AppHandle, trigger: &str) -> Result<PluginTriggerWorkflow, String> {
    let registry = read_registry(app)?;
    Ok(PluginTriggerWorkflow {
        trigger: trigger.to_string(),
        steps: workflow_registry_for_trigger(&registry, trigger)
            .map(|workflow| workflow.steps.clone())
            .unwrap_or_default(),
    })
}

fn snapshot_step_from_plugin(
    plugin: &PluginSummary,
    step: &PluginWorkflowStepConfig,
) -> PluginWorkflowStepSnapshot {
    PluginWorkflowStepSnapshot {
        plugin_id: plugin.manifest.plugin_id.clone(),
        plugin_name: plugin.manifest.name.clone(),
        plugin_version: plugin.manifest.version.clone(),
        selected_provider: plugin.installation.selected_provider.clone(),
        approved_permissions: plugin.installation.approved_permissions.clone(),
        failure_policy: step.failure_policy.clone(),
    }
}

fn resolve_workflow_step_snapshots(
    plugins_by_id: &BTreeMap<String, PluginSummary>,
    workflow: &PluginTriggerWorkflow,
) -> Vec<PluginWorkflowStepSnapshot> {
    workflow
        .steps
        .iter()
        .filter_map(|step| {
            plugins_by_id
                .get(&step.plugin_id)
                .filter(|plugin| plugin.installation.enabled)
                .map(|plugin| snapshot_step_from_plugin(plugin, step))
        })
        .collect()
}

fn build_legacy_workflow_snapshots(
    plugins_by_id: &BTreeMap<String, PluginSummary>,
    registry: &PluginRegistry,
    trigger: &str,
    plugin_ids: &[String],
) -> Vec<PluginWorkflowStepSnapshot> {
    let workflow = workflow_registry_for_trigger(registry, trigger);

    plugin_ids
        .iter()
        .filter_map(|plugin_id| {
            let plugin = plugins_by_id.get(plugin_id)?;
            let step = workflow
                .and_then(|workflow| workflow.steps.iter().find(|step| step.plugin_id == *plugin_id))
                .cloned()
                .unwrap_or(PluginWorkflowStepConfig {
                    plugin_id: plugin_id.clone(),
                    failure_policy: PluginWorkflowFailurePolicy::Continue,
                });
            Some(snapshot_step_from_plugin(plugin, &step))
        })
        .collect()
}

fn build_chain_state(payload: &PostDownloadPluginPayload) -> PluginChainState {
    PluginChainState {
        job_id: payload.job_id.clone(),
        source: payload.source.clone(),
        download_kind: payload.download_kind.clone(),
        url: payload.url.clone(),
        title: payload.title.clone(),
        thumbnail: payload.thumbnail.clone(),
        history_id: payload.history_id.clone(),
        time_range: payload.time_range.clone(),
        active_filepath: payload.filepath.clone(),
        active_filename: payload.filename.clone(),
        directory: payload.directory.clone(),
        filesize: payload.filesize,
        format: payload.format.clone(),
        quality: payload.quality.clone(),
        extra_files: Vec::new(),
        metadata: None,
    }
}

fn payload_from_chain_state(
    payload: &PostDownloadPluginPayload,
    workflow_run_id: &str,
    step_index: usize,
    step_plugin_id: &str,
    chain_state: &PluginChainState,
) -> PostDownloadPluginPayload {
    PostDownloadPluginPayload {
        job_id: payload.job_id.clone(),
        source: payload.source.clone(),
        trigger: payload.trigger.clone(),
        filepath: chain_state.active_filepath.clone(),
        filename: chain_state.active_filename.clone(),
        directory: chain_state.directory.clone(),
        filesize: chain_state.filesize,
        format: chain_state.format.clone(),
        quality: chain_state.quality.clone(),
        url: chain_state.url.clone(),
        title: chain_state.title.clone(),
        thumbnail: chain_state.thumbnail.clone(),
        history_id: chain_state.history_id.clone(),
        time_range: chain_state.time_range.clone(),
        download_kind: chain_state.download_kind.clone(),
        workflow_run_id: Some(workflow_run_id.to_string()),
        workflow_step_index: Some(step_index),
        workflow_step_plugin_id: Some(step_plugin_id.to_string()),
        chain_state: Some(chain_state.clone()),
    }
}

fn merge_chain_mutation(chain_state: &mut PluginChainState, mutation: &PluginChainMutation) {
    if let Some(active_filepath) = mutation.active_filepath.as_ref() {
        chain_state.active_filepath = active_filepath.clone();
        let path = Path::new(active_filepath);
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            chain_state.active_filename = filename.to_string();
        }
        if let Some(directory) = path.parent() {
            chain_state.directory = directory.to_string_lossy().to_string();
        }
    }
    if let Some(active_filename) = mutation.active_filename.as_ref() {
        chain_state.active_filename = active_filename.clone();
    }
    if !mutation.extra_files.is_empty() {
        for file in &mutation.extra_files {
            if !chain_state.extra_files.iter().any(|existing| existing == file) {
                chain_state.extra_files.push(file.clone());
            }
        }
    }
    if let Some(metadata_patch) = mutation.metadata_patch.as_ref() {
        match chain_state.metadata.as_mut() {
            Some(Value::Object(current)) => {
                if let Value::Object(patch) = metadata_patch {
                    for (key, value) in patch {
                        current.insert(key.clone(), value.clone());
                    }
                } else {
                    chain_state.metadata = Some(metadata_patch.clone());
                }
            }
            _ => {
                chain_state.metadata = Some(metadata_patch.clone());
            }
        }
    }
}

fn collect_compatibility_issues(manifest: &PluginManifest) -> Result<Vec<String>, String> {
    let Some(compatibility) = manifest.compatibility.as_ref() else {
        return Ok(Vec::new());
    };

    let mut issues = Vec::new();

    if let Some(range) = compatibility.app_version.as_ref() {
        if !satisfies_version_range(env!("CARGO_PKG_VERSION"), range)? {
            issues.push(format!(
                "Requires Youwee app version {} but current app version is {}",
                range,
                env!("CARGO_PKG_VERSION")
            ));
        }
    }

    if let Some(range) = compatibility.sdk_version.as_ref() {
        let sdk_version = current_sdk_version();
        if !satisfies_version_range(&sdk_version, range)? {
            issues.push(format!(
                "Requires youwee-sdk version {} but bundled SDK version is {}",
                range, sdk_version
            ));
        }
    }

    Ok(issues)
}

fn validate_install_compatibility(manifest: &PluginManifest) -> Result<(), String> {
    let issues = collect_compatibility_issues(manifest)?;
    if issues.is_empty() {
        return Ok(());
    }

    Err(format!(
        "Plugin is not compatible with this Youwee build:\n- {}",
        issues.join("\n- ")
    ))
}

pub fn list_plugins_internal(app: &AppHandle) -> Result<Vec<PluginSummary>, String> {
    let root = ensure_plugins_root(app)?;
    let registry = read_registry(app)?;
    let mut plugins = Vec::new();

    for entry in std::fs::read_dir(&root)
        .map_err(|e| format!("Failed to read plugins directory {}: {}", root.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read plugin entry: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest = match load_manifest_from_dir(&path) {
            Ok(manifest) => manifest,
            Err(error) => {
                add_log_internal("error", "Invalid plugin manifest", Some(&error), None).ok();
                continue;
            }
        };
        let checksum = compute_dir_checksum(&path).ok();
        let installation = build_installation_from_registry(
            &registry,
            &manifest,
            PluginPackageSource {
                kind: PluginPackageSourceKind::LocalFolder,
                value: path.to_string_lossy().to_string(),
                checksum,
            },
            path.to_string_lossy().to_string(),
        );
        let warnings = collect_compatibility_issues(&manifest).unwrap_or_else(|error| vec![error]);
        plugins.push(manifest_summary(manifest, installation, warnings));
    }

    plugins.sort_by(|left, right| {
        left.manifest
            .name
            .to_lowercase()
            .cmp(&right.manifest.name.to_lowercase())
            .then_with(|| left.manifest.plugin_id.cmp(&right.manifest.plugin_id))
    });

    Ok(plugins)
}

pub fn get_plugin_details_internal(app: &AppHandle, plugin_id: &str) -> Result<PluginSummary, String> {
    list_plugins_internal(app)?
        .into_iter()
        .find(|plugin| plugin.manifest.plugin_id == plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))
}

pub async fn inspect_plugin_folder_internal(
    _app: &AppHandle,
    path: String,
) -> Result<PluginPackageInspection, String> {
    let package = prepared_from_folder(Path::new(&path), PluginPackageSourceKind::LocalFolder)?;
    let mut warnings = package.warnings;
    warnings.extend(collect_compatibility_issues(&package.manifest)?);
    Ok(PluginPackageInspection {
        manifest: package.manifest,
        source: package.source,
        warnings,
    })
}

pub async fn inspect_plugin_zip_internal(
    _app: &AppHandle,
    path: String,
) -> Result<PluginPackageInspection, String> {
    let package = prepared_from_zip_file(Path::new(&path))?;
    let mut warnings = package.warnings;
    warnings.extend(collect_compatibility_issues(&package.manifest)?);
    Ok(PluginPackageInspection {
        manifest: package.manifest,
        source: package.source,
        warnings,
    })
}

pub async fn inspect_plugin_url_internal(
    _app: &AppHandle,
    url: String,
) -> Result<PluginPackageInspection, String> {
    let package = prepared_from_url(&url).await?;
    let mut warnings = package.warnings;
    warnings.extend(collect_compatibility_issues(&package.manifest)?);
    Ok(PluginPackageInspection {
        manifest: package.manifest,
        source: package.source,
        warnings,
    })
}

pub async fn install_plugin_internal(
    app: &AppHandle,
    source: InstallPluginSourceInput,
    trusted: bool,
) -> Result<PluginSummary, String> {
    let package = prepare_package(&source).await?;
    if let Err(error) = validate_install_compatibility(&package.manifest) {
        add_log_internal(
            "error",
            &format!("Plugin install blocked: {}", package.manifest.name),
            Some(&error),
            None,
        )
        .ok();
        return Err(error);
    }
    let root = ensure_plugins_root(app)?;
    let destination = installation_path(&root, &package.manifest);
    if destination.exists() {
        std::fs::remove_dir_all(&destination).map_err(|e| {
            format!(
                "Failed to replace existing plugin installation {}: {}",
                destination.display(),
                e
            )
        })?;
    }
    copy_dir_recursive(&package.package_root, &destination)?;

    let mut registry = read_registry(app)?;
    let selected_provider = package.manifest.runtime.preferred_provider.clone();
    registry.installations.insert(
        package.manifest.plugin_id.clone(),
        PluginRegistryEntry {
            enabled: false,
            trusted,
            approved_permissions: PluginPermissionApproval::default(),
            env_values: BTreeMap::new(),
            selected_provider,
            source: Some(package.source.clone()),
            last_resolved_provider: None,
            last_resolved_source: None,
            last_execution_status: Some("installed".to_string()),
            last_error: None,
        },
    );
    write_registry(app, &registry)?;

    let installation = build_installation_from_registry(
        &registry,
        &package.manifest,
        package.source,
        destination.to_string_lossy().to_string(),
    );

    add_log_internal(
        "info",
        &format!("Installed plugin: {}", package.manifest.name),
        Some(&format!("Plugin ID: {}", package.manifest.plugin_id)),
        None,
    )
    .ok();

    Ok(manifest_summary(package.manifest, installation, Vec::new()))
}

pub fn create_plugin_scaffold_internal(
    app: &AppHandle,
    input: CreatePluginScaffoldInput,
) -> Result<PluginSummary, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Plugin name cannot be empty".to_string());
    }

    let plugin_id = Uuid::new_v4().to_string();
    let slug = input.slug.as_deref().map(sanitize_slug).unwrap_or_else(|| sanitize_slug(name));
    let manifest = PluginManifest {
        plugin_id: plugin_id.clone(),
        slug: slug.clone(),
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: Some("Describe what this plugin does.".to_string()),
        author: None,
        homepage: None,
        repository: None,
        license: Some("MIT".to_string()),
        runtime: PluginRuntimeSpec {
            language: PluginRuntimeLanguage::Javascript,
            supported_providers: vec![PluginProvider::Node, PluginProvider::Bun],
            preferred_provider: Some(PluginProvider::Node),
            entrypoint: "src/plugin.js".to_string(),
        },
        compatibility: Some(PluginCompatibilitySpec {
            app_version: Some(build_scaffold_compatibility_range(env!("CARGO_PKG_VERSION"))),
            sdk_version: Some(build_scaffold_compatibility_range(&current_sdk_version())),
        }),
        triggers: vec!["download.completed".to_string()],
        permissions: PluginPermissionRequest::default(),
        timeout_sec: 60,
        readme: Some("README.md".to_string()),
        checksum: None,
        published_at: None,
    };

    let root = ensure_plugins_root(app)?;
    let destination = installation_path(&root, &manifest);
    if destination.exists() {
        return Err(format!(
            "Plugin scaffold destination already exists: {}",
            destination.display()
        ));
    }
    std::fs::create_dir_all(destination.join("src")).map_err(|e| {
        format!(
            "Failed to create plugin scaffold directory {}: {}",
            destination.display(),
            e
        )
    })?;
    std::fs::create_dir_all(destination.join("examples")).map_err(|e| {
        format!(
            "Failed to create plugin examples directory {}: {}",
            destination.join("examples").display(),
            e
        )
    })?;
    std::fs::create_dir_all(destination.join("vendor").join("youwee-sdk")).map_err(|e| {
        format!(
            "Failed to create vendored SDK directory {}: {}",
            destination.join("vendor").join("youwee-sdk").display(),
            e
        )
    })?;

    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize plugin manifest: {}", e))?;
    std::fs::write(destination.join("plugin.json"), manifest_json).map_err(|e| {
        format!(
            "Failed to write plugin manifest {}: {}",
            destination.join("plugin.json").display(),
            e
        )
    })?;
    std::fs::write(
        destination.join("package.json"),
        build_scaffold_package_json(&manifest),
    )
    .map_err(|e| {
        format!(
            "Failed to write plugin package.json {}: {}",
            destination.join("package.json").display(),
            e
        )
    })?;
    std::fs::write(destination.join("src").join("plugin.js"), build_scaffold_plugin_module()).map_err(|e| {
        format!(
            "Failed to write plugin module {}: {}",
            destination.join("src").join("plugin.js").display(),
            e
        )
    })?;
    write_scaffold_sdk_package(&destination)?;
    std::fs::write(destination.join("README.md"), build_scaffold_readme(&manifest)).map_err(|e| {
        format!(
            "Failed to write plugin README {}: {}",
            destination.join("README.md").display(),
            e
        )
    })?;
    std::fs::write(destination.join("CHANGELOG.md"), build_scaffold_changelog()).map_err(|e| {
        format!(
            "Failed to write plugin changelog {}: {}",
            destination.join("CHANGELOG.md").display(),
            e
        )
    })?;
    std::fs::write(destination.join(".gitignore"), "dist/\nnode_modules/\n").map_err(|e| {
        format!(
            "Failed to write plugin gitignore {}: {}",
            destination.join(".gitignore").display(),
            e
        )
    })?;
    let payload = sample_download_payload();
    let payload_json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize sample payload: {}", e))?;
    std::fs::write(
        destination.join("examples").join("payload.download.completed.json"),
        payload_json,
    )
    .map_err(|e| {
        format!(
            "Failed to write sample payload {}: {}",
            destination
                .join("examples")
                .join("payload.download.completed.json")
                .display(),
            e
        )
    })?;
    std::fs::write(
        destination.join("examples").join("result.success.json"),
        build_scaffold_success_result_example(),
    )
    .map_err(|e| {
        format!(
            "Failed to write sample success result {}: {}",
            destination
                .join("examples")
                .join("result.success.json")
                .display(),
            e
        )
    })?;
    std::fs::write(
        destination.join("examples").join("result.failure.json"),
        build_scaffold_failure_result_example(),
    )
    .map_err(|e| {
        format!(
            "Failed to write sample failure result {}: {}",
            destination
                .join("examples")
                .join("result.failure.json")
                .display(),
            e
        )
    })?;

    let mut registry = read_registry(app)?;
    registry.installations.insert(
        manifest.plugin_id.clone(),
        PluginRegistryEntry {
            enabled: false,
            trusted: true,
            approved_permissions: PluginPermissionApproval::default(),
            env_values: BTreeMap::new(),
            selected_provider: manifest.runtime.preferred_provider.clone(),
            source: Some(PluginPackageSource {
                kind: PluginPackageSourceKind::AppScaffold,
                value: destination.to_string_lossy().to_string(),
                checksum: compute_dir_checksum(&destination).ok(),
            }),
            last_resolved_provider: None,
            last_resolved_source: None,
            last_execution_status: Some("created".to_string()),
            last_error: None,
        },
    );
    write_registry(app, &registry)?;

    let installation = build_installation_from_registry(
        &registry,
        &manifest,
        PluginPackageSource {
            kind: PluginPackageSourceKind::AppScaffold,
            value: destination.to_string_lossy().to_string(),
            checksum: compute_dir_checksum(&destination).ok(),
        },
        destination.to_string_lossy().to_string(),
    );

    Ok(manifest_summary(manifest, installation, Vec::new()))
}

pub fn update_plugin_state_internal(
    app: &AppHandle,
    plugin_id: &str,
    enabled: bool,
) -> Result<(), String> {
    let mut registry = read_registry(app)?;
    let entry = registry
        .installations
        .get_mut(plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;
    entry.enabled = enabled;
    write_registry(app, &registry)
}

pub fn get_plugin_trigger_workflow_internal(
    app: &AppHandle,
    trigger: &str,
) -> Result<PluginTriggerWorkflow, String> {
    get_trigger_workflow_internal(app, trigger)
}

pub fn update_plugin_trigger_workflow_internal(
    app: &AppHandle,
    workflow: PluginTriggerWorkflow,
) -> Result<PluginTriggerWorkflow, String> {
    let valid_plugin_ids = list_plugins_internal(app)?
        .into_iter()
        .map(|plugin| plugin.manifest.plugin_id)
        .collect::<Vec<_>>();

    let steps = workflow
        .steps
        .into_iter()
        .filter(|step| valid_plugin_ids.iter().any(|plugin_id| plugin_id == &step.plugin_id))
        .collect::<Vec<_>>();

    let mut registry = read_registry(app)?;
    registry.trigger_workflows.insert(
        workflow.trigger.clone(),
        PluginTriggerWorkflowRegistry {
            steps: steps.clone(),
        },
    );
    write_registry(app, &registry)?;

    Ok(PluginTriggerWorkflow {
        trigger: workflow.trigger,
        steps,
    })
}

pub fn approve_plugin_permissions_internal(
    app: &AppHandle,
    plugin_id: &str,
    permissions: PluginPermissionApprovalInput,
) -> Result<(), String> {
    let mut registry = read_registry(app)?;
    let entry = registry
        .installations
        .get_mut(plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;
    entry.approved_permissions = PluginPermissionApproval {
        network: permissions.network,
        read_paths: permissions.read_paths,
        write_paths: permissions.write_paths,
        env: permissions.env,
    };
    write_registry(app, &registry)
}

pub fn update_plugin_env_values_internal(
    app: &AppHandle,
    plugin_id: &str,
    input: PluginEnvValuesInput,
) -> Result<(), String> {
    let plugin = get_plugin_details_internal(app, plugin_id)?;
    let allowed_keys = &plugin.manifest.permissions.env;

    let mut registry = read_registry(app)?;
    let entry = registry
        .installations
        .get_mut(plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

    for (key, value) in input.values {
        if !allowed_keys.iter().any(|allowed| allowed == &key) {
            return Err(format!("Plugin does not request env key: {}", key));
        }

        match value {
            Some(raw) if !raw.trim().is_empty() => {
                entry.env_values.insert(key, raw);
            }
            _ => {
                entry.env_values.remove(&key);
            }
        }
    }

    write_registry(app, &registry)
}

pub fn set_plugin_trust_internal(
    app: &AppHandle,
    plugin_id: &str,
    trusted: bool,
) -> Result<(), String> {
    let mut registry = read_registry(app)?;
    let entry = registry
        .installations
        .get_mut(plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;
    entry.trusted = trusted;
    if !trusted {
        entry.enabled = false;
    }
    write_registry(app, &registry)
}

pub fn set_plugin_provider_internal(
    app: &AppHandle,
    plugin_id: &str,
    provider: PluginProvider,
) -> Result<(), String> {
    let plugin = get_plugin_details_internal(app, plugin_id)?;
    if !plugin
        .manifest
        .runtime
        .supported_providers
        .iter()
        .any(|candidate| candidate == &provider)
    {
        return Err(format!(
            "Plugin {} does not support provider {}",
            plugin.manifest.plugin_id,
            provider.as_str()
        ));
    }

    let mut registry = read_registry(app)?;
    let entry = registry
        .installations
        .get_mut(plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;
    entry.selected_provider = Some(provider);
    write_registry(app, &registry)
}

pub fn set_default_provider_for_language_internal(
    app: &AppHandle,
    language: PluginRuntimeLanguage,
    provider: PluginProvider,
) -> Result<(), String> {
    let allowed = default_supported_providers(&language);
    if !allowed.iter().any(|candidate| candidate == &provider) {
        return Err(format!(
            "Provider {} is not valid for language {}",
            provider.as_str(),
            language.as_str()
        ));
    }
    let mut registry = read_registry(app)?;
    registry
        .default_providers
        .insert(language.as_str().to_string(), provider);
    write_registry(app, &registry)
}

pub async fn open_plugin_directory_internal(app: &AppHandle, plugin_id: &str) -> Result<(), String> {
    let plugin = get_plugin_details_internal(app, plugin_id)?;
    let path = plugin.installation.installed_path.clone();

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open plugin directory: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("explorer");
        cmd.arg(&path);
        cmd.hide_window();
        cmd.spawn()
            .map_err(|e| format!("Failed to open plugin directory: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open plugin directory: {}", e))?;
    }

    Ok(())
}

async fn resolve_command_path(binary: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    let locator = "which";
    #[cfg(windows)]
    let locator = "where";

    let mut cmd = Command::new(locator);
    cmd.arg(binary);
    cmd.hide_window();
    let output = cmd.output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let first_line = String::from_utf8_lossy(&output.stdout).lines().next()?.trim().to_string();
    if first_line.is_empty() {
        None
    } else {
        Some(PathBuf::from(first_line))
    }
}

pub async fn get_runtime_provider_status_internal(
    app: &AppHandle,
    provider: PluginProvider,
) -> RuntimeProviderStatus {
    match provider {
        PluginProvider::Deno => {
            let path = crate::services::get_deno_path(app).await;
            let (resolved_path, resolved_source) = if let Some(path) = path.clone() {
                let source = match app.path().app_data_dir() {
                    Ok(app_data) if path.starts_with(app_data.join("bin")) => "app-managed",
                    _ => "system",
                };
                (Some(path.to_string_lossy().to_string()), Some(source.to_string()))
            } else {
                (None, None)
            };
            RuntimeProviderStatus {
                provider,
                available: resolved_path.is_some(),
                resolved_path,
                resolved_source,
                details: Some("Resolves app-managed Deno first, then system Deno.".to_string()),
            }
        }
        PluginProvider::Node => {
            let path = resolve_command_path("node").await;
            RuntimeProviderStatus {
                provider,
                available: path.is_some(),
                resolved_path: path.map(|value| value.to_string_lossy().to_string()),
                resolved_source: Some("system".to_string()),
                details: None,
            }
        }
        PluginProvider::Bun => {
            let path = resolve_command_path("bun").await;
            RuntimeProviderStatus {
                provider,
                available: path.is_some(),
                resolved_path: path.map(|value| value.to_string_lossy().to_string()),
                resolved_source: Some("system".to_string()),
                details: None,
            }
        }
        PluginProvider::Python => {
            let path = if let Some(path) = resolve_command_path("python3").await {
                Some(path)
            } else {
                resolve_command_path("python").await
            };
            RuntimeProviderStatus {
                provider,
                available: path.is_some(),
                resolved_path: path.map(|value| value.to_string_lossy().to_string()),
                resolved_source: Some("system".to_string()),
                details: None,
            }
        }
    }
}

pub async fn list_runtime_providers_internal(app: &AppHandle) -> Vec<RuntimeProviderStatus> {
    let mut statuses = Vec::new();
    for provider in [
        PluginProvider::Deno,
        PluginProvider::Node,
        PluginProvider::Bun,
        PluginProvider::Python,
    ] {
        statuses.push(get_runtime_provider_status_internal(app, provider).await);
    }
    statuses
}

fn resolve_plugin_entrypoint(plugin_dir: &Path, entrypoint: &str) -> Result<PathBuf, String> {
    let entrypoint_path = PathBuf::from(entrypoint);
    if entrypoint_path.is_absolute() {
        return Err("Plugin entrypoint must be relative".to_string());
    }
    let candidate = plugin_dir.join(entrypoint_path);
    let canonical_plugin_dir = std::fs::canonicalize(plugin_dir).map_err(|e| {
        format!(
            "Failed to resolve plugin directory {}: {}",
            plugin_dir.display(),
            e
        )
    })?;
    let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|e| {
        format!(
            "Failed to resolve plugin entrypoint {}: {}",
            candidate.display(),
            e
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_plugin_dir) {
        return Err("Plugin entrypoint must stay inside the plugin directory".to_string());
    }
    if !canonical_candidate.is_file() {
        return Err(format!(
            "Plugin entrypoint {} is not a file",
            canonical_candidate.display()
        ));
    }
    Ok(canonical_candidate)
}

fn collect_missing_permissions(
    requested: &PluginPermissionRequest,
    approved: &PluginPermissionApproval,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if requested.network && !approved.network {
        missing.push("network");
    }
    if !requested.read_paths.is_empty() && !approved.read_paths {
        missing.push("readPaths");
    }
    if !requested.write_paths.is_empty() && !approved.write_paths {
        missing.push("writePaths");
    }
    if !requested.env.is_empty() && !approved.env {
        missing.push("env");
    }
    missing
}

fn resolve_permission_paths(plugin_dir: &Path, raw_paths: &[String], read: bool) -> Result<Vec<PathBuf>, String> {
    let mut resolved = Vec::new();
    for raw in raw_paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        let candidate = if path.is_absolute() {
            path
        } else {
            plugin_dir.join(path)
        };
        if read {
            let canonical = std::fs::canonicalize(&candidate)
                .map_err(|e| format!("Failed to resolve path {}: {}", candidate.display(), e))?;
            resolved.push(canonical);
        } else {
            resolved.push(candidate);
        }
    }
    Ok(resolved)
}

fn push_allow_flag(args: &mut Vec<String>, flag_name: &str, values: &[PathBuf]) {
    if values.is_empty() {
        return;
    }
    args.push(format!(
        "--{}={}",
        flag_name,
        values
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
}

async fn resolve_provider_command(
    app: &AppHandle,
    provider: &PluginProvider,
) -> Result<(String, Option<String>), String> {
    match provider {
        PluginProvider::Deno => {
            let path = crate::services::get_deno_path(app)
                .await
                .ok_or_else(|| "Deno runtime is not available".to_string())?;
            let source = match app.path().app_data_dir() {
                Ok(app_data) if path.starts_with(app_data.join("bin")) => "app-managed",
                _ => "system",
            };
            Ok((path.to_string_lossy().to_string(), Some(source.to_string())))
        }
        PluginProvider::Node => resolve_command_path("node")
            .await
            .map(|path| (path.to_string_lossy().to_string(), Some("system".to_string())))
            .ok_or_else(|| "Node runtime is not available".to_string()),
        PluginProvider::Bun => resolve_command_path("bun")
            .await
            .map(|path| (path.to_string_lossy().to_string(), Some("system".to_string())))
            .ok_or_else(|| "Bun runtime is not available".to_string()),
        PluginProvider::Python => {
            let path = if let Some(path) = resolve_command_path("python3").await {
                Some(path)
            } else {
                resolve_command_path("python").await
            };
            path.map(|value| (value.to_string_lossy().to_string(), Some("system".to_string())))
                .ok_or_else(|| "Python runtime is not available".to_string())
        }
    }
}

fn emit_plugin_runtime_output(
    app: &AppHandle,
    plugin_id: &str,
    plugin_name: &str,
    run_id: Option<&str>,
    stream: &str,
    bytes: &[u8],
    log_url: Option<&str>,
    media_title: Option<&str>,
    filename: Option<&str>,
) {
    if bytes.is_empty() {
        return;
    }
    let chunk = String::from_utf8_lossy(bytes).to_string();
    let trimmed = chunk.trim_end_matches(&['\n', '\r'][..]).trim();
    if trimmed.is_empty() {
        return;
    }

    let log_type = if stream == "stdout" {
        "info"
    } else {
        match trimmed
            .trim_start()
            .strip_prefix('[')
            .and_then(|value| value.split_once(']').map(|(level, _)| level.to_ascii_lowercase()))
        {
            Some(level) if level == "info" || level == "debug" => "info",
            Some(level) if level == "warn" => "stderr",
            Some(level) if level == "error" => "error",
            _ => "stderr",
        }
    };
    let details = format!("pluginId: {} | pluginName: {} | stream: {}", plugin_id, plugin_name, stream);
    add_log_internal(log_type, &chunk, Some(&details), log_url).ok();

    app.emit(
        "plugin-execution-output",
        PluginExecutionOutputEvent {
            plugin_id: plugin_id.to_string(),
            run_id: run_id.map(|value| value.to_string()),
            plugin_name: Some(plugin_name.to_string()),
            stream: stream.to_string(),
            chunk: trimmed.to_string(),
            media_title: media_title.map(str::to_string),
            filename: filename.map(str::to_string),
            media_url: log_url.map(str::to_string),
        },
    )
    .ok();
}

async fn capture_process_stream<R>(
    app: AppHandle,
    stream_name: &str,
    plugin_id: String,
    plugin_name: String,
    run_id: Option<String>,
    mut reader: R,
    log_url: Option<String>,
    media_title: Option<String>,
    filename: Option<String>,
) -> Vec<u8>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(size) => {
                raw.extend_from_slice(&buffer[..size]);
                emit_plugin_runtime_output(
                    &app,
                    &plugin_id,
                    &plugin_name,
                    run_id.as_deref(),
                    stream_name,
                    &buffer[..size],
                    log_url.as_deref(),
                    media_title.as_deref(),
                    filename.as_deref(),
                );
            }
            Err(_) => break,
        }
    }

    raw
}

fn output_to_string(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    text.trim_end_matches(&['\r', '\n'][..]).to_string()
}

fn plugin_output_details(stdout: &str, stderr: &str) -> String {
    let mut parts = Vec::new();
    if !stdout.trim().is_empty() {
        parts.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }

    if parts.is_empty() {
        "No output captured from plugin process.".to_string()
    } else {
        parts.join("\n\n")
    }
}

fn parse_plugin_result(stdout: &str) -> Option<PluginScriptOutput> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(output) = serde_json::from_str::<PluginScriptOutput>(trimmed) {
        return Some(output);
    }

    trimmed
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .and_then(|line| serde_json::from_str::<PluginScriptOutput>(line).ok())
}

#[cfg(unix)]
fn plugin_exit_reason(status: &std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    if let Some(code) = status.code() {
        format!("code {}", code)
    } else if let Some(signal) = status.signal() {
        format!("signal {}", signal)
    } else {
        "terminated".to_string()
    }
}

#[cfg(not(unix))]
fn plugin_exit_reason(status: &std::process::ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "terminated".to_string(), |code| format!("code {}", code))
}

async fn capture_process_stream_err(
    app: AppHandle,
    stream_name: &str,
    plugin_id: String,
    plugin_name: String,
    run_id: Option<String>,
    reader: tokio::process::ChildStderr,
    log_url: Option<String>,
    media_title: Option<String>,
    filename: Option<String>,
) -> Vec<u8> {
    capture_process_stream(
        app,
        stream_name,
        plugin_id,
        plugin_name,
        run_id,
        reader,
        log_url,
        media_title,
        filename,
    )
    .await
}

async fn execute_plugin(
    app: &AppHandle,
    plugin: &PluginSummary,
    run_id: &str,
    payload: &PostDownloadPluginPayload,
) -> Result<(PluginExecutionResult, PluginProvider, Option<String>), String> {
    let selected_provider = plugin
        .installation
        .selected_provider
        .clone()
        .or(plugin.manifest.runtime.preferred_provider.clone())
        .unwrap_or_else(|| default_provider_for_language(&plugin.manifest.runtime.language));
    if !plugin
        .manifest
        .runtime
        .supported_providers
        .iter()
        .any(|provider| provider == &selected_provider)
    {
        return Err(format!(
            "Plugin does not support provider {}",
            selected_provider.as_str()
        ));
    }

    let missing_permissions =
        collect_missing_permissions(&plugin.manifest.permissions, &plugin.installation.approved_permissions);
    if !missing_permissions.is_empty() {
        return Err(format!(
            "Plugin requires unapproved permissions: {}",
            missing_permissions.join(", ")
        ));
    }

    validate_execution_compatibility(&plugin.manifest)?;

    let plugin_dir = PathBuf::from(&plugin.installation.installed_path);
    let entrypoint = resolve_plugin_entrypoint(&plugin_dir, &plugin.manifest.runtime.entrypoint)?;
    let (command_path, resolved_source) = resolve_provider_command(app, &selected_provider).await?;
    let registry = read_registry(app).unwrap_or_default();
    let env_values = registry
        .installations
        .get(&plugin.manifest.plugin_id)
        .map(|entry| entry.env_values.clone())
        .unwrap_or_default();
    let ffmpeg_path = crate::services::get_ffmpeg_path(app)
        .await
        .map(|path| path.to_string_lossy().to_string());
    let ytdlp_path = crate::services::get_ytdlp_path(app)
        .await
        .map(|(path, _)| path.to_string_lossy().to_string());
    let ai_config = crate::commands::get_ai_config(app.clone()).await.unwrap_or_default();

    let payload_json = serde_json::to_vec(payload)
        .map_err(|e| format!("Failed to serialize plugin payload: {}", e))?;
    let payload_file = std::fs::canonicalize(PathBuf::from(&payload.filepath))
        .unwrap_or_else(|_| PathBuf::from(&payload.filepath));

    let mut command_args = Vec::<String>::new();
    match selected_provider {
        PluginProvider::Deno => {
            let mut allow_read = vec![plugin_dir.clone(), payload_file.clone()];
            if let Some(parent) = payload_file.parent() {
                allow_read.push(parent.to_path_buf());
            }
            allow_read.extend(resolve_permission_paths(
                &plugin_dir,
                &plugin.manifest.permissions.read_paths,
                true,
            )?);
            let allow_write = resolve_permission_paths(
                &plugin_dir,
                &plugin.manifest.permissions.write_paths,
                false,
            )?;

            command_args.push("run".to_string());
            command_args.push("--quiet".to_string());
            if plugin.manifest.permissions.network {
                command_args.push("--allow-net".to_string());
            }
            push_allow_flag(&mut command_args, "allow-read", &allow_read);
            if !allow_write.is_empty() {
                push_allow_flag(&mut command_args, "allow-write", &allow_write);
            }
            if !plugin.manifest.permissions.env.is_empty() {
                command_args.push(format!(
                    "--allow-env={}",
                    plugin.manifest.permissions.env.join(",")
                ));
            }
            command_args.push(entrypoint.to_string_lossy().to_string());
        }
        PluginProvider::Node | PluginProvider::Bun => {
            let runtime_cli = plugin_dir
                .join("vendor")
                .join("youwee-sdk")
                .join("dist")
                .join("runtime-cli.js");
            command_args.push(runtime_cli.to_string_lossy().to_string());
        }
        PluginProvider::Python => {
            command_args.push(entrypoint.to_string_lossy().to_string());
        }
    }

    let mut cmd = Command::new(&command_path);
    cmd.args(&command_args)
        .current_dir(&plugin_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.hide_window();
    cmd.env("YOUWEE_PLUGIN_TIMEOUT_MS", plugin.manifest.timeout_sec.saturating_mul(1000).to_string());
    cmd.env("YOUWEE_PLUGIN_ID", &plugin.manifest.plugin_id);
    cmd.env("YOUWEE_PLUGIN_SLUG", &plugin.manifest.slug);
    cmd.env("YOUWEE_PLUGIN_NAME", &plugin.manifest.name);
    cmd.env("YOUWEE_PLUGIN_VERSION", &plugin.manifest.version);
    cmd.env("YOUWEE_APP_VERSION", env!("CARGO_PKG_VERSION"));
    cmd.env("YOUWEE_PLUGIN_LANGUAGE", plugin.manifest.runtime.language.as_str());
    cmd.env("YOUWEE_PLUGIN_PROVIDER", selected_provider.as_str());
    cmd.env("YOUWEE_PLUGIN_MAIN", entrypoint.to_string_lossy().to_string());
    if let Some(source) = resolved_source.as_ref() {
        cmd.env("YOUWEE_PLUGIN_PROVIDER_SOURCE", source);
    }
    if let Some(path) = ffmpeg_path.as_ref() {
        cmd.env("YOUWEE_FFMPEG_PATH", path);
    }
    if let Some(path) = ytdlp_path.as_ref() {
        cmd.env("YOUWEE_YTDLP_PATH", path);
    }
    cmd.env("YOUWEE_AI_ENABLED", if ai_config.enabled { "true" } else { "false" });
    cmd.env(
        "YOUWEE_AI_PROVIDER",
        serde_json::to_value(&ai_config.provider)
            .ok()
            .and_then(|value| value.as_str().map(|value| value.to_string()))
            .unwrap_or_else(|| "gemini".to_string()),
    );
    cmd.env("YOUWEE_AI_MODEL", &ai_config.model);
    if let Some(value) = ai_config.api_key.as_ref() {
        cmd.env("YOUWEE_AI_API_KEY", value);
    }
    if let Some(value) = ai_config.proxy_url.as_ref() {
        cmd.env("YOUWEE_AI_PROXY_URL", value);
    }
    if let Some(value) = ai_config.ollama_url.as_ref() {
        cmd.env("YOUWEE_AI_OLLAMA_URL", value);
    }
    if let Some(value) = ai_config.lmstudio_url.as_ref() {
        cmd.env("YOUWEE_AI_LMSTUDIO_URL", value);
    }
    if let Some(value) = ai_config.timeout_seconds {
        cmd.env("YOUWEE_AI_TIMEOUT_SECONDS", value.to_string());
    }
    cmd.env(
        "YOUWEE_AI_SUMMARY_STYLE",
        serde_json::to_value(&ai_config.summary_style)
            .ok()
            .and_then(|value| value.as_str().map(|value| value.to_string()))
            .unwrap_or_else(|| "concise".to_string()),
    );
    cmd.env("YOUWEE_AI_SUMMARY_LANGUAGE", &ai_config.summary_language);
    cmd.env(
        "YOUWEE_AI_WHISPER_ENABLED",
        if ai_config.whisper_enabled { "true" } else { "false" },
    );
    if let Some(value) = ai_config.whisper_api_key.as_ref() {
        cmd.env("YOUWEE_AI_WHISPER_API_KEY", value);
    }
    if let Some(value) = ai_config.whisper_endpoint_url.as_ref() {
        cmd.env("YOUWEE_AI_WHISPER_ENDPOINT_URL", value);
    }
    if let Some(value) = ai_config.whisper_model.as_ref() {
        cmd.env("YOUWEE_AI_WHISPER_MODEL", value);
    }
    let vendor_root = plugin_dir.join("vendor");
    if vendor_root.is_dir()
        && matches!(selected_provider, PluginProvider::Node | PluginProvider::Bun)
    {
        cmd.env("NODE_PATH", vendor_root.to_string_lossy().to_string());
    }
    for (key, value) in env_values {
        cmd.env(key, value);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start plugin {}: {}", plugin.manifest.plugin_id, e))?;
    let mut stdin = child.stdin.take().ok_or_else(|| "Failed to open plugin stdin".to_string())?;
    stdin
        .write_all(&payload_json)
        .await
        .map_err(|e| format!("Failed to write plugin payload: {}", e))?;
    stdin
        .shutdown()
        .await
        .map_err(|e| format!("Failed to close plugin stdin: {}", e))?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open plugin stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to open plugin stderr".to_string())?;

    let plugin_id = plugin.manifest.plugin_id.clone();
    let plugin_name = plugin.manifest.name.clone();
    let log_url = payload.url.clone();
    let media_title = payload.title.clone();
    let filename = Some(payload.filename.clone());
    let log_run_id = Some(run_id.to_string());
    let stdout_task = tokio::spawn(capture_process_stream(
        app.clone(),
        "stdout",
        plugin_id.clone(),
        plugin_name.clone(),
        log_run_id.clone(),
        stdout,
        Some(log_url.clone()),
        media_title.clone(),
        filename.clone(),
    ));
    let stderr_task = tokio::spawn(capture_process_stream_err(
        app.clone(),
        "stderr",
        plugin_id.clone(),
        plugin_name.clone(),
        log_run_id,
        stderr,
        Some(log_url),
        media_title.clone(),
        filename.clone(),
    ));

    let status = match tokio::time::timeout(
        std::time::Duration::from_secs(plugin.manifest.timeout_sec.max(1)),
        child.wait(),
    )
    .await
    {
        Ok(result) => result.map_err(|e| format!("Failed waiting for plugin process: {}", e))?,
        Err(_) => {
            if let Err(error) = child.kill().await {
                add_log_internal(
                    "warn",
                    "Failed to stop plugin process after timeout",
                    Some(&format!("pluginId: {}; provider: {} - {}", plugin_id, selected_provider.as_str(), error)),
                    Some(&payload.url),
                )
                .ok();
            }
            let stdout = output_to_string(&stdout_task.await.unwrap_or_default());
            let stderr = output_to_string(&stderr_task.await.unwrap_or_default());

            let mut message = format!("Plugin timed out after {}s", plugin.manifest.timeout_sec);
            message.push_str(&format!(
                "\nProvider: {}\nResolved source: {}",
                selected_provider.as_str(),
                resolved_source.as_deref().unwrap_or("unknown")
            ));
            if !stderr.is_empty() {
                message.push_str(&format!("\n\nstderr:\n{}", stderr));
            }
            if !stdout.is_empty() {
                message.push_str(&format!("\n\nstdout:\n{}", stdout));
            }
            return Err(message);
        }
    };

    let stdout = output_to_string(&stdout_task.await.unwrap_or_default());
    let stderr = output_to_string(&stderr_task.await.unwrap_or_default());

    if !status.success() {
        let details = plugin_output_details(&stdout, &stderr);
        return Err(format!("Plugin exited with {}.\n{}", plugin_exit_reason(&status), details));
    }

    let parsed_output = parse_plugin_result(&stdout);

    Ok((
        PluginExecutionResult {
            plugin_id: plugin.manifest.plugin_id.clone(),
            success: parsed_output
                .as_ref()
                .and_then(|value| value.success)
                .unwrap_or(true),
            message: parsed_output.as_ref().and_then(|value| value.message.clone()),
            artifacts: parsed_output.as_ref().and_then(|value| value.artifacts.clone()),
            metadata: parsed_output.as_ref().and_then(|value| value.metadata.clone()),
            mutations: parsed_output.as_ref().and_then(|value| value.mutations.clone()),
            stdout: if stdout.is_empty() { None } else { Some(stdout) },
            stderr: if stderr.is_empty() { None } else { Some(stderr) },
        },
        selected_provider,
        resolved_source,
    ))
}

fn workflow_result_status(
    results: &[PluginExecutionResult],
    stopped_early: bool,
) -> PluginWorkflowRunStatus {
    if results.is_empty() {
        return PluginWorkflowRunStatus::Completed;
    }
    let success_count = results.iter().filter(|result| result.success).count();
    if success_count == results.len() {
        PluginWorkflowRunStatus::Completed
    } else if success_count == 0 || stopped_early {
        PluginWorkflowRunStatus::Failed
    } else {
        PluginWorkflowRunStatus::PartialFailed
    }
}

async fn execute_plugin_workflow_run(
    app: AppHandle,
    workflow_run: PluginWorkflowRun,
) -> Vec<PluginExecutionResult> {
    let _queue_guard = workflow_queue_lock().lock().await;

    add_log_internal(
        "info",
        "Post-processing workflow started",
        Some(&format!(
            "Workflow Run ID: {}\nTrigger: {}\nSteps: {}",
            workflow_run.run_id,
            workflow_run.trigger,
            workflow_run.steps.len()
        )),
        Some(&workflow_run.initial_payload.url),
    )
    .ok();

    let plugins = match list_plugins_internal(&app) {
        Ok(plugins) => plugins,
        Err(error) => {
            add_log_internal(
                "error",
                "Failed to load plugins for workflow execution",
                Some(&error),
                Some(&workflow_run.initial_payload.url),
            )
            .ok();
            return Vec::new();
        }
    };
    let plugins_by_id: BTreeMap<String, PluginSummary> = plugins
        .into_iter()
        .map(|plugin| (plugin.manifest.plugin_id.clone(), plugin))
        .collect();

    let mut chain_state = workflow_run.current_chain_state.clone();
    let mut registry = read_registry(&app).unwrap_or_default();
    let mut results = Vec::new();
    let mut stopped_early = false;

    for (step_index, step) in workflow_run.steps.iter().enumerate() {
        let Some(base_plugin) = plugins_by_id.get(&step.plugin_id) else {
            let message = format!(
                "Workflow step plugin not found: {}",
                step.plugin_id
            );
            add_log_internal(
                "error",
                "Post-processing step missing",
                Some(&format!(
                    "Workflow Run ID: {}\nStep: {}\n{}",
                    workflow_run.run_id,
                    step_index + 1,
                    message
                )),
                Some(&chain_state.url),
            )
            .ok();
            results.push(PluginExecutionResult {
                plugin_id: step.plugin_id.clone(),
                success: false,
                message: Some(message),
                artifacts: None,
                metadata: None,
                mutations: None,
                stdout: None,
                stderr: None,
            });
            if step.failure_policy == PluginWorkflowFailurePolicy::StopChain {
                stopped_early = true;
                break;
            }
            continue;
        };

        let mut plugin = base_plugin.clone();
        plugin.installation.enabled = true;
        plugin.installation.selected_provider = step.selected_provider.clone();
        plugin.installation.approved_permissions = step.approved_permissions.clone();

        let selected_provider = plugin
            .installation
            .selected_provider
            .clone()
            .or(plugin.manifest.runtime.preferred_provider.clone())
            .unwrap_or_else(|| default_provider_for_language(&plugin.manifest.runtime.language));
        let step_payload = payload_from_chain_state(
            &workflow_run.initial_payload,
            &workflow_run.run_id,
            step_index,
            &plugin.manifest.plugin_id,
            &chain_state,
        );

        add_log_internal(
            "info",
            &format!("Running post-processing step: {}", plugin.manifest.name),
            Some(&format!(
                "Workflow Run ID: {}\nStep: {} / {}\nPolicy: {}",
                workflow_run.run_id,
                step_index + 1,
                workflow_run.steps.len(),
                match step.failure_policy {
                    PluginWorkflowFailurePolicy::Continue => "continue",
                    PluginWorkflowFailurePolicy::StopChain => "stop-chain",
                }
            )),
            Some(&step_payload.url),
        )
        .ok();

        if let Some(entry) = registry.installations.get_mut(&plugin.manifest.plugin_id) {
            entry.last_execution_status = Some("running".to_string());
            entry.last_error = None;
        }
        app.emit(
            "plugin-execution-status",
            PluginExecutionStatusEvent {
                plugin_id: plugin.manifest.plugin_id.clone(),
                run_id: Some(workflow_run.run_id.clone()),
                plugin_name: Some(plugin.manifest.name.clone()),
                runtime: Some(plugin.manifest.runtime.language.as_str().to_string()),
                provider: Some(selected_provider.as_str().to_string()),
                resolved_provider: None,
                resolved_source: None,
                status: "running".to_string(),
                message: Some(format!("Running {}", plugin.manifest.name)),
                details: Some(format!(
                    "Runtime: {}\nTimeout: {}s\nStep: {} / {}",
                    plugin.manifest.runtime.language.as_str(),
                    plugin.manifest.timeout_sec,
                    step_index + 1,
                    workflow_run.steps.len()
                )),
                media_title: step_payload.title.clone(),
                filename: Some(step_payload.filename.clone()),
                media_url: Some(step_payload.url.clone()),
            },
        )
        .ok();
        write_registry(&app, &registry).ok();

        match execute_plugin(&app, &plugin, &workflow_run.run_id, &step_payload).await {
            Ok((result, resolved_provider, resolved_source)) => {
                if let Some(entry) = registry.installations.get_mut(&plugin.manifest.plugin_id) {
                    entry.last_resolved_provider = Some(resolved_provider.clone());
                    entry.last_resolved_source = resolved_source.clone();
                    entry.last_execution_status = Some(if result.success {
                        "success".to_string()
                    } else {
                        "error".to_string()
                    });
                    entry.last_error = if result.success {
                        None
                    } else {
                        result.message.clone()
                    };
                }

                if let Some(mutation) = result.mutations.as_ref() {
                    merge_chain_mutation(&mut chain_state, mutation);
                }

                app.emit(
                    "plugin-execution-status",
                    PluginExecutionStatusEvent {
                        plugin_id: plugin.manifest.plugin_id.clone(),
                        run_id: Some(workflow_run.run_id.clone()),
                        plugin_name: Some(plugin.manifest.name.clone()),
                        runtime: Some(plugin.manifest.runtime.language.as_str().to_string()),
                        provider: Some(selected_provider.as_str().to_string()),
                        resolved_provider: Some(resolved_provider.as_str().to_string()),
                        resolved_source: resolved_source.clone(),
                        status: if result.success {
                            "success".to_string()
                        } else {
                            "error".to_string()
                        },
                        message: result.message.clone(),
                        details: shorten_for_event(combine_plugin_event_details(
                            result.message.as_ref(),
                            result.stdout.as_ref(),
                            result.stderr.as_ref(),
                        )),
                        media_title: step_payload.title.clone(),
                        filename: Some(step_payload.filename.clone()),
                        media_url: Some(step_payload.url.clone()),
                    },
                )
                .ok();

                let details = combine_plugin_event_details(
                    result.message.as_ref(),
                    result.stdout.as_ref(),
                    result.stderr.as_ref(),
                );
                add_log_internal(
                    if result.success { "success" } else { "error" },
                    &format!("Post-processing step finished: {}", plugin.manifest.name),
                    Some(&format!(
                        "Workflow Run ID: {}\nStep: {} / {}\n{}",
                        workflow_run.run_id,
                        step_index + 1,
                        workflow_run.steps.len(),
                        details.as_deref().unwrap_or("")
                    )),
                    Some(&step_payload.url),
                )
                .ok();

                let should_stop = !result.success
                    && step.failure_policy == PluginWorkflowFailurePolicy::StopChain;
                results.push(result);
                if should_stop {
                    stopped_early = true;
                    break;
                }
            }
            Err(error) => {
                if let Some(entry) = registry.installations.get_mut(&plugin.manifest.plugin_id) {
                    entry.last_execution_status = Some("error".to_string());
                    entry.last_error = Some(error.clone());
                }
                app.emit(
                    "plugin-execution-status",
                    PluginExecutionStatusEvent {
                        plugin_id: plugin.manifest.plugin_id.clone(),
                        run_id: Some(workflow_run.run_id.clone()),
                        plugin_name: Some(plugin.manifest.name.clone()),
                        runtime: Some(plugin.manifest.runtime.language.as_str().to_string()),
                        provider: Some(selected_provider.as_str().to_string()),
                        resolved_provider: Some(selected_provider.as_str().to_string()),
                        resolved_source: None,
                        status: "error".to_string(),
                        message: Some(error.clone()),
                        details: shorten_for_event(Some(error.clone())),
                        media_title: step_payload.title.clone(),
                        filename: Some(step_payload.filename.clone()),
                        media_url: Some(step_payload.url.clone()),
                    },
                )
                .ok();
                add_log_internal(
                    "error",
                    &format!("Post-processing step failed: {}", plugin.manifest.name),
                    Some(&format!(
                        "Workflow Run ID: {}\nStep: {} / {}\n{}",
                        workflow_run.run_id,
                        step_index + 1,
                        workflow_run.steps.len(),
                        error
                    )),
                    Some(&step_payload.url),
                )
                .ok();

                results.push(PluginExecutionResult {
                    plugin_id: plugin.manifest.plugin_id.clone(),
                    success: false,
                    message: Some(error),
                    artifacts: None,
                    metadata: None,
                    mutations: None,
                    stdout: None,
                    stderr: None,
                });
                if step.failure_policy == PluginWorkflowFailurePolicy::StopChain {
                    stopped_early = true;
                    break;
                }
            }
        }
    }

    write_registry(&app, &registry).ok();

    let final_status = workflow_result_status(&results, stopped_early);
    add_log_internal(
        match final_status {
            PluginWorkflowRunStatus::Completed => "success",
            PluginWorkflowRunStatus::PartialFailed => "error",
            PluginWorkflowRunStatus::Failed => "error",
            PluginWorkflowRunStatus::Queued | PluginWorkflowRunStatus::Running => "info",
        },
        "Post-processing workflow finished",
        Some(&format!(
            "Workflow Run ID: {}\nStatus: {:?}\nSteps run: {}",
            workflow_run.run_id,
            final_status,
            results.len()
        )),
        Some(&workflow_run.initial_payload.url),
    )
    .ok();

    results
}

pub fn enqueue_post_download_workflow(
    app: &AppHandle,
    workflow_steps: Vec<PluginWorkflowStepSnapshot>,
    payload: PostDownloadPluginPayload,
) -> Option<String> {
    if workflow_steps.is_empty() {
        return None;
    }

    let run_id = Uuid::new_v4().to_string();
    let workflow_run = PluginWorkflowRun {
        run_id: run_id.clone(),
        trigger: payload.trigger.clone(),
        status: PluginWorkflowRunStatus::Queued,
        current_chain_state: build_chain_state(&payload),
        initial_payload: payload,
        steps: workflow_steps,
        current_step_index: None,
        failed_step_plugin_id: None,
    };
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = execute_plugin_workflow_run(app_handle, workflow_run).await;
    });
    Some(run_id)
}

pub fn enqueue_plugin_trigger_workflow(
    app: &AppHandle,
    trigger: &str,
    workflow_steps: Option<Vec<PluginWorkflowStepSnapshot>>,
    mut payload: PostDownloadPluginPayload,
) -> Option<String> {
    payload.trigger = trigger.to_string();
    let steps = resolve_download_workflow_snapshot(app, trigger, workflow_steps, &[]);
    enqueue_post_download_workflow(app, steps, payload)
}

pub fn resolve_download_workflow_snapshot(
    app: &AppHandle,
    trigger: &str,
    workflow_steps: Option<Vec<PluginWorkflowStepSnapshot>>,
    legacy_plugin_ids: &[String],
) -> Vec<PluginWorkflowStepSnapshot> {
    if let Some(steps) = workflow_steps {
        if !steps.is_empty() {
            return steps;
        }
    }

    let registry = match read_registry(app) {
        Ok(registry) => registry,
        Err(error) => {
            add_log_internal(
                "error",
                "Failed to read plugin workflow registry",
                Some(&error),
                None,
            )
            .ok();
            return Vec::new();
        }
    };
    let plugins = match list_plugins_internal(app) {
        Ok(plugins) => plugins,
        Err(error) => {
            add_log_internal(
                "error",
                "Failed to load plugins for workflow snapshot",
                Some(&error),
                None,
            )
            .ok();
            return Vec::new();
        }
    };
    let plugins_by_id: BTreeMap<String, PluginSummary> = plugins
        .into_iter()
        .map(|plugin| (plugin.manifest.plugin_id.clone(), plugin))
        .collect();

    if !legacy_plugin_ids.is_empty() {
        return build_legacy_workflow_snapshots(&plugins_by_id, &registry, trigger, legacy_plugin_ids);
    }

    let workflow = PluginTriggerWorkflow {
        trigger: trigger.to_string(),
        steps: workflow_registry_for_trigger(&registry, trigger)
            .map(|workflow| workflow.steps.clone())
            .unwrap_or_default(),
    };
    resolve_workflow_step_snapshots(&plugins_by_id, &workflow)
}

fn build_scaffold_plugin_module() -> String {
    r#"const { definePlugin, triggers } = require("youwee-sdk");

module.exports = definePlugin({
  meta: {
    name: "Replace this name",
    version: "0.1.0",
    description: "Describe what this plugin does.",
  },

  hooks: {
    [triggers.downloadCompleted]: async (ctx) => {
      ctx.log.info("Hook started", {
        filename: ctx.file.name,
        trigger: ctx.trigger,
        ffmpegAvailable: ctx.youwee.tools.ffmpeg.available,
      });

      // Start editing here:
      // 1. Read the downloaded file info from ctx.file
      // 2. Read extra metadata from ctx.media or ctx.download
      // 3. Read secrets from ctx.env.require("YOUR_ENV_NAME")
      // 4. Use app capabilities from ctx.youwee.tools / ctx.youwee.ai
      // 5. Return ctx.ok(...) or ctx.fail(...)

      return ctx.ok("Plugin scaffold ran successfully.", {
        filepath: ctx.file.path,
        filename: ctx.file.name,
        trigger: ctx.trigger,
      });
    },
  },
});
"#
    .to_string()
}

fn build_scaffold_package_json(manifest: &PluginManifest) -> String {
    format!(
        r#"{{
  "name": "{slug}",
  "version": "{version}",
  "private": true,
  "description": "{description}",
  "type": "commonjs",
  "main": "src/plugin.js",
  "scripts": {{
    "test:node": "NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js node vendor/youwee-sdk/dist/runtime-cli.js",
    "test:bun": "NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js bun vendor/youwee-sdk/dist/runtime-cli.js"
  }},
  "dependencies": {{
    "youwee-sdk": "file:vendor/youwee-sdk"
  }}
}}
"#,
        slug = manifest.slug,
        version = manifest.version,
        description = manifest
            .description
            .as_deref()
            .unwrap_or("Youwee plugin scaffold")
            .replace('"', "\\\"")
    )
}

fn write_scaffold_sdk_package(destination: &Path) -> Result<(), String> {
    let vendor_root = destination.join("vendor").join("youwee-sdk");
    std::fs::create_dir_all(vendor_root.join("dist")).map_err(|e| {
        format!(
            "Failed to create scaffold SDK dist directory {}: {}",
            vendor_root.join("dist").display(),
            e
        )
    })?;
    let files = [
        ("package.json", SDK_JS_PACKAGE_JSON),
        ("dist/index.js", SDK_JS_INDEX),
        ("dist/runtime.js", SDK_JS_RUNTIME),
        ("dist/runtime-cli.js", SDK_JS_RUNTIME_CLI),
        ("dist/ai.js", SDK_JS_AI),
        ("dist/compatibility.js", SDK_JS_COMPATIBILITY),
        ("dist/schema.js", SDK_JS_SCHEMA),
        ("dist/types.js", SDK_JS_SHARED_RUNTIME_TYPES),
        ("dist/manifest.js", SDK_JS_MANIFEST),
        ("dist/index.d.ts", SDK_JS_TYPES),
        ("dist/runtime.d.ts", SDK_JS_RUNTIME_TYPES),
        ("dist/runtime-cli.d.ts", SDK_JS_RUNTIME_CLI_TYPES),
        ("dist/ai.d.ts", SDK_JS_AI_TYPES),
        ("dist/compatibility.d.ts", SDK_JS_COMPATIBILITY_TYPES),
        ("dist/schema.d.ts", SDK_JS_SCHEMA_TYPES),
        ("dist/manifest.d.ts", SDK_JS_MANIFEST_TYPES),
        ("dist/types.d.ts", SDK_JS_SHARED_TYPES),
        ("README.md", SDK_JS_README),
    ];

    for (relative_path, content) in files {
        let path = vendor_root.join(relative_path);
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write scaffold SDK file {}: {}", path.display(), e))?;
    }

    Ok(())
}

fn build_scaffold_readme(manifest: &PluginManifest) -> String {
    format!(
        r#"# {name}

## Overview

This plugin scaffold targets the Youwee JavaScript plugin runtime.

Identity:
- `pluginId`: `{plugin_id}`
- `slug`: `{slug}`
- `language`: `{language}`
- `supportedProviders`: `{providers}`
- `preferredProvider`: `{preferred}`
- `compatibility.appVersion`: use this to declare the minimum compatible Youwee app range
- `compatibility.sdkVersion`: use this to declare the minimum compatible SDK range

Package layout:
- `plugin.json`: plugin manifest consumed by Youwee
- `package.json`: package metadata and local test scripts
- `src/plugin.js`: plugin module and hook implementations
- `vendor/youwee-sdk/`: vendored SDK runtime and type declarations
- `examples/`: sample payload and result files

## Entry module

The plugin entrypoint is `src/plugin.js`.

You do not need a per-plugin runner file. Youwee launches the shared bootstrap from
`youwee-sdk` and passes your plugin entry module through the runtime bridge.

## Trigger naming

Use raw runtime trigger strings in `plugin.json`:

```json
{{
  "triggers": ["download.completed", "download.failed"]
}}
```

Use SDK identifiers only inside `src/plugin.js`:

```js
hooks: {{
  [triggers.downloadCompleted]: async (ctx) => {{
    return ctx.ok("Done");
  }},
}}
```

Do not write values like `"triggers.downloadCompleted"` in `plugin.json`.

## Execution model

Execution flow:
1. Youwee dispatches a trigger such as `download.completed`
2. The shared SDK bootstrap loads `src/plugin.js`
3. The SDK reads the payload JSON from `stdin`
4. The SDK creates `ctx`
5. The matching hook runs
6. The hook returns `ctx.ok(...)` or `ctx.fail(...)`
7. The SDK writes the final JSON result to `stdout`

## Hook implementation

Implement hooks in `src/plugin.js`:

```js
hooks: {{
  [triggers.downloadCompleted]: async (ctx) => {{
    return ctx.ok("Done");
  }},
}}
```

Available high-level APIs:
- `ctx.trigger`
- `ctx.download`
- `ctx.file`
- `ctx.media`
- `ctx.env.get(...)`
- `ctx.env.require(...)`
- `ctx.log.info(...)`
- `ctx.youwee.runtime`
- `ctx.youwee.app.version`
- `ctx.youwee.sdk.assertAppVersion(...)`
- `ctx.youwee.tools.ffmpeg`
- `ctx.youwee.tools.ytdlp`
- `ctx.youwee.fs.readText(...)`
- `ctx.youwee.http.getJson(...)`
- `ctx.youwee.ai.generateText(...)`
- `ctx.youwee.ai.summarize(...)`
- `ctx.youwee.ai.extractJson(...)`
- `ctx.ok(...)`
- `ctx.fail(...)`

Reference payload: `examples/payload.download.completed.json`

## Result contract

Return a JSON-serializable result:

```json
{{
  "success": true,
  "message": "Human readable summary",
  "artifacts": null,
  "metadata": {{}}
}}
```

Examples:

```js
return ctx.ok("Uploaded successfully", {{ driveFileId: "abc123" }});
return ctx.fail("Missing API token");
```

## Logging contract

Use:
- `ctx.log.debug(message, metadata?)`
- `ctx.log.info(message, metadata?)`
- `ctx.log.warn(message, metadata?)`
- `ctx.log.error(message, metadata?)`

Runtime logs are written to `stderr`.
The final structured result must remain on `stdout`.

## Runtime notes

This scaffold is optimized for:
- Node
- Bun

If your implementation depends on runtime-specific APIs, update
`runtime.supportedProviders` in `plugin.json`.

## Local execution

Node:

```bash
cat examples/payload.download.completed.json | NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js node vendor/youwee-sdk/dist/runtime-cli.js
```

Bun:

```bash
cat examples/payload.download.completed.json | NODE_PATH=vendor YOUWEE_PLUGIN_MAIN=src/plugin.js bun vendor/youwee-sdk/dist/runtime-cli.js
```

## Packaging

To share this plugin:
1. Keep `plugin.json` at the package root
2. Include `src/`, `vendor/youwee-sdk/`, `package.json`, `README.md`, and `CHANGELOG.md`
3. Zip the plugin root directory
4. Import it into Youwee from a folder, ZIP, or URL

## Next step

Edit `src/plugin.js` first and replace the example hook body with your actual logic.
"#,
        name = manifest.name,
        plugin_id = manifest.plugin_id,
        slug = manifest.slug,
        language = manifest.runtime.language.as_str(),
        providers = manifest
            .runtime
            .supported_providers
            .iter()
            .map(PluginProvider::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        preferred = manifest
            .runtime
            .preferred_provider
            .as_ref()
            .map(PluginProvider::as_str)
            .unwrap_or("none")
    )
}

fn build_scaffold_changelog() -> String {
    "# Changelog\n\n## [0.1.0]\n- Initial scaffold\n".to_string()
}

fn build_scaffold_success_result_example() -> String {
    r#"{
  "success": true,
  "message": "Uploaded successfully",
  "artifacts": null,
  "metadata": {
    "example": true
  }
}
"#
    .to_string()
}

fn build_scaffold_failure_result_example() -> String {
    r#"{
  "success": false,
  "message": "Missing configuration",
  "artifacts": null,
  "metadata": {
    "reason": "GOOGLE_DRIVE_ACCESS_TOKEN is missing"
  }
}
"#
    .to_string()
}

fn sample_download_payload() -> PostDownloadPluginPayload {
    PostDownloadPluginPayload {
        job_id: "sample-job".to_string(),
        source: Some("youtube".to_string()),
        trigger: "download.completed".to_string(),
        filepath: "/tmp/sample.mp4".to_string(),
        filename: "sample.mp4".to_string(),
        directory: "/tmp".to_string(),
        filesize: Some(12345678),
        format: Some("mp4".to_string()),
        quality: Some("1080p".to_string()),
        url: "https://example.com/video".to_string(),
        title: Some("Sample video".to_string()),
        thumbnail: Some("https://example.com/thumb.jpg".to_string()),
        history_id: Some("sample-history-id".to_string()),
        time_range: None,
        download_kind: "download".to_string(),
        workflow_run_id: None,
        workflow_step_index: None,
        workflow_step_plugin_id: None,
        chain_state: None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        build_scaffold_package_json, build_scaffold_readme, collect_compatibility_issues,
        parse_plugin_result, satisfies_version_range, sanitize_slug, validate_manifest,
        write_scaffold_sdk_package,
    };
    use crate::types::{PluginPermissionRequest, PluginProvider, PluginRuntimeLanguage, PluginRuntimeSpec};

    #[test]
    fn sanitize_slug_normalizes_values() {
        assert_eq!(sanitize_slug(" Google Drive Upload "), "google-drive-upload");
    }

    #[test]
    fn scaffold_readme_mentions_framework_entrypoint() {
        let manifest = crate::types::PluginManifest {
            plugin_id: "id".to_string(),
            slug: "slug".to_string(),
            name: "Name".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            runtime: PluginRuntimeSpec {
                language: PluginRuntimeLanguage::Javascript,
                supported_providers: vec![PluginProvider::Node, PluginProvider::Bun],
                preferred_provider: Some(PluginProvider::Node),
                entrypoint: "src/plugin.js".to_string(),
            },
            compatibility: None,
            triggers: vec!["download.completed".to_string()],
            permissions: PluginPermissionRequest::default(),
            timeout_sec: 60,
            readme: None,
            checksum: None,
            published_at: None,
        };
        let readme = build_scaffold_readme(&manifest);
        assert!(readme.contains("src/plugin.js"));
        assert!(readme.contains("ctx.ok"));
        assert!(readme.contains("Execution flow"));
        assert!(readme.contains("vendor/youwee-sdk"));
    }

    #[test]
    fn validate_manifest_rejects_empty_supported_providers() {
        let manifest = crate::types::PluginManifest {
            plugin_id: "id".to_string(),
            slug: "slug".to_string(),
            name: "Name".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            runtime: PluginRuntimeSpec {
                language: PluginRuntimeLanguage::Javascript,
                supported_providers: Vec::new(),
                preferred_provider: None,
                entrypoint: "index.ts".to_string(),
            },
            compatibility: None,
            triggers: vec!["download.completed".to_string()],
            permissions: PluginPermissionRequest::default(),
            timeout_sec: 60,
            readme: None,
            checksum: None,
            published_at: None,
        };
        let err = validate_manifest(&manifest, Path::new("/tmp/plugin.json")).unwrap_err();
        assert!(err.contains("supportedProviders"));
    }

    #[test]
    fn validate_manifest_rejects_sdk_trigger_identifiers() {
        let manifest = crate::types::PluginManifest {
            plugin_id: "id".to_string(),
            slug: "slug".to_string(),
            name: "Name".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            runtime: PluginRuntimeSpec {
                language: PluginRuntimeLanguage::Javascript,
                supported_providers: vec![PluginProvider::Node],
                preferred_provider: Some(PluginProvider::Node),
                entrypoint: "src/plugin.js".to_string(),
            },
            compatibility: None,
            triggers: vec!["triggers.downloadQueued".to_string()],
            permissions: PluginPermissionRequest::default(),
            timeout_sec: 60,
            readme: None,
            checksum: None,
            published_at: None,
        };
        let err = validate_manifest(&manifest, Path::new("/tmp/plugin.json")).unwrap_err();
        assert!(err.contains("raw runtime names"));
    }

    #[test]
    fn scaffold_readme_mentions_runtime_contract() {
        let manifest = crate::types::PluginManifest {
            plugin_id: "id".to_string(),
            slug: "slug".to_string(),
            name: "Name".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            runtime: PluginRuntimeSpec {
                language: PluginRuntimeLanguage::Javascript,
                supported_providers: vec![PluginProvider::Deno],
                preferred_provider: Some(PluginProvider::Deno),
                entrypoint: "index.ts".to_string(),
            },
            compatibility: None,
            triggers: vec!["download.completed".to_string()],
            permissions: PluginPermissionRequest::default(),
            timeout_sec: 60,
            readme: Some("README.md".to_string()),
            checksum: None,
            published_at: None,
        };
        let readme = build_scaffold_readme(&manifest);
        assert!(readme.contains("supportedProviders"));
        assert!(readme.contains("ctx.youwee.ai"));
    }

    #[test]
    fn scaffold_package_json_uses_vendored_sdk_dependency() {
        let manifest = crate::types::PluginManifest {
            plugin_id: "id".to_string(),
            slug: "gg-drive".to_string(),
            name: "GG Drive".to_string(),
            version: "0.1.0".to_string(),
            description: Some("Upload files to Drive".to_string()),
            author: None,
            homepage: None,
            repository: None,
            license: None,
            runtime: PluginRuntimeSpec {
                language: PluginRuntimeLanguage::Javascript,
                supported_providers: vec![PluginProvider::Node, PluginProvider::Bun],
                preferred_provider: Some(PluginProvider::Node),
                entrypoint: "src/plugin.js".to_string(),
            },
            compatibility: None,
            triggers: vec!["download.completed".to_string()],
            permissions: PluginPermissionRequest::default(),
            timeout_sec: 60,
            readme: Some("README.md".to_string()),
            checksum: None,
            published_at: None,
        };
        let package_json = build_scaffold_package_json(&manifest);
        assert!(package_json.contains("\"youwee-sdk\": \"file:vendor/youwee-sdk\""));
        assert!(package_json.contains("YOUWEE_PLUGIN_MAIN=src/plugin.js"));
        assert!(package_json.contains("vendor/youwee-sdk/dist/runtime-cli.js"));
    }

    #[test]
    fn parse_plugin_result_accepts_json_on_the_last_stdout_line() {
        let stdout = "plain text before result\n{\"success\":true,\"message\":\"Uploaded\"}\n";
        let parsed = parse_plugin_result(stdout).expect("expected plugin result");
        assert_eq!(parsed.success, Some(true));
        assert_eq!(parsed.message.as_deref(), Some("Uploaded"));
    }

    #[test]
    fn version_ranges_are_checked_correctly() {
        assert!(satisfies_version_range("0.13.3", ">=0.13.0 <0.14.0").unwrap());
        assert!(!satisfies_version_range("0.14.0", ">=0.13.0 <0.14.0").unwrap());
        assert!(satisfies_version_range("0.13.3", "=0.13.3").unwrap());
    }

    #[test]
    fn compatibility_issues_are_reported_for_mismatched_ranges() {
        let manifest = crate::types::PluginManifest {
            plugin_id: "id".to_string(),
            slug: "slug".to_string(),
            name: "Name".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            runtime: PluginRuntimeSpec {
                language: PluginRuntimeLanguage::Javascript,
                supported_providers: vec![PluginProvider::Node],
                preferred_provider: Some(PluginProvider::Node),
                entrypoint: "src/plugin.js".to_string(),
            },
            compatibility: Some(crate::types::PluginCompatibilitySpec {
                app_version: Some(">=999.0.0 <1000.0.0".to_string()),
                sdk_version: Some(">=999.0.0 <1000.0.0".to_string()),
            }),
            triggers: vec!["download.completed".to_string()],
            permissions: PluginPermissionRequest::default(),
            timeout_sec: 60,
            readme: None,
            checksum: None,
            published_at: None,
        };

        let issues = collect_compatibility_issues(&manifest).unwrap();
        assert_eq!(issues.len(), 2);
        assert!(issues[0].contains("Requires Youwee app version"));
        assert!(issues[1].contains("Requires youwee-sdk version"));
    }

    #[test]
    fn scaffold_sdk_bundle_includes_all_runtime_modules() {
        let temp_dir = std::env::temp_dir().join(format!("youwee-sdk-bundle-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        write_scaffold_sdk_package(&temp_dir).unwrap();

        for relative_path in [
            "vendor/youwee-sdk/dist/index.js",
            "vendor/youwee-sdk/dist/runtime.js",
            "vendor/youwee-sdk/dist/runtime-cli.js",
            "vendor/youwee-sdk/dist/ai.js",
            "vendor/youwee-sdk/dist/compatibility.js",
            "vendor/youwee-sdk/dist/schema.js",
            "vendor/youwee-sdk/dist/manifest.js",
            "vendor/youwee-sdk/dist/types.js",
        ] {
            assert!(temp_dir.join(relative_path).exists(), "missing {relative_path}");
        }

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
