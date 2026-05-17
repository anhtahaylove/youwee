use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntimeLanguage {
    Javascript,
    Python,
}

impl PluginRuntimeLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Javascript => "javascript",
            Self::Python => "python",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PluginProvider {
    Deno,
    Node,
    Bun,
    Python,
}

impl PluginProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deno => "deno",
            Self::Node => "node",
            Self::Bun => "bun",
            Self::Python => "python",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissionRequest {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub read_paths: Vec<String>,
    #[serde(default)]
    pub write_paths: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissionApproval {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub read_paths: bool,
    #[serde(default)]
    pub write_paths: bool,
    #[serde(default)]
    pub env: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeSpec {
    pub language: PluginRuntimeLanguage,
    pub supported_providers: Vec<PluginProvider>,
    #[serde(default)]
    pub preferred_provider: Option<PluginProvider>,
    pub entrypoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginCompatibilitySpec {
    #[serde(default)]
    pub app_version: Option<String>,
    #[serde(default)]
    pub sdk_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub plugin_id: String,
    pub slug: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    pub runtime: PluginRuntimeSpec,
    #[serde(default)]
    pub compatibility: Option<PluginCompatibilitySpec>,
    #[serde(default = "default_triggers")]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub permissions: PluginPermissionRequest,
    #[serde(default = "default_timeout_sec")]
    pub timeout_sec: u64,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
}

fn default_triggers() -> Vec<String> {
    vec!["download.completed".to_string()]
}

const fn default_timeout_sec() -> u64 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PluginPackageSourceKind {
    AppScaffold,
    LocalFolder,
    LocalZip,
    RemoteUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPackageSource {
    pub kind: PluginPackageSourceKind,
    pub value: String,
    #[serde(default)]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallation {
    pub plugin_id: String,
    pub enabled: bool,
    pub trusted: bool,
    pub approved_permissions: PluginPermissionApproval,
    #[serde(default)]
    pub selected_provider: Option<PluginProvider>,
    pub installed_path: String,
    pub source: PluginPackageSource,
    #[serde(default)]
    pub last_resolved_provider: Option<PluginProvider>,
    #[serde(default)]
    pub last_resolved_source: Option<String>,
    #[serde(default)]
    pub last_execution_status: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub env_value_status: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    pub manifest: PluginManifest,
    pub installation: PluginInstallation,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPackageInspection {
    pub manifest: PluginManifest,
    pub source: PluginPackageSource,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProviderStatus {
    pub provider: PluginProvider,
    pub available: bool,
    #[serde(default)]
    pub resolved_path: Option<String>,
    #[serde(default)]
    pub resolved_source: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExecutionResult {
    pub plugin_id: String,
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub artifacts: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub mutations: Option<PluginChainMutation>,
    #[serde(default)]
    pub stdout: Option<String>,
    #[serde(default)]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PluginWorkflowFailurePolicy {
    Continue,
    StopChain,
}

impl Default for PluginWorkflowFailurePolicy {
    fn default() -> Self {
        Self::Continue
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginWorkflowStepConfig {
    pub plugin_id: String,
    #[serde(default)]
    pub failure_policy: PluginWorkflowFailurePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginWorkflowStepSnapshot {
    pub plugin_id: String,
    pub plugin_name: String,
    pub plugin_version: String,
    #[serde(default)]
    pub selected_provider: Option<PluginProvider>,
    #[serde(default)]
    pub approved_permissions: PluginPermissionApproval,
    #[serde(default)]
    pub failure_policy: PluginWorkflowFailurePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginTriggerWorkflow {
    pub trigger: String,
    #[serde(default)]
    pub steps: Vec<PluginWorkflowStepConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PluginWorkflowRunStatus {
    Queued,
    Running,
    Completed,
    PartialFailed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginChainMutation {
    #[serde(default)]
    pub active_filepath: Option<String>,
    #[serde(default)]
    pub active_filename: Option<String>,
    #[serde(default)]
    pub extra_files: Vec<String>,
    #[serde(default)]
    pub metadata_patch: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginChainState {
    pub job_id: String,
    pub source: Option<String>,
    pub download_kind: String,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub history_id: Option<String>,
    #[serde(default)]
    pub time_range: Option<String>,
    pub active_filepath: String,
    pub active_filename: String,
    pub directory: String,
    #[serde(default)]
    pub filesize: Option<u64>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub extra_files: Vec<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginWorkflowRun {
    pub run_id: String,
    pub trigger: String,
    pub status: PluginWorkflowRunStatus,
    pub initial_payload: PostDownloadPluginPayload,
    pub current_chain_state: PluginChainState,
    #[serde(default)]
    pub steps: Vec<PluginWorkflowStepSnapshot>,
    #[serde(default)]
    pub current_step_index: Option<usize>,
    #[serde(default)]
    pub failed_step_plugin_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExecutionStatusEvent {
    pub plugin_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_source: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub media_title: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub media_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExecutionStatusPayload {
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub plugin_name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub resolved_provider: Option<String>,
    #[serde(default)]
    pub resolved_source: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExecutionOutputEvent {
    pub plugin_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    #[serde(rename = "stream")]
    pub stream: String,
    pub chunk: String,
    #[serde(default)]
    pub media_title: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub media_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostDownloadPluginPayload {
    pub job_id: String,
    pub source: Option<String>,
    pub trigger: String,
    pub filepath: String,
    pub filename: String,
    pub directory: String,
    pub filesize: Option<u64>,
    pub format: Option<String>,
    pub quality: Option<String>,
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub history_id: Option<String>,
    pub time_range: Option<String>,
    pub download_kind: String,
    #[serde(default)]
    pub workflow_run_id: Option<String>,
    #[serde(default)]
    pub workflow_step_index: Option<usize>,
    #[serde(default)]
    pub workflow_step_plugin_id: Option<String>,
    #[serde(default)]
    pub chain_state: Option<PluginChainState>,
}
