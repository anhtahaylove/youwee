use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::process::Command;

use super::{get_packaged_dependency_path, select_preferred_dependency_path};
use crate::types::{BackendError, GalleryDlStatus};
use crate::utils::{find_system_binary, unix_system_binary_dirs, CommandExt};

const GALLERYDL_STABLE_RELEASES_URL: &str = "https://codeberg.org/mikf/gallery-dl/releases";
const GALLERYDL_NIGHTLY_RELEASES_URL: &str = "https://github.com/gdl-org/builds/releases/latest";

#[derive(Clone, Debug, Serialize)]
pub struct GalleryDlUpdateInfo {
    pub has_update: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
}

pub fn system_gallerydl_not_found_message() -> String {
    #[cfg(target_os = "macos")]
    {
        return "System gallery-dl not found. Install it with Homebrew (`brew install gallery-dl`) and ensure `gallery-dl` is available in PATH.".to_string();
    }
    #[cfg(target_os = "windows")]
    {
        return "System gallery-dl not found. Install it with a package manager (e.g. `choco install gallery-dl` or `scoop install gallery-dl`) and ensure `gallery-dl` is available in PATH.".to_string();
    }
    #[cfg(target_os = "linux")]
    {
        return "System gallery-dl not found. Install it with your distro package manager and ensure `gallery-dl` is available in PATH.".to_string();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "System gallery-dl not found. Install it and ensure `gallery-dl` is available in PATH."
            .to_string()
    }
}

pub fn get_system_gallerydl_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let binary_name = "gallery-dl.exe";
    #[cfg(not(windows))]
    let binary_name = "gallery-dl";

    find_system_binary(binary_name, &unix_system_binary_dirs())
}

fn gallerydl_binary_name() -> &'static str {
    #[cfg(windows)]
    {
        "gallery-dl.exe"
    }
    #[cfg(not(windows))]
    {
        "gallery-dl"
    }
}

fn get_app_gallerydl_target_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|error| {
        BackendError::from_message(format!(
            "Failed to resolve gallery-dl app data path: {error}"
        ))
        .to_wire_string()
    })?;

    Ok(app_data_dir.join("bin").join(gallerydl_binary_name()))
}

fn get_app_gallerydl_path(app: &AppHandle) -> Option<PathBuf> {
    let binary_path = get_app_gallerydl_target_path(app).ok()?;

    if binary_path.exists() {
        Some(binary_path)
    } else {
        None
    }
}

fn get_packaged_gallerydl_path(app: &AppHandle) -> Option<PathBuf> {
    get_packaged_dependency_path(app, gallerydl_binary_name())
}

pub fn get_gallerydl_path(app: &AppHandle) -> Option<PathBuf> {
    select_preferred_dependency_path(
        get_app_gallerydl_path(app),
        get_packaged_gallerydl_path(app),
        get_system_gallerydl_path(),
    )
}

pub async fn check_gallerydl_internal(app: &AppHandle) -> Result<GalleryDlStatus, String> {
    let Some(binary_path) = get_gallerydl_path(app) else {
        return Ok(GalleryDlStatus {
            installed: false,
            version: None,
            binary_path: None,
            is_system: false,
            is_bundled: false,
        });
    };

    let mut cmd = Command::new(&binary_path);
    cmd.arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.hide_window();

    let output = cmd.output().await.map_err(|e| {
        BackendError::from_message(format!("Failed to run gallery-dl: {}", e)).to_wire_string()
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BackendError::from_message(format!(
            "gallery-dl command failed: {}",
            stderr.trim()
        ))
        .to_wire_string());
    }

    Ok(GalleryDlStatus {
        installed: true,
        version: Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        binary_path: Some(binary_path.to_string_lossy().to_string()),
        is_system: get_system_gallerydl_path().as_ref() == Some(&binary_path),
        is_bundled: get_packaged_gallerydl_path(app).as_ref() == Some(&binary_path),
    })
}

fn gallerydl_release_url(version: Option<&str>) -> String {
    if version.is_some_and(|value| value.contains("-dev:")) {
        GALLERYDL_NIGHTLY_RELEASES_URL.to_string()
    } else {
        GALLERYDL_STABLE_RELEASES_URL.to_string()
    }
}

fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
    [
        String::from_utf8_lossy(stdout).trim(),
        String::from_utf8_lossy(stderr).trim(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn parse_gallerydl_update_check(
    output: &str,
    fallback_current_version: Option<String>,
) -> Result<GalleryDlUpdateInfo, String> {
    for line in output.lines() {
        if let Some((_, versions)) = line.split_once("A new release is available:") {
            let Some((current, latest)) = versions.trim().split_once("->") else {
                break;
            };
            let current_version = current.trim().to_string();
            let latest_version = latest.trim().to_string();
            return Ok(GalleryDlUpdateInfo {
                has_update: true,
                current_version: Some(current_version.clone()),
                latest_version: Some(latest_version),
                release_url: Some(gallerydl_release_url(Some(&current_version))),
            });
        }

        let normalized = line.to_ascii_lowercase();
        if normalized.contains("up to date") || normalized.contains("up-to-date") {
            return Ok(GalleryDlUpdateInfo {
                has_update: false,
                current_version: fallback_current_version.clone(),
                latest_version: fallback_current_version.clone(),
                release_url: Some(gallerydl_release_url(fallback_current_version.as_deref())),
            });
        }
    }

    Err(BackendError::from_message(format!(
        "gallery-dl returned an unrecognized update status: {}",
        output.trim()
    ))
    .to_wire_string())
}

pub async fn check_gallerydl_update_internal(
    app: &AppHandle,
) -> Result<GalleryDlUpdateInfo, String> {
    let status = check_gallerydl_internal(app).await?;
    if !status.installed || status.is_system {
        return Ok(GalleryDlUpdateInfo {
            has_update: false,
            current_version: status.version,
            latest_version: None,
            release_url: None,
        });
    }

    let binary_path = status.binary_path.as_ref().ok_or_else(|| {
        BackendError::from_message("gallery-dl binary path is unavailable").to_wire_string()
    })?;
    let mut cmd = Command::new(binary_path);
    cmd.args(["--update-check", "--no-colors", "--no-input"])
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.hide_window();

    let output = tokio::time::timeout(Duration::from_secs(45), cmd.output())
        .await
        .map_err(|_| {
            BackendError::from_message("gallery-dl update check timed out").to_wire_string()
        })?
        .map_err(|error| {
            BackendError::from_message(format!("Failed to check gallery-dl updates: {error}"))
                .to_wire_string()
        })?;
    let output_text = command_output_text(&output.stdout, &output.stderr);
    if !output.status.success() {
        return Err(BackendError::from_message(format!(
            "gallery-dl update check failed: {output_text}"
        ))
        .to_wire_string());
    }

    parse_gallerydl_update_check(&output_text, status.version)
}

async fn ensure_app_managed_gallerydl(app: &AppHandle, source: &Path) -> Result<PathBuf, String> {
    let target = get_app_gallerydl_target_path(app)?;
    if source == &target || target.is_file() {
        return Ok(target);
    }

    let target_dir = target.parent().ok_or_else(|| {
        BackendError::from_message("Invalid gallery-dl app data path").to_wire_string()
    })?;
    tokio::fs::create_dir_all(target_dir)
        .await
        .map_err(|error| {
            BackendError::from_message(format!("Failed to create gallery-dl directory: {error}"))
                .to_wire_string()
        })?;

    let bootstrap = target.with_extension("bootstrap");
    if bootstrap.exists() {
        tokio::fs::remove_file(&bootstrap).await.map_err(|error| {
            BackendError::from_message(format!(
                "Failed to remove stale gallery-dl bootstrap: {error}"
            ))
            .to_wire_string()
        })?;
    }
    tokio::fs::copy(source, &bootstrap).await.map_err(|error| {
        BackendError::from_message(format!("Failed to bootstrap gallery-dl: {error}"))
            .to_wire_string()
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = tokio::fs::metadata(&bootstrap)
            .await
            .map_err(|error| {
                BackendError::from_message(format!(
                    "Failed to read gallery-dl permissions: {error}"
                ))
                .to_wire_string()
            })?
            .permissions();
        permissions.set_mode(0o755);
        tokio::fs::set_permissions(&bootstrap, permissions)
            .await
            .map_err(|error| {
                BackendError::from_message(format!("Failed to set gallery-dl permissions: {error}"))
                    .to_wire_string()
            })?;
    }

    tokio::fs::rename(&bootstrap, &target)
        .await
        .map_err(|error| {
            BackendError::from_message(format!("Failed to install gallery-dl bootstrap: {error}"))
                .to_wire_string()
        })?;
    Ok(target)
}

pub async fn update_gallerydl_internal(app: &AppHandle) -> Result<String, String> {
    let status = check_gallerydl_internal(app).await?;
    if !status.installed {
        return Err(BackendError::from_message("gallery-dl is not installed").to_wire_string());
    }
    if status.is_system {
        return Err(BackendError::from_message(
            "System gallery-dl must be updated with the system package manager",
        )
        .to_wire_string());
    }

    let source = PathBuf::from(status.binary_path.ok_or_else(|| {
        BackendError::from_message("gallery-dl binary path is unavailable").to_wire_string()
    })?);
    let binary_path = ensure_app_managed_gallerydl(app, &source).await?;

    let mut cmd = Command::new(&binary_path);
    cmd.args(["--update", "--no-colors", "--no-input"])
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.hide_window();
    let output = tokio::time::timeout(Duration::from_secs(300), cmd.output())
        .await
        .map_err(|_| BackendError::from_message("gallery-dl update timed out").to_wire_string())?
        .map_err(|error| {
            BackendError::from_message(format!("Failed to update gallery-dl: {error}"))
                .to_wire_string()
        })?;
    let output_text = command_output_text(&output.stdout, &output.stderr);
    if !output.status.success() {
        return Err(
            BackendError::from_message(format!("gallery-dl update failed: {output_text}"))
                .to_wire_string(),
        );
    }

    let mut version_cmd = Command::new(&binary_path);
    version_cmd
        .arg("--version")
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    version_cmd.hide_window();
    let version_output = tokio::time::timeout(Duration::from_secs(30), version_cmd.output())
        .await
        .map_err(|_| {
            BackendError::from_message("gallery-dl update verification timed out").to_wire_string()
        })?
        .map_err(|error| {
            BackendError::from_message(format!("Failed to verify gallery-dl update: {error}"))
                .to_wire_string()
        })?;
    if !version_output.status.success() {
        return Err(BackendError::from_message(format!(
            "Updated gallery-dl could not be verified: {}",
            command_output_text(&version_output.stdout, &version_output.stderr)
        ))
        .to_wire_string());
    }

    Ok(String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_gallerydl_update_check;

    #[test]
    fn parses_available_nightly_update() {
        let info = parse_gallerydl_update_check(
            "[update][info] A new release is available: 1.32.7-dev:2026.07.16 -> 2026.07.18",
            Some("1.32.7-dev:2026.07.16".to_string()),
        )
        .expect("update output should parse");

        assert!(info.has_update);
        assert_eq!(
            info.current_version.as_deref(),
            Some("1.32.7-dev:2026.07.16")
        );
        assert_eq!(info.latest_version.as_deref(), Some("2026.07.18"));
        assert_eq!(
            info.release_url.as_deref(),
            Some("https://github.com/gdl-org/builds/releases/latest")
        );
    }

    #[test]
    fn parses_up_to_date_status() {
        let info = parse_gallerydl_update_check(
            "[update][info] gallery-dl is up to date (1.32.6)",
            Some("1.32.6".to_string()),
        )
        .expect("up-to-date output should parse");

        assert!(!info.has_update);
        assert_eq!(info.current_version.as_deref(), Some("1.32.6"));
        assert_eq!(info.latest_version.as_deref(), Some("1.32.6"));
    }

    #[test]
    fn rejects_unknown_update_output() {
        assert!(parse_gallerydl_update_check("unexpected", Some("1.32.6".to_string())).is_err());
    }
}
