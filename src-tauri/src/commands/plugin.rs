use tauri::AppHandle;

use crate::services::{
    approve_plugin_permissions_internal, create_plugin_scaffold_internal,
    enqueue_plugin_trigger_workflow,
    get_plugin_details_internal, get_plugin_trigger_workflow_internal,
    get_runtime_provider_status_internal,
    inspect_plugin_folder_internal, inspect_plugin_url_internal, inspect_plugin_zip_internal,
    install_plugin_internal, list_plugins_internal, list_runtime_providers_internal,
    open_plugin_directory_internal, set_default_provider_for_language_internal,
    set_plugin_provider_internal, set_plugin_trust_internal, update_plugin_env_values_internal,
    update_plugin_state_internal, update_plugin_trigger_workflow_internal,
    CreatePluginScaffoldInput, InstallPluginSourceInput, PluginEnvValuesInput,
    PluginPermissionApprovalInput,
};
use crate::types::{
    PluginPackageInspection, PluginProvider, PluginRuntimeLanguage, PluginSummary,
    PluginTriggerWorkflow, PluginWorkflowStepSnapshot, PostDownloadPluginPayload,
    RuntimeProviderStatus,
};

#[tauri::command]
pub fn list_plugins(app: AppHandle) -> Result<Vec<PluginSummary>, String> {
    list_plugins_internal(&app)
}

#[tauri::command]
pub fn get_plugin_details(app: AppHandle, plugin_id: String) -> Result<PluginSummary, String> {
    get_plugin_details_internal(&app, &plugin_id)
}

#[tauri::command]
pub async fn inspect_plugin_folder(
    app: AppHandle,
    path: String,
) -> Result<PluginPackageInspection, String> {
    inspect_plugin_folder_internal(&app, path).await
}

#[tauri::command]
pub async fn inspect_plugin_zip(
    app: AppHandle,
    path: String,
) -> Result<PluginPackageInspection, String> {
    inspect_plugin_zip_internal(&app, path).await
}

#[tauri::command]
pub async fn inspect_plugin_url(
    app: AppHandle,
    url: String,
) -> Result<PluginPackageInspection, String> {
    inspect_plugin_url_internal(&app, url).await
}

#[tauri::command]
pub async fn install_plugin(
    app: AppHandle,
    source: InstallPluginSourceInput,
    trusted: bool,
) -> Result<PluginSummary, String> {
    install_plugin_internal(&app, source, trusted).await
}

#[tauri::command]
pub fn create_plugin_scaffold(
    app: AppHandle,
    input: CreatePluginScaffoldInput,
) -> Result<PluginSummary, String> {
    create_plugin_scaffold_internal(&app, input)
}

#[tauri::command]
pub fn update_plugin_state(app: AppHandle, plugin_id: String, enabled: bool) -> Result<(), String> {
    update_plugin_state_internal(&app, &plugin_id, enabled)
}

#[tauri::command]
pub fn get_plugin_trigger_workflow(
    app: AppHandle,
    trigger: String,
) -> Result<PluginTriggerWorkflow, String> {
    get_plugin_trigger_workflow_internal(&app, &trigger)
}

#[tauri::command]
pub fn update_plugin_trigger_workflow(
    app: AppHandle,
    workflow: PluginTriggerWorkflow,
) -> Result<PluginTriggerWorkflow, String> {
    update_plugin_trigger_workflow_internal(&app, workflow)
}

#[tauri::command]
pub fn enqueue_plugin_workflow_trigger(
    app: AppHandle,
    trigger: String,
    payload: PostDownloadPluginPayload,
    workflow_steps: Option<Vec<PluginWorkflowStepSnapshot>>,
) -> Result<Option<String>, String> {
    Ok(enqueue_plugin_trigger_workflow(
        &app,
        &trigger,
        workflow_steps,
        payload,
    ))
}

#[tauri::command]
pub fn approve_plugin_permissions(
    app: AppHandle,
    plugin_id: String,
    permissions: PluginPermissionApprovalInput,
) -> Result<(), String> {
    approve_plugin_permissions_internal(&app, &plugin_id, permissions)
}

#[tauri::command]
pub fn update_plugin_env_values(
    app: AppHandle,
    plugin_id: String,
    input: PluginEnvValuesInput,
) -> Result<(), String> {
    update_plugin_env_values_internal(&app, &plugin_id, input)
}

#[tauri::command]
pub fn set_plugin_trust(app: AppHandle, plugin_id: String, trusted: bool) -> Result<(), String> {
    set_plugin_trust_internal(&app, &plugin_id, trusted)
}

#[tauri::command]
pub fn set_plugin_provider(
    app: AppHandle,
    plugin_id: String,
    provider: PluginProvider,
) -> Result<(), String> {
    set_plugin_provider_internal(&app, &plugin_id, provider)
}

#[tauri::command]
pub async fn open_plugin_directory(app: AppHandle, plugin_id: String) -> Result<(), String> {
    open_plugin_directory_internal(&app, &plugin_id).await
}

#[tauri::command]
pub async fn list_runtime_providers(app: AppHandle) -> Result<Vec<RuntimeProviderStatus>, String> {
    Ok(list_runtime_providers_internal(&app).await)
}

#[tauri::command]
pub async fn get_runtime_provider_status(
    app: AppHandle,
    provider: PluginProvider,
) -> Result<RuntimeProviderStatus, String> {
    Ok(get_runtime_provider_status_internal(&app, provider).await)
}

#[tauri::command]
pub fn set_default_provider_for_language(
    app: AppHandle,
    language: PluginRuntimeLanguage,
    provider: PluginProvider,
) -> Result<(), String> {
    set_default_provider_for_language_internal(&app, language, provider)
}
