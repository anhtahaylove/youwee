use super::{get_packaged_dependency_path, select_dependency_path_for_source};
use crate::types::{DependencySource, FfmpegStatus};
use crate::utils::{find_system_binary, unix_system_binary_dirs, CommandExt};
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Manager};
use tokio::process::Command;

const SOURCE_CONFIG_FILE: &str = "ffmpeg-source.txt";
const RELEASE_VERSION_FILE: &str = "ffmpeg-release-version.txt";

pub fn system_ffmpeg_upgrade_message() -> String {
    #[cfg(target_os = "macos")]
    {
        return "System FFmpeg is managed externally. Update it with Homebrew (`brew upgrade ffmpeg`) or switch source to App managed.".to_string();
    }
    #[cfg(target_os = "windows")]
    {
        return "System FFmpeg is managed externally. Update it with your package manager (e.g. `winget`, `choco`, or `scoop`) or switch source to App managed.".to_string();
    }
    #[cfg(target_os = "linux")]
    {
        return "System FFmpeg is managed externally. Update it with your distro package manager or switch source to App managed.".to_string();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "System FFmpeg is managed externally. Update it with your package manager or switch source to App managed.".to_string()
    }
}

fn get_ffmpeg_source_config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|p| p.join("bin").join(SOURCE_CONFIG_FILE))
}

fn get_ffmpeg_release_version_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|p| p.join("bin").join(RELEASE_VERSION_FILE))
}

pub async fn read_app_ffmpeg_release_version(app: &AppHandle) -> Option<String> {
    let version_path = get_ffmpeg_release_version_path(app)?;
    let content = tokio::fs::read_to_string(&version_path).await.ok()?;
    let version = content.trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

pub async fn write_app_ffmpeg_release_version(
    app: &AppHandle,
    version: &str,
) -> Result<(), String> {
    let version_path =
        get_ffmpeg_release_version_path(app).ok_or("Failed to get FFmpeg version path")?;

    if let Some(parent) = version_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create bin directory: {}", e))?;
    }

    tokio::fs::write(&version_path, version)
        .await
        .map_err(|e| format!("Failed to save FFmpeg release version: {}", e))?;

    Ok(())
}

pub async fn get_ffmpeg_source(app: &AppHandle) -> DependencySource {
    if let Some(config_path) = get_ffmpeg_source_config_path(app) {
        if let Ok(content) = tokio::fs::read_to_string(&config_path).await {
            return DependencySource::from_str(content.trim());
        }
    }
    DependencySource::Auto
}

pub async fn set_ffmpeg_source(app: &AppHandle, source: &DependencySource) -> Result<(), String> {
    let config_path = get_ffmpeg_source_config_path(app).ok_or("Failed to get config path")?;

    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create bin directory: {}", e))?;
    }

    tokio::fs::write(&config_path, source.as_str())
        .await
        .map_err(|e| format!("Failed to save source config: {}", e))?;

    Ok(())
}

fn get_app_ffmpeg_path(app: &AppHandle) -> Option<PathBuf> {
    let app_data_dir = app.path().app_data_dir().ok()?;
    let bin_dir = app_data_dir.join("bin");
    #[cfg(windows)]
    let ffmpeg_path = bin_dir.join("ffmpeg.exe");
    #[cfg(not(windows))]
    let ffmpeg_path = bin_dir.join("ffmpeg");

    if ffmpeg_path.exists() {
        Some(ffmpeg_path)
    } else {
        None
    }
}

fn get_system_ffmpeg_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let binary_name = "ffmpeg.exe";
    #[cfg(not(windows))]
    let binary_name = "ffmpeg";

    find_system_binary(binary_name, &unix_system_binary_dirs())
}

fn get_packaged_ffmpeg_path(app: &AppHandle) -> Option<PathBuf> {
    #[cfg(windows)]
    let binary_name = "ffmpeg.exe";
    #[cfg(not(windows))]
    let binary_name = "ffmpeg";

    get_packaged_dependency_path(app, binary_name)
}

/// Get the FFmpeg binary path (app data or system)
pub async fn get_ffmpeg_path(app: &AppHandle) -> Option<PathBuf> {
    let source = get_ffmpeg_source(app).await;
    select_dependency_path_for_source(
        &source,
        get_app_ffmpeg_path(app),
        get_packaged_ffmpeg_path(app),
        get_system_ffmpeg_path(),
    )
}

/// Resolve ffprobe from the same managed/system directory as the selected FFmpeg binary.
pub async fn get_ffprobe_path(app: &AppHandle) -> Option<PathBuf> {
    #[cfg(windows)]
    let binary_name = "ffprobe.exe";
    #[cfg(not(windows))]
    let binary_name = "ffprobe";

    let ffmpeg_path = get_ffmpeg_path(app).await?;
    let ffprobe_path = ffmpeg_path.parent()?.join(binary_name);
    ffprobe_path.is_file().then_some(ffprobe_path)
}

/// Check FFmpeg status
pub async fn check_ffmpeg_internal(app: &AppHandle) -> Result<FfmpegStatus, String> {
    if let Some(ffmpeg_path) = get_ffmpeg_path(app).await {
        let mut cmd = Command::new(&ffmpeg_path);
        cmd.args(["-version"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd.hide_window();

        if let Ok(output) = cmd.output().await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let binary_version = parse_ffmpeg_version(&stdout);
                let is_app_managed = get_app_ffmpeg_path(app).as_ref() == Some(&ffmpeg_path);
                let is_bundled = get_packaged_ffmpeg_path(app).as_ref() == Some(&ffmpeg_path);
                let is_system = get_system_ffmpeg_path().as_ref() == Some(&ffmpeg_path);
                let version = if is_app_managed {
                    read_app_ffmpeg_release_version(app)
                        .await
                        .unwrap_or(binary_version)
                } else {
                    binary_version
                };

                return Ok(FfmpegStatus {
                    installed: true,
                    version: Some(version),
                    binary_path: Some(ffmpeg_path.to_string_lossy().to_string()),
                    is_system,
                    is_bundled,
                });
            }
        }
    }

    Ok(FfmpegStatus {
        installed: false,
        version: None,
        binary_path: None,
        is_system: false,
        is_bundled: false,
    })
}

/// Parse FFmpeg version from output
pub fn parse_ffmpeg_version(output: &str) -> String {
    if let Some(line) = output.lines().next() {
        for prefix in ["ffmpeg version ", "ffprobe version "] {
            if let Some(version_part) = line.strip_prefix(prefix) {
                return version_part
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Extract a sortable date from common FFmpeg build version formats.
/// Examples:
/// - "git-2026-01-25-1e1dde8" -> "2026-01-25"
/// - "2026.01.25" -> "2026-01-25"
/// - "n8.1.2-22-g94138f6973-20260717" -> "2026-07-17"
fn extract_date_from_version(version: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:^|\D)(\d{4})[-.]?(\d{2})[-.]?(\d{2})(?:\D|$)").ok()?;
    let caps = re.captures(version)?;
    let year = caps[1].parse::<u16>().ok()?;
    let month = caps[2].parse::<u8>().ok()?;
    let day = caps[3].parse::<u8>().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn ffmpeg_version_has_update(current_version: &str, latest_version: &str) -> bool {
    let current_date = extract_date_from_version(current_version);
    let latest_date = extract_date_from_version(latest_version);

    match (current_date, latest_date) {
        (Some(curr), Some(lat)) => lat > curr,
        _ => false,
    }
}

/// FFmpeg download info with checksum support
pub struct FfmpegDownloadInfo {
    pub url: &'static str,
    pub archive_type: &'static str,
    pub checksum_url: &'static str,
    pub checksum_filename: &'static str,
}

/// Get FFmpeg download URL for current platform
/// All platforms now support SHA256 checksum verification
pub fn get_ffmpeg_download_info() -> FfmpegDownloadInfo {
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            FfmpegDownloadInfo {
                url: "https://github.com/vanloctech/ffmpeg-macos/releases/latest/download/ffmpeg-macos-arm64.tar.gz",
                archive_type: "tar.gz",
                checksum_url: "https://github.com/vanloctech/ffmpeg-macos/releases/latest/download/ffmpeg-macos-arm64.tar.gz.sha256",
                checksum_filename: "ffmpeg-macos-arm64.tar.gz",
            }
        }
        #[cfg(target_arch = "x86_64")]
        {
            FfmpegDownloadInfo {
                url: "https://github.com/vanloctech/ffmpeg-macos/releases/latest/download/ffmpeg-macos-x64.tar.gz",
                archive_type: "tar.gz",
                checksum_url: "https://github.com/vanloctech/ffmpeg-macos/releases/latest/download/ffmpeg-macos-x64.tar.gz.sha256",
                checksum_filename: "ffmpeg-macos-x64.tar.gz",
            }
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            FfmpegDownloadInfo {
                url: "https://github.com/vanloctech/ffmpeg-macos/releases/latest/download/ffmpeg-macos-arm64.tar.gz",
                archive_type: "tar.gz",
                checksum_url: "https://github.com/vanloctech/ffmpeg-macos/releases/latest/download/ffmpeg-macos-arm64.tar.gz.sha256",
                checksum_filename: "ffmpeg-macos-arm64.tar.gz",
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        FfmpegDownloadInfo {
            url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip",
            archive_type: "zip",
            checksum_url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/checksums.sha256",
            checksum_filename: "ffmpeg-master-latest-win64-gpl.zip",
        }
    }
    #[cfg(target_os = "linux")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            FfmpegDownloadInfo {
                url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linuxarm64-gpl.tar.xz",
                archive_type: "tar.xz",
                checksum_url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/checksums.sha256",
                checksum_filename: "ffmpeg-master-latest-linuxarm64-gpl.tar.xz",
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            FfmpegDownloadInfo {
                url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz",
                archive_type: "tar.xz",
                checksum_url: "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/checksums.sha256",
                checksum_filename: "ffmpeg-master-latest-linux64-gpl.tar.xz",
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        FfmpegDownloadInfo {
            url: "",
            archive_type: "",
            checksum_url: "",
            checksum_filename: "",
        }
    }
}

/// FFmpeg update info
#[derive(Debug, Clone, serde::Serialize)]
pub struct FfmpegUpdateInfo {
    pub has_update: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FfmpegReleaseInfo {
    pub version: String,
    pub html_url: Option<String>,
}

/// Get the GitHub API URL for checking latest release
fn get_ffmpeg_release_api_url() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "https://api.github.com/repos/vanloctech/ffmpeg-macos/releases/latest"
    }
    #[cfg(target_os = "windows")]
    {
        "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest"
    }
    #[cfg(target_os = "linux")]
    {
        "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        ""
    }
}

pub fn normalize_ffmpeg_release_version(tag_name: &str) -> String {
    tag_name
        .trim()
        .strip_prefix("ffmpeg-")
        .unwrap_or(tag_name.trim())
        .trim_start_matches('v')
        .to_string()
}

fn select_ffmpeg_release_version(
    tag_name: &str,
    release_name: Option<&str>,
    published_at: Option<&str>,
) -> String {
    [Some(tag_name), release_name, published_at]
        .into_iter()
        .flatten()
        .find_map(extract_date_from_version)
        .unwrap_or_else(|| normalize_ffmpeg_release_version(tag_name))
}

pub async fn get_latest_ffmpeg_release_info() -> Result<FfmpegReleaseInfo, String> {
    let api_url = get_ffmpeg_release_api_url();
    if api_url.is_empty() {
        return Err("Unsupported platform".to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("Youwee/0.6.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(api_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch release info: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch release info: HTTP {}",
            response.status()
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let tag_name = json["tag_name"].as_str().ok_or("No tag_name in release")?;

    Ok(FfmpegReleaseInfo {
        version: select_ffmpeg_release_version(
            tag_name,
            json["name"].as_str(),
            json["published_at"].as_str(),
        ),
        html_url: json["html_url"].as_str().map(|s| s.to_string()),
    })
}

/// Check if FFmpeg update is available
pub async fn check_ffmpeg_update_internal(app: &AppHandle) -> Result<FfmpegUpdateInfo, String> {
    // Get current installed version
    let current_status = check_ffmpeg_internal(app).await?;

    if !current_status.installed {
        return Ok(FfmpegUpdateInfo {
            has_update: false,
            current_version: None,
            latest_version: None,
            release_url: None,
        });
    }

    let current_version = current_status.version.clone();

    // Only check updates for bundled FFmpeg (not system)
    if current_status.is_system {
        return Ok(FfmpegUpdateInfo {
            has_update: false,
            current_version,
            latest_version: None,
            release_url: Some("System FFmpeg - update via package manager".to_string()),
        });
    }

    let latest_release = get_latest_ffmpeg_release_info().await?;
    let latest_version = latest_release.version;

    // Compare versions by extracting date parts
    // Current version format: "git-2026-01-25-1e1dde8" -> extract "2026-01-25"
    // Latest version format: "2026.01.25" or "ffmpeg-2026.01.25" -> extract "2026.01.25"
    let has_update = if let Some(ref current) = current_version {
        ffmpeg_version_has_update(current, &latest_version)
    } else {
        false
    };

    Ok(FfmpegUpdateInfo {
        has_update,
        current_version,
        latest_version: Some(latest_version),
        release_url: latest_release.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ffmpeg_version_has_update, normalize_ffmpeg_release_version, parse_ffmpeg_version,
        select_ffmpeg_release_version,
    };

    #[test]
    fn normalizes_ffmpeg_macos_release_tags() {
        assert_eq!(
            normalize_ffmpeg_release_version("ffmpeg-2026.06.11"),
            "2026.06.11"
        );
        assert_eq!(
            normalize_ffmpeg_release_version("v2026.06.11"),
            "2026.06.11"
        );
    }

    #[test]
    fn compares_binary_git_versions_with_release_versions() {
        assert!(ffmpeg_version_has_update(
            "git-2026-06-10-5f998e3",
            "2026.06.11"
        ));
        assert!(!ffmpeg_version_has_update("2026.06.11", "2026.06.11"));
    }

    #[test]
    fn compares_compact_btb_n_build_dates() {
        assert!(ffmpeg_version_has_update(
            "n8.1.2-22-g94138f6973-20260717",
            "2026-07-18"
        ));
        assert!(!ffmpeg_version_has_update(
            "n8.1.2-22-g94138f6973-20260717",
            "Latest Auto-Build (2026-07-17 13:22)"
        ));
    }

    #[test]
    fn derives_btb_n_version_from_release_name_when_tag_is_latest() {
        assert_eq!(
            select_ffmpeg_release_version(
                "latest",
                Some("Latest Auto-Build (2026-07-17 13:22)"),
                Some("2026-07-17T14:32:32Z")
            ),
            "2026-07-17"
        );
    }

    #[test]
    fn parses_ffmpeg_and_ffprobe_versions() {
        let version = "n8.1.2-22-g94138f6973-20260717";
        assert_eq!(
            parse_ffmpeg_version(&format!("ffmpeg version {version} Copyright")),
            version
        );
        assert_eq!(
            parse_ffmpeg_version(&format!("ffprobe version {version} Copyright")),
            version
        );
    }
}
