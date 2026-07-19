//! Download command - handles video downloading with yt-dlp
//!
//! This module contains the core download functionality including:
//! - Video/audio download with quality/format options
//! - Playlist support
//! - Progress tracking
//! - Subtitle handling

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::utils::{normalize_url, validate_url};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::database::add_log_internal;
use crate::database::update_history_download;
use crate::database::{
    add_history_collection_in_db, add_history_internal, ensure_collection_for_download_in_db,
};
use crate::services::{
    add_safe_filename_args, build_cookie_args, build_proxy_args, build_site_header_args,
    calc_trim_filenames_bytes, enqueue_post_download_workflow, get_bundled_ytdlp_fallback_path,
    get_deno_path, get_ffmpeg_path, get_ytdlp_path, get_ytdlp_source, is_upcoming_live_error,
    resolve_download_workflow_snapshot, run_ytdlp_with_stderr, system_ytdlp_not_found_message,
};
use crate::types::{
    BackendError, DependencySource, DownloadProgress, PluginWorkflowStepSnapshot,
    PostDownloadPluginPayload,
};
use crate::utils::{
    build_format_string, format_size, parse_progress, sanitize_output_path, CommandExt,
};

pub static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);
const DEFAULT_FILENAME_TEMPLATE: &str = "%(title)s.%(ext)s";

const RECENT_OUTPUT_LIMIT: usize = 30;
const REMOTE_THUMBNAIL_MAX_BYTES: usize = 5 * 1024 * 1024;
const REMOTE_THUMBNAIL_CACHE_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Clone)]
struct CoreDownloadFallback {
    binary_path: PathBuf,
    binary_label: String,
    args: Vec<String>,
    home_dir: String,
    path: OsString,
}

#[derive(Clone, Default)]
struct CoreFallbackMetadata {
    title: Option<String>,
    thumbnail: Option<String>,
}

fn extract_time_range(download_sections: &Option<String>) -> Option<String> {
    download_sections.as_ref().and_then(|s| {
        let stripped = s.strip_prefix('*').unwrap_or(s);
        if stripped.is_empty() {
            None
        } else {
            Some(stripped.to_string())
        }
    })
}

async fn skipped_live_status(
    app: &AppHandle,
    url: &str,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
    proxy_url: Option<&str>,
) -> Result<Option<String>, String> {
    let mut args = vec![
        "--print".to_string(),
        "live_status".to_string(),
        "--output-na-placeholder".to_string(),
        "not_live".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
        "--socket-timeout".to_string(),
        "15".to_string(),
    ];

    if url.contains("youtube.com") || url.contains("youtu.be") {
        if let Some(deno_path) = get_deno_path(app).await {
            args.push("--js-runtimes".to_string());
            args.push(format!("deno:{}", deno_path.to_string_lossy()));
        }
    }

    args.push("--".to_string());
    args.push(url.to_string());

    let mut extra_args = build_site_header_args(url);
    extra_args.extend(build_cookie_args(
        url,
        cookie_mode,
        cookie_browser,
        cookie_browser_profile,
        cookie_file_path,
        cookie_skip_patterns,
    ));
    extra_args.extend(build_proxy_args(proxy_url));
    if let Some(separator_index) = args.iter().position(|arg| arg == "--") {
        args.splice(separator_index..separator_index, extra_args);
    }

    let command_str = format!("yt-dlp {}", args.join(" "));
    add_log_internal("command", &command_str, None, Some(url)).ok();

    let args_ref: Vec<&str> = args.iter().map(|arg| arg.as_str()).collect();
    let output = run_ytdlp_with_stderr(app, &args_ref).await?;

    if !output.stderr.trim().is_empty() {
        add_log_internal("stderr", output.stderr.trim(), None, Some(url)).ok();
    }

    if !output.success {
        let message = output.stderr.trim();
        if is_upcoming_live_error(message) {
            return Ok(Some("is_upcoming".to_string()));
        }
        return Err(BackendError::from_message(if message.is_empty() {
            "Failed to check live status before download.".to_string()
        } else {
            format!("Failed to check live status before download: {}", message)
        })
        .to_wire_string());
    }

    let status = output.stdout.lines().last().unwrap_or("").trim();
    if status.is_empty() || status == "not_live" {
        Ok(None)
    } else {
        Ok(Some(status.to_string()))
    }
}

fn workflow_steps_for_trigger(
    app: &AppHandle,
    trigger: &str,
    workflow_snapshots: &BTreeMap<String, Vec<PluginWorkflowStepSnapshot>>,
) -> Vec<PluginWorkflowStepSnapshot> {
    workflow_snapshots
        .get(trigger)
        .cloned()
        .unwrap_or_else(|| resolve_download_workflow_snapshot(app, trigger, None, &[]))
}

#[allow(clippy::too_many_arguments)]
fn build_trigger_payload(
    job_id: &str,
    source: Option<String>,
    trigger: &str,
    output_path: &str,
    filesize: Option<u64>,
    format: Option<String>,
    quality: Option<String>,
    url: &str,
    title: Option<String>,
    thumbnail: Option<String>,
    history_id: Option<String>,
    time_range: Option<String>,
    download_kind: &str,
) -> PostDownloadPluginPayload {
    let display_name = title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| url.to_string());

    PostDownloadPluginPayload {
        job_id: job_id.to_string(),
        source,
        trigger: trigger.to_string(),
        filepath: String::new(),
        filename: display_name,
        directory: output_path.to_string(),
        filesize,
        format,
        quality,
        url: url.to_string(),
        title,
        thumbnail,
        history_id,
        time_range,
        download_kind: download_kind.to_string(),
        workflow_run_id: None,
        workflow_step_index: None,
        workflow_step_plugin_id: None,
        chain_state: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn enqueue_failed_workflow(
    app: &AppHandle,
    workflow_steps: &[PluginWorkflowStepSnapshot],
    job_id: &str,
    source: Option<String>,
    output_path: &str,
    format: Option<String>,
    quality: Option<String>,
    url: &str,
    title: Option<String>,
    thumbnail: Option<String>,
    history_id: Option<String>,
    time_range: Option<String>,
    download_kind: &str,
) {
    if workflow_steps.is_empty() {
        return;
    }

    let payload = build_trigger_payload(
        job_id,
        source,
        "download.failed",
        output_path,
        None,
        format,
        quality,
        url,
        title,
        thumbnail,
        history_id,
        time_range,
        download_kind,
    );
    let _ = enqueue_post_download_workflow(app, workflow_steps.to_vec(), payload);
}

#[allow(clippy::too_many_arguments)]
fn enqueue_before_start_workflow(
    app: &AppHandle,
    workflow_steps: &[PluginWorkflowStepSnapshot],
    job_id: &str,
    source: Option<String>,
    output_path: &str,
    format: Option<String>,
    quality: Option<String>,
    url: &str,
    title: Option<String>,
    thumbnail: Option<String>,
    history_id: Option<String>,
    time_range: Option<String>,
    download_kind: &str,
) {
    if workflow_steps.is_empty() {
        return;
    }

    let payload = build_trigger_payload(
        job_id,
        source,
        "download.beforeStart",
        output_path,
        None,
        format,
        quality,
        url,
        title,
        thumbnail,
        history_id,
        time_range,
        download_kind,
    );
    let _ = enqueue_post_download_workflow(app, workflow_steps.to_vec(), payload);
}

async fn run_completed_plugins(
    app: &AppHandle,
    workflow_steps: &[PluginWorkflowStepSnapshot],
    job_id: &str,
    source: Option<String>,
    filepath: &str,
    filesize: Option<u64>,
    format: Option<String>,
    quality: Option<String>,
    url: &str,
    title: Option<String>,
    thumbnail: Option<String>,
    history_id: Option<String>,
    time_range: Option<String>,
    download_kind: &str,
) {
    if workflow_steps.is_empty() {
        return;
    }

    let path = std::path::Path::new(filepath);
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(filepath)
        .to_string();
    let directory = path
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default();

    let payload = PostDownloadPluginPayload {
        job_id: job_id.to_string(),
        source,
        trigger: "download.completed".to_string(),
        filepath: filepath.to_string(),
        filename,
        directory,
        filesize,
        format,
        quality,
        url: url.to_string(),
        title,
        thumbnail,
        history_id,
        time_range,
        download_kind: download_kind.to_string(),
        workflow_run_id: None,
        workflow_step_index: None,
        workflow_step_plugin_id: None,
        chain_state: None,
    };

    let _ = enqueue_post_download_workflow(app, workflow_steps.to_vec(), payload);
}

/// Decode raw bytes from a child process into a Rust String.
///
/// On Windows with a non-UTF-8 locale (e.g. Chinese → GBK), yt-dlp outputs
/// file paths in the system ANSI code page.  Tokio's `BufReader::lines()`
/// expects UTF-8 and returns `Err` on such bytes, which silently stops the
/// reading loop and loses the filepath — causing history records to never be
/// created.  This helper decodes via the Win32 `MultiByteToWideChar` API so
/// the full filepath (including CJK characters) is preserved.
#[cfg(windows)]
fn decode_process_output(bytes: &[u8]) -> String {
    // Fast path: already valid UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    extern "system" {
        fn MultiByteToWideChar(
            code_page: u32,
            flags: u32,
            multi_byte_str: *const u8,
            multi_byte: i32,
            wide_char_str: *mut u16,
            wide_char: i32,
        ) -> i32;
    }

    const CP_ACP: u32 = 0; // System default Windows ANSI code page

    unsafe {
        let len = MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        );
        if len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; len as usize];
        MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            len,
        );
        OsString::from_wide(&wide).to_string_lossy().into_owned()
    }
}

#[cfg(not(windows))]
fn decode_process_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Kill all yt-dlp and ffmpeg processes
fn kill_all_download_processes() {
    #[cfg(unix)]
    {
        use std::process::Command as StdCommand;
        StdCommand::new("pkill")
            .args(["-9", "-f", "yt-dlp"])
            .spawn()
            .ok();
        StdCommand::new("pkill")
            .args(["-9", "-f", "ffmpeg"])
            .spawn()
            .ok();
    }
    #[cfg(windows)]
    {
        use crate::utils::CommandExt as _;
        use std::process::Command as StdCommand;
        let mut cmd1 = StdCommand::new("taskkill");
        cmd1.args(["/F", "/IM", "yt-dlp.exe"]);
        cmd1.hide_window();
        cmd1.spawn().ok();

        let mut cmd2 = StdCommand::new("taskkill");
        cmd2.args(["/F", "/IM", "ffmpeg.exe"]);
        cmd2.hide_window();
        cmd2.spawn().ok();
    }
}

fn push_recent_output(buffer: &mut VecDeque<String>, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if buffer.len() >= RECENT_OUTPUT_LIMIT {
        buffer.pop_front();
    }
    buffer.push_back(trimmed.to_string());
}

fn push_recent_output_shared(buffer: &Arc<Mutex<VecDeque<String>>>, line: &str) {
    if let Ok(mut guard) = buffer.lock() {
        push_recent_output(&mut guard, line);
    }
}

fn recent_output_snapshot(buffer: &Arc<Mutex<VecDeque<String>>>) -> Vec<String> {
    buffer
        .lock()
        .map(|guard| guard.iter().cloned().collect())
        .unwrap_or_default()
}

fn is_facebook_reel_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(|host| host.to_ascii_lowercase()) else {
        return false;
    };
    let is_facebook = host == "facebook.com" || host.ends_with(".facebook.com");
    let path = parsed.path();
    is_facebook && (path == "/reel" || path.starts_with("/reel/"))
}

fn is_placeholder_facebook_title(title: &str) -> bool {
    let title = title.trim();
    title.is_empty()
        || title.eq_ignore_ascii_case("video")
        || title.eq_ignore_ascii_case("facebook video")
        || reqwest::Url::parse(title).is_ok()
}

fn escape_metadata_replacement(value: &str) -> String {
    value.replace('\\', "\\\\").replace('$', "\\$")
}

fn push_facebook_title_replacement_args(args: &mut Vec<String>, url: &str, title: Option<&str>) {
    let Some(title) = title
        .map(str::trim)
        .filter(|title| !is_placeholder_facebook_title(title))
    else {
        return;
    };
    if !is_facebook_reel_url(url) {
        return;
    }

    args.extend([
        "--replace-in-metadata".to_string(),
        "title".to_string(),
        r"(?is)^\s*(?:video|facebook video|https?://\S+|(?:(?:\d+(?:[.,]\d+)?[kmb]?\s*(?:comments?|reactions?|views?|likes?|shares?))\s*(?:[|·•—–-]\s*)?)+.*)$"
            .to_string(),
        escape_metadata_replacement(title),
    ]);
}

fn is_facebook_parse_failure(recent_lines: &[String]) -> bool {
    recent_lines.iter().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("[facebook]") && lower.contains("cannot parse data")
    })
}

fn should_core_fallback_facebook_reel(url: &str, recent_lines: &[String]) -> bool {
    is_facebook_reel_url(url) && is_facebook_parse_failure(recent_lines)
}

fn push_arg_before_url(args: &mut Vec<String>, name: &str, value: &str) {
    if args.iter().any(|arg| arg == name) {
        return;
    }
    let insert_at = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    args.splice(insert_at..insert_at, [name.to_string(), value.to_string()]);
}

fn facebook_reel_fallback_basename(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .filter(|id| {
            id.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
        .map(|id| format!("facebook-com-reel-{}", id))
        .unwrap_or_else(|| "facebook-com-reel".to_string())
}

fn is_facebook_reel_fallback_stem(value: &str) -> bool {
    value.starts_with("facebook-com-reel-")
}

fn title_from_download_filename(current_title: Option<String>, filename: &str) -> Option<String> {
    let filename = filename.trim_end_matches(" has already been downloaded");
    let stem = filename.rfind('.').map(|end| &filename[..end])?;
    if current_title.is_some() && is_facebook_reel_fallback_stem(stem) {
        current_title
    } else {
        Some(stem.to_string())
    }
}

fn replace_output_template(args: &mut [String], output_template: String) {
    if let Some(index) = args.iter().position(|arg| arg == "-o") {
        if let Some(value) = args.get_mut(index + 1) {
            *value = output_template;
        }
    }
}

fn replace_output_path(args: &mut Vec<String>, output_directory: &str) {
    let path_arg = output_path_arg(output_directory);
    if let Some(index) = args.iter().position(|arg| arg == "--paths") {
        if let Some(value) = args.get_mut(index + 1) {
            *value = path_arg;
        } else {
            args.push(path_arg);
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "-o") {
        args.splice(index..index, ["--paths".to_string(), path_arg]);
    } else {
        args.push("--paths".to_string());
        args.push(path_arg);
    }
}

fn normalize_filename_template(filename_template: Option<String>) -> String {
    let candidate = filename_template.unwrap_or_default();
    let trimmed = candidate.trim();
    if trimmed.is_empty() || trimmed.contains('\0') || trimmed.chars().any(char::is_control) {
        return DEFAULT_FILENAME_TEMPLATE.to_string();
    }

    let leaf = trimmed
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if leaf.is_empty() || leaf == "." || leaf == ".." {
        return DEFAULT_FILENAME_TEMPLATE.to_string();
    }
    if leaf.contains("%(ext") {
        leaf
    } else {
        format!("{leaf}.%(ext)s")
    }
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn source_directory_name(url: &str) -> &'static str {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return "other";
    };
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return "other";
    };

    if host_matches(&host, "youtube.com")
        || host == "youtu.be"
        || host_matches(&host, "youtube-nocookie.com")
    {
        "youtube"
    } else if host_matches(&host, "tiktok.com") {
        "tiktok"
    } else if host_matches(&host, "instagram.com") {
        "instagram"
    } else if host_matches(&host, "facebook.com") || host == "fb.watch" {
        "facebook"
    } else if host_matches(&host, "twitter.com") || host == "x.com" {
        "twitter"
    } else if host_matches(&host, "bilibili.com") || host == "b23.tv" {
        "bilibili"
    } else if host_matches(&host, "vimeo.com") {
        "vimeo"
    } else if host_matches(&host, "twitch.tv") {
        "twitch"
    } else if host_matches(&host, "soundcloud.com") {
        "soundcloud"
    } else if host_matches(&host, "dailymotion.com") {
        "dailymotion"
    } else {
        "other"
    }
}

fn build_output_directory(
    output_directory: &str,
    url: &str,
    organize_by_source: Option<bool>,
) -> String {
    let base = output_directory.trim_end_matches(|ch| ch == '/' || ch == '\\');
    if organize_by_source.unwrap_or(false) {
        format!("{}/{}", base, source_directory_name(url))
    } else {
        base.to_string()
    }
}

fn number_width(total: Option<u32>) -> usize {
    total
        .filter(|value| *value >= 100)
        .map(|value| value.to_string().len())
        .unwrap_or(2)
}

fn build_item_prefix(
    number_playlist_items: bool,
    download_playlist: bool,
    playlist_index: Option<u32>,
    playlist_total: Option<u32>,
    number_queue_items: bool,
    queue_index: Option<u32>,
    queue_total: Option<u32>,
) -> String {
    if number_playlist_items {
        let width = number_width(playlist_total);
        if let Some(index) = playlist_index {
            return format!("{index:0width$} - ");
        }
        if download_playlist {
            return format!("%(playlist_index)0{width}d - ");
        }
    }

    if number_queue_items && !download_playlist {
        if let Some(index) = queue_index {
            let width = number_width(queue_total);
            return format!("{index:0width$} - ");
        }
    }

    String::new()
}

fn output_path_arg(output_directory: &str) -> String {
    format!(
        "home:{}",
        output_directory.trim_end_matches(|ch| ch == '/' || ch == '\\')
    )
}

fn build_output_template(filename_template: Option<String>, item_prefix: &str) -> String {
    format!(
        "{}{}",
        item_prefix,
        normalize_filename_template(filename_template)
    )
}

fn output_collision_suffix(policy: Option<&str>, job_id: &str) -> Result<Option<String>, String> {
    match policy.unwrap_or("overwrite") {
        "overwrite" => Ok(None),
        "unique" => {
            let suffix: String = job_id
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .take(8)
                .collect();
            Ok(Some(if suffix.is_empty() {
                "copy".to_string()
            } else {
                format!("copy-{suffix}")
            }))
        }
        value => Err(format!("Unsupported output collision policy: {value}")),
    }
}

fn apply_output_collision_suffix(mut template: String, suffix: Option<&str>) -> String {
    let Some(suffix) = suffix else {
        return template;
    };
    let label = format!(" [{suffix}]");
    if let Some(extension_index) = template.rfind(".%(ext") {
        template.insert_str(extension_index, &label);
    } else {
        template.push_str(&label);
    }
    template
}

fn constrain_title_template_for_output(template: String, output_directory: &str) -> String {
    if !cfg!(target_os = "windows") {
        return template;
    }

    let title_limit = calc_trim_filenames_bytes(output_directory);
    template.replace("%(title)s", &format!("%(title).{title_limit}B"))
}

fn build_chapter_output_template(item_prefix: &str, number_chapter_files: bool) -> String {
    let chapter_prefix = if number_chapter_files {
        "%(section_number)02d - "
    } else {
        ""
    };
    format!("{}{}%(section_title)s.%(ext)s", item_prefix, chapter_prefix)
}

fn parse_printed_filepaths(contents: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in contents.lines() {
        let path = line.trim();
        push_unique_filepath(&mut paths, path);
    }
    paths
}

fn parse_split_chapter_filepaths<'a>(lines: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut paths = Vec::new();
    for line in lines {
        if !line.contains("[SplitChapters]") || !line.contains("Destination:") {
            continue;
        }
        if let Some((_, path)) = line.split_once("Destination:") {
            push_unique_filepath(&mut paths, path.trim());
        }
    }
    paths
}

fn is_media_filepath(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        ".mp3", ".m4a", ".opus", ".mp4", ".mkv", ".webm", ".flac", ".wav",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension))
}

fn clean_ytdlp_filepath(value: &str) -> &str {
    value
        .trim()
        .trim_matches(|ch| ch == '"' || ch == '\'')
        .trim()
}

fn filepath_from_download_output(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let candidate = if let Some((_, path)) = trimmed.split_once("Destination:") {
        clean_ytdlp_filepath(path)
    } else if let Some(path) = trimmed.strip_prefix("[Merger] Merging formats into ") {
        clean_ytdlp_filepath(path)
    } else if let Some(path) = trimmed
        .strip_prefix("[download] ")
        .and_then(|value| value.strip_suffix(" has already been downloaded"))
    {
        clean_ytdlp_filepath(path)
    } else {
        return None;
    };

    if is_media_filepath(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn push_unique_filepath(paths: &mut Vec<String>, path: &str) {
    if path.is_empty() || paths.iter().any(|existing| existing == path) {
        return;
    }
    paths.push(path.to_string());
}

fn output_filepaths(
    printed_filepaths: &[String],
    chapter_filepaths: &[String],
    final_filepath: &Option<String>,
) -> Vec<String> {
    let mut paths = Vec::new();
    if printed_filepaths.is_empty() {
        if let Some(filepath) = final_filepath {
            push_unique_filepath(&mut paths, filepath);
        }
    } else {
        for filepath in printed_filepaths {
            push_unique_filepath(&mut paths, filepath);
        }
    }
    for filepath in chapter_filepaths {
        push_unique_filepath(&mut paths, filepath);
    }
    paths
}

fn newest_media_filepath_in_dir(output_directory: &str) -> Option<String> {
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in std::fs::read_dir(output_directory).ok()?.flatten() {
        let path = entry.path();
        let path_text = path.to_string_lossy().to_string();
        if !is_media_filepath(&path_text) {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
        if newest
            .as_ref()
            .map(|(current_modified, _)| modified > *current_modified)
            .unwrap_or(true)
        {
            newest = Some((modified, path_text));
        }
    }

    newest.map(|(_, path)| path)
}

fn title_from_filepath(filepath: &str) -> Option<String> {
    Path::new(filepath)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

fn display_title_for_download(
    metadata_title: Option<String>,
    current_title: Option<String>,
    final_filepath: &Option<String>,
    total_count: Option<u32>,
) -> Option<String> {
    let filepath_title = || {
        final_filepath
            .as_ref()
            .and_then(|path| title_from_filepath(path))
    };

    if total_count.unwrap_or(1) > 1 {
        current_title.or_else(filepath_title).or(metadata_title)
    } else {
        metadata_title.or_else(filepath_title).or(current_title)
    }
}

fn build_auto_collection_names(
    enabled: bool,
    playlist_collection_name: Option<&str>,
) -> Vec<String> {
    if !enabled {
        return Vec::new();
    }

    let mut names = Vec::new();
    if let Some(raw_name) = playlist_collection_name {
        let name = raw_name.trim();
        let normalized = name.to_lowercase();
        if !name.is_empty()
            && !names
                .iter()
                .any(|existing: &String| existing.trim().to_lowercase() == normalized)
        {
            names.push(name.to_string());
        }
    }
    names
}

fn build_output_collection_names(
    base_names: &[String],
    enabled: bool,
    include_split_parent: bool,
    parent_title: Option<&str>,
) -> Vec<String> {
    let mut names = base_names.to_vec();
    if !enabled || !include_split_parent {
        return names;
    }

    if let Some(raw_name) = parent_title {
        let name = raw_name.trim();
        let normalized = name.to_lowercase();
        if !name.is_empty()
            && !names
                .iter()
                .any(|existing| existing.trim().to_lowercase() == normalized)
        {
            names.push(name.to_string());
        }
    }

    names
}

fn assign_history_auto_collections(history_id: &str, collection_names: &[String]) {
    for collection_name in collection_names {
        let result =
            ensure_collection_for_download_in_db(collection_name, None).and_then(|collection| {
                add_history_collection_in_db(history_id.to_string(), collection.id)
            });
        if let Err(error) = result {
            add_log_internal(
                "error",
                "Failed to organize download into collection",
                Some(&error),
                None,
            )
            .ok();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn save_download_outputs_to_history(
    output_paths: &[String],
    history_id: Option<&String>,
    url: &str,
    display_title: Option<String>,
    thumbnail: Option<String>,
    source: Option<String>,
    quality_display: Option<String>,
    format: &str,
    reported_filesize: Option<u64>,
    download_sections: &Option<String>,
    auto_collection_names: &[String],
) -> Option<String> {
    let mut progress_history_id = None;

    for (index, filepath) in output_paths.iter().enumerate() {
        let time_range = extract_time_range(download_sections);
        let file_filesize = std::fs::metadata(filepath)
            .ok()
            .map(|m| m.len())
            .or(if index == 0 { reported_filesize } else { None });
        let entry_title = if index == 0 {
            display_title
                .clone()
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            title_from_filepath(filepath)
                .or_else(|| display_title.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        };

        if index == 0 {
            if let Some(hist_id) = history_id {
                update_history_download(
                    hist_id.clone(),
                    filepath.clone(),
                    file_filesize,
                    quality_display.clone(),
                    Some(format.to_string()),
                    time_range,
                    display_title.clone(),
                    thumbnail.clone(),
                )
                .ok();
                assign_history_auto_collections(hist_id, auto_collection_names);
                progress_history_id = Some(hist_id.clone());
                continue;
            }
        }

        let history_row_id = add_history_internal(
            url.to_string(),
            entry_title,
            thumbnail.clone().or_else(|| generate_thumbnail_url(url)),
            filepath.clone(),
            file_filesize,
            None,
            quality_display.clone(),
            Some(format.to_string()),
            effective_source(&source, url),
            time_range,
        )
        .ok();
        if let Some(ref hist_id) = history_row_id {
            assign_history_auto_collections(hist_id, auto_collection_names);
        }
        if index == 0 {
            progress_history_id = history_row_id;
        }
    }

    progress_history_id
}

fn push_overwrite_args(args: &mut Vec<String>, skip_existing: Option<bool>) {
    if skip_existing.unwrap_or(false) {
        args.push("--no-overwrites".to_string());
    } else {
        // Force overwrite to avoid HTTP 416 errors from stale .part files.
        args.push("--force-overwrites".to_string());
    }
}

fn push_filename_safety_args(args: &mut Vec<String>, output_path: Option<&str>) {
    add_safe_filename_args(args, output_path);
}

fn facebook_reel_core_fallback_args(
    args: &[String],
    url: &str,
    output_directory: &str,
    item_prefix: &str,
    title: Option<&str>,
    collision_suffix: Option<&str>,
) -> Vec<String> {
    let mut fallback_args = Vec::with_capacity(args.len() + 4);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--cookies" | "--cookies-from-browser" | "--downloader" | "--downloader-args" => {
                index += 2;
            }
            _ => {
                fallback_args.push(args[index].clone());
                index += 1;
            }
        }
    }
    push_arg_before_url(&mut fallback_args, "--downloader", "native");
    push_arg_before_url(&mut fallback_args, "--impersonate", "chrome");
    replace_output_path(&mut fallback_args, output_directory);
    if title.is_none_or(is_placeholder_facebook_title) {
        replace_output_template(
            &mut fallback_args,
            apply_output_collision_suffix(
                format!(
                    "{}{}.%(ext)s",
                    item_prefix,
                    facebook_reel_fallback_basename(url)
                ),
                collision_suffix,
            ),
        );
    }
    fallback_args
}

fn spawn_core_download(
    fallback: &CoreDownloadFallback,
    args: &[String],
) -> Result<tokio::process::Child, String> {
    let mut cmd = Command::new(&fallback.binary_path);
    cmd.args(args)
        .env("HOME", &fallback.home_dir)
        .env("PATH", &fallback.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.hide_window();
    cmd.spawn()
        .map_err(|error| format!("Failed to start core fallback yt-dlp: {}", error))
}

fn json_string_field(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn thumbnail_from_info_json(json: &serde_json::Value) -> Option<String> {
    json_string_field(json, "thumbnail").or_else(|| {
        json.get("thumbnails")
            .and_then(|value| value.as_array())
            .and_then(|items| {
                items
                    .iter()
                    .rev()
                    .filter_map(|item| json_string_field(item, "url"))
                    .next()
            })
    })
}

fn metadata_from_info_json(json: &serde_json::Value) -> CoreFallbackMetadata {
    CoreFallbackMetadata {
        title: json_string_field(json, "title")
            .filter(|title| !is_placeholder_facebook_title(title)),
        thumbnail: thumbnail_from_info_json(json).map(|value| {
            if let Some(rest) = value.strip_prefix("http://") {
                format!("https://{}", rest)
            } else {
                value
            }
        }),
    }
}

async fn probe_core_facebook_reel_metadata(
    fallback: &CoreDownloadFallback,
    url: &str,
) -> Option<CoreFallbackMetadata> {
    let args = vec![
        "--dump-single-json",
        "--skip-download",
        "--no-warnings",
        "--no-playlist",
        "--socket-timeout",
        "15",
        "--impersonate",
        "chrome",
        "--",
        url,
    ];

    let mut cmd = Command::new(&fallback.binary_path);
    cmd.args(&args)
        .env("HOME", &fallback.home_dir)
        .env("PATH", &fallback.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.hide_window();

    let output = match cmd.output().await {
        Ok(output) => output,
        Err(error) => {
            add_log_internal(
                "warning",
                &format!("Facebook Reel metadata probe could not start: {}", error),
                None,
                Some(url),
            )
            .ok();
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        add_log_internal(
            "warning",
            &format!("Facebook Reel metadata probe failed: {}", stderr),
            None,
            Some(url),
        )
        .ok();
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(json) => json,
        Err(error) => {
            add_log_internal(
                "warning",
                &format!(
                    "Facebook Reel metadata probe returned invalid JSON: {}",
                    error
                ),
                None,
                Some(url),
            )
            .ok();
            return None;
        }
    };
    let metadata = metadata_from_info_json(&json);
    if metadata.title.is_some() || metadata.thumbnail.is_some() {
        add_log_internal(
            "info",
            "Facebook Reel metadata recovered in core fallback",
            metadata.title.as_deref(),
            Some(url),
        )
        .ok();
        Some(metadata)
    } else {
        None
    }
}

fn is_aria2_not_found_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    (lower.contains("aria2c") || lower.contains("aria2"))
        && (lower.contains("not found")
            || lower.contains("no such file")
            || lower.contains("is not recognized"))
}

fn normalize_aria2_args(raw_args: &str) -> Option<String> {
    let trimmed = raw_args.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("aria2c:") {
        let normalized = rest.trim_start();
        return if normalized.is_empty() {
            None
        } else {
            Some(format!("aria2c:{}", normalized))
        };
    }
    if let Some(rest) = trimmed.strip_prefix("aria2:") {
        let normalized = rest.trim_start();
        return if normalized.is_empty() {
            None
        } else {
            Some(format!("aria2c:{}", normalized))
        };
    }
    Some(format!("aria2c:{}", trimmed))
}

fn build_download_error_message(exit_code: Option<i32>, recent_lines: &[String]) -> BackendError {
    if recent_lines
        .iter()
        .any(|line| is_aria2_not_found_line(line))
    {
        return BackendError::new(
            crate::types::code::ARIA2_NOT_FOUND,
            "aria2c not found. Install aria2 and ensure aria2c is available in PATH.",
        )
        .with_retryable(false);
    }

    let reason = recent_lines
        .iter()
        .rev()
        .find(|line| {
            let lower = line.to_lowercase();
            lower.contains("error")
                || lower.contains("unable")
                || lower.contains("failed")
                || lower.contains("http error")
                || lower.contains("forbidden")
                || lower.contains("too many requests")
                || lower.contains("timed out")
        })
        .cloned()
        .or_else(|| recent_lines.last().cloned())
        .unwrap_or_else(|| "Unknown error".to_string());

    match exit_code {
        Some(code) => {
            BackendError::from_message(format!("Download failed (exit code {}): {}", code, reason))
                .with_param("exitCode", code)
        }
        None => BackendError::from_message(format!("Download failed: {}", reason)),
    }
}

fn download_cancelled_error() -> BackendError {
    BackendError::new(crate::types::code::DOWNLOAD_CANCELLED, "Download cancelled")
        .with_retryable(false)
}

fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com") || url.contains("youtu.be")
}

fn normalize_youtube_player_client(value: Option<&str>) -> Option<&'static str> {
    match value.unwrap_or("auto") {
        "web" => Some("web"),
        "mweb" => Some("mweb"),
        "tv" => Some("tv"),
        "ios" => Some("ios"),
        "android" => Some("android"),
        "web_safari" => Some("web_safari"),
        _ => None,
    }
}

fn build_youtube_extractor_args(
    use_actual_player_js: bool,
    youtube_player_client: Option<&str>,
) -> Option<String> {
    let mut values = Vec::new();

    if let Some(client) = normalize_youtube_player_client(youtube_player_client) {
        values.push(format!("player_client={client}"));
    }
    if use_actual_player_js {
        values.push("player_js_version=actual".to_string());
    }

    if values.is_empty() {
        None
    } else {
        Some(format!("youtube:{}", values.join(",")))
    }
}

#[tauri::command]
pub async fn download_video(
    app: AppHandle,
    id: String,
    url: String,
    output_path: String,
    filename_template: Option<String>,
    skip_existing: Option<bool>,
    organize_by_source: Option<bool>,
    quality: String,
    format: String,
    download_playlist: bool,
    playlist_index: Option<u32>,
    playlist_total: Option<u32>,
    number_playlist_items: Option<bool>,
    queue_index: Option<u32>,
    queue_total: Option<u32>,
    number_queue_items: Option<bool>,
    split_embedded_chapters: Option<bool>,
    number_chapter_files: Option<bool>,
    auto_organize_collections: Option<bool>,
    playlist_collection_name: Option<String>,
    video_codec: String,
    preferred_fps: Option<String>,
    audio_bitrate: String,
    playlist_limit: Option<u32>,
    subtitle_mode: String,
    subtitle_langs: String,
    subtitle_embed: bool,
    subtitle_format: String,
    log_stderr: Option<bool>,
    _use_bun_runtime: Option<bool>, // Deprecated - now auto uses deno
    use_actual_player_js: Option<bool>,
    youtube_player_client: Option<String>,
    history_id: Option<String>,
    output_collision_policy: Option<String>,
    // Cookie settings
    cookie_mode: Option<String>,
    cookie_browser: Option<String>,
    cookie_browser_profile: Option<String>,
    cookie_file_path: Option<String>,
    cookie_skip_patterns: Option<Vec<String>>,
    // Embed settings
    embed_metadata: Option<bool>,
    embed_thumbnail: Option<bool>,
    // Proxy settings
    proxy_url: Option<String>,
    // Live stream settings
    live_from_start: Option<bool>,
    skip_live: Option<bool>,
    // Speed limit settings
    speed_limit: Option<String>,
    // External downloader settings
    use_aria2: Option<bool>,
    aria2_args: Option<String>,
    // SponsorBlock settings
    sponsorblock_remove: Option<String>, // comma-separated categories to remove
    sponsorblock_mark: Option<String>,   // comma-separated categories to mark as chapters
    // Download sections (time range)
    download_sections: Option<String>, // e.g. "*10:30-14:30" for partial download
    // Title (optional, passed from frontend for display purposes)
    title: Option<String>,
    // Thumbnail URL (optional, passed from frontend for non-YouTube sites)
    thumbnail: Option<String>,
    // Source/extractor name (optional, from yt-dlp extractor e.g. "BiliBili", "TikTok")
    source: Option<String>,
    // Legacy snapshot of plugin ids enabled when the job was queued
    post_download_plugins: Option<Vec<String>>,
    // Snapshot of workflow steps by trigger at queue time
    plugin_workflow_snapshots: Option<BTreeMap<String, Vec<PluginWorkflowStepSnapshot>>>,
    // Full workflow step snapshot used for post-processing
    post_download_workflow_steps: Option<Vec<PluginWorkflowStepSnapshot>>,
    // When false, caller is responsible for firing the final download.failed workflow.
    emit_failed_workflow: Option<bool>,
    // Caller context used in plugin payload
    download_kind: Option<String>,
) -> Result<(), String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);
    validate_url(&url).map_err(|e| BackendError::from_message(e).to_wire_string())?;
    let url = normalize_url(&url);
    let post_download_plugins = post_download_plugins.unwrap_or_default();
    let mut plugin_workflow_snapshots = plugin_workflow_snapshots.unwrap_or_default();
    if !plugin_workflow_snapshots.contains_key("download.completed") {
        let completed_steps = resolve_download_workflow_snapshot(
            &app,
            "download.completed",
            post_download_workflow_steps,
            &post_download_plugins,
        );
        plugin_workflow_snapshots.insert("download.completed".to_string(), completed_steps);
    }
    let emit_failed_workflow = emit_failed_workflow.unwrap_or(true);
    let download_kind = download_kind.unwrap_or_else(|| "download".to_string());

    if skip_live.unwrap_or(false) {
        if let Some(live_status) = skipped_live_status(
            &app,
            &url,
            cookie_mode.as_deref(),
            cookie_browser.as_deref(),
            cookie_browser_profile.as_deref(),
            cookie_file_path.as_deref(),
            cookie_skip_patterns.as_deref(),
            proxy_url.as_deref(),
        )
        .await?
        {
            add_log_internal(
                "info",
                &format!(
                    "Skipped live video due to --skip-live (status: {})",
                    live_status
                ),
                None,
                Some(&url),
            )
            .ok();
            return Err(BackendError::new(
                crate::types::code::YT_SKIPPED_LIVE,
                format!("Skipped live video (status: {})", live_status),
            )
            .with_retryable(false)
            .with_param("liveStatus", live_status)
            .to_wire_string());
        }
    }

    let should_log_stderr = log_stderr.unwrap_or(true);
    let sanitized_path = sanitize_output_path(&output_path)
        .map_err(|e| BackendError::from_message(e).to_wire_string())?;
    let format_string =
        build_format_string(&quality, &format, &video_codec, preferred_fps.as_deref());
    let output_directory = build_output_directory(&sanitized_path, &url, organize_by_source);
    if !output_directory.is_empty() {
        tokio::fs::create_dir_all(&output_directory)
            .await
            .map_err(|e| {
                BackendError::from_message(format!("Failed to create output directory: {}", e))
                    .to_wire_string()
            })?;
    }
    let item_prefix = build_item_prefix(
        number_playlist_items.unwrap_or(false),
        download_playlist,
        playlist_index,
        playlist_total,
        number_queue_items.unwrap_or(false),
        queue_index,
        queue_total,
    );
    let split_embedded_chapters = split_embedded_chapters.unwrap_or(false);
    let number_chapter_files = number_chapter_files.unwrap_or(true);
    let auto_organize_collections_enabled = auto_organize_collections.unwrap_or(false);
    let auto_collection_names = build_auto_collection_names(
        auto_organize_collections_enabled,
        playlist_collection_name.as_deref(),
    );
    let collision_suffix = output_collision_suffix(output_collision_policy.as_deref(), &id)
        .map_err(|error| BackendError::from_message(error).to_wire_string())?;
    let output_template = constrain_title_template_for_output(
        apply_output_collision_suffix(
            build_output_template(filename_template, &item_prefix),
            collision_suffix.as_deref(),
        ),
        &output_directory,
    );

    // Use a temp file to capture the final filepath from yt-dlp.
    // On Windows with non-UTF-8 locales (e.g. Chinese/GBK), stdout is encoded
    // in the system ANSI code page which cannot represent all Unicode characters
    // (such as ⧸ U+29F8 used by yt-dlp to replace / in filenames).
    // --print-to-file always writes UTF-8, so we get the exact filepath.
    let filepath_tmp = std::env::temp_dir().join(format!("youwee-fp-{}.txt", id));

    let mut args = vec![
        "--newline".to_string(),
        "--progress".to_string(),
        "--no-warnings".to_string(),
        "-f".to_string(),
        format_string,
        "--paths".to_string(),
        output_path_arg(&output_directory),
        "-o".to_string(),
        output_template,
        "--print-to-file".to_string(),
        "after_move:filepath".to_string(),
        filepath_tmp.to_string_lossy().to_string(),
        "--no-keep-video".to_string(),
        "--no-keep-fragments".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "--fragment-retries".to_string(),
        "3".to_string(),
        "--extractor-retries".to_string(),
        "2".to_string(),
        "--file-access-retries".to_string(),
        "2".to_string(),
    ];

    if split_embedded_chapters {
        args.push("--split-chapters".to_string());
        args.push("-o".to_string());
        args.push(format!(
            "chapter:{}",
            apply_output_collision_suffix(
                build_chapter_output_template(&item_prefix, number_chapter_files),
                collision_suffix.as_deref(),
            )
        ));
    }

    // Auto use Deno runtime for YouTube (required for JS extractor)
    // Use --js-runtimes instead of --extractor-args (handles spaces in path correctly)
    if is_youtube_url(&url) {
        if let Some(deno_path) = get_deno_path(&app).await {
            args.push("--js-runtimes".to_string());
            args.push(format!("deno:{}", deno_path.to_string_lossy()));
        }
    }

    // Add YouTube extractor args if enabled (fixes some YouTube download issues)
    // See: https://github.com/yt-dlp/yt-dlp/issues/14680
    if is_youtube_url(&url) {
        if let Some(extractor_args) = build_youtube_extractor_args(
            use_actual_player_js.unwrap_or(false),
            youtube_player_client.as_deref(),
        ) {
            args.push("--extractor-args".to_string());
            args.push(extractor_args);
        }
    }

    // Add FFmpeg location if available
    if let Some(ffmpeg_path) = get_ffmpeg_path(&app).await {
        if let Some(parent) = ffmpeg_path.parent() {
            args.push("--ffmpeg-location".to_string());
            args.push(parent.to_string_lossy().to_string());
        }
    }

    // Subtitle settings
    if subtitle_mode != "off" {
        args.push("--write-subs".to_string());
        if subtitle_mode == "auto" {
            args.push("--write-auto-subs".to_string());
            args.push("--sub-langs".to_string());
            args.push("all".to_string());
        } else {
            args.push("--sub-langs".to_string());
            args.push(subtitle_langs.clone());
        }
        args.push("--sub-format".to_string());
        args.push(subtitle_format.clone());
        if subtitle_embed {
            args.push("--embed-subs".to_string());
        }
    }

    args.extend(build_site_header_args(&url));

    args.extend(build_cookie_args(
        &url,
        cookie_mode.as_deref(),
        cookie_browser.as_deref(),
        cookie_browser_profile.as_deref(),
        cookie_file_path.as_deref(),
        cookie_skip_patterns.as_deref(),
    ));

    // Proxy settings
    if let Some(proxy) = proxy_url.as_ref() {
        if !proxy.is_empty() {
            args.push("--proxy".to_string());
            args.push(proxy.clone());
        }
    }

    // Live stream settings
    if live_from_start.unwrap_or(false) {
        args.push("--live-from-start".to_string());
        args.push("--no-part".to_string());
    }

    // Speed limit settings
    if let Some(limit) = speed_limit.as_ref() {
        if !limit.is_empty() {
            args.push("--limit-rate".to_string());
            args.push(limit.clone());
        }
    }

    // External downloader settings (aria2c)
    if use_aria2.unwrap_or(false) {
        args.push("--downloader".to_string());
        args.push("aria2c".to_string());
        if let Some(raw_args) = aria2_args.as_ref() {
            if let Some(normalized_args) = normalize_aria2_args(raw_args) {
                args.push("--downloader-args".to_string());
                args.push(normalized_args);
            }
        }
    }

    push_overwrite_args(&mut args, skip_existing);
    push_filename_safety_args(&mut args, Some(&output_directory));

    // Playlist handling
    if !download_playlist {
        args.push("--no-playlist".to_string());
    } else if let Some(limit) = playlist_limit {
        if limit > 0 {
            args.push("--playlist-end".to_string());
            args.push(limit.to_string());
        }
    }

    // Audio formats
    let is_audio_format =
        format == "mp3" || format == "m4a" || format == "opus" || quality == "audio";

    if is_audio_format {
        args.push("-x".to_string());
        args.push("--audio-format".to_string());
        match format.as_str() {
            "mp3" => args.push("mp3".to_string()),
            "m4a" => args.push("m4a".to_string()),
            "opus" => args.push("opus".to_string()),
            _ => args.push("mp3".to_string()),
        }
        args.push("--audio-quality".to_string());
        match audio_bitrate.as_str() {
            "128" => args.push("128K".to_string()),
            _ => args.push("0".to_string()),
        }
    } else {
        args.push("--merge-output-format".to_string());
        args.push(format.clone());
    }

    // Embed metadata and thumbnail
    if embed_metadata.unwrap_or(false) {
        args.push("--embed-metadata".to_string());
    }
    if embed_thumbnail.unwrap_or(false) {
        args.push("--embed-thumbnail".to_string());
        // Convert thumbnail to jpg for better compatibility with MP4 container
        args.push("--convert-thumbnails".to_string());
        args.push("jpg".to_string());
    }

    // SponsorBlock settings
    if let Some(ref remove_cats) = sponsorblock_remove {
        if !remove_cats.is_empty() {
            args.push("--sponsorblock-remove".to_string());
            args.push(remove_cats.clone());
        }
    }
    if let Some(ref mark_cats) = sponsorblock_mark {
        if !mark_cats.is_empty() {
            args.push("--sponsorblock-mark".to_string());
            args.push(mark_cats.clone());
        }
    }

    // Download sections (time range)
    if let Some(ref sections) = download_sections {
        if !sections.is_empty() {
            args.push("--download-sections".to_string());
            args.push(sections.clone());
        }
    }

    push_facebook_title_replacement_args(&mut args, &url, title.as_deref());

    args.push("--".to_string());
    args.push(url.clone());

    // Get binary info for logging
    let binary_info = get_ytdlp_path(&app).await;
    let binary_path_str = binary_info
        .as_ref()
        .map(|(p, is_bundled)| format!("{} (bundled: {})", p.display(), is_bundled))
        .unwrap_or_else(|| "sidecar".to_string());

    // Log command with binary path
    let command_str = format!("[{}] yt-dlp {}", binary_path_str, args.join(" "));
    add_log_internal("command", &command_str, None, Some(&url)).ok();

    let trigger_source = effective_source(&source, &url);
    let trigger_time_range = extract_time_range(&download_sections);
    let before_start_steps =
        workflow_steps_for_trigger(&app, "download.beforeStart", &plugin_workflow_snapshots);
    let completed_workflow_steps =
        workflow_steps_for_trigger(&app, "download.completed", &plugin_workflow_snapshots);
    let failed_workflow_steps =
        workflow_steps_for_trigger(&app, "download.failed", &plugin_workflow_snapshots);

    // Try to get yt-dlp path (prioritizes bundled version for stability)
    if let Some((mut binary_path, selected_is_bundled)) = binary_info {
        // Build extended PATH with deno/bun locations for JavaScript runtime support
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/Users".to_string());
        let mut path_entries: Vec<std::path::PathBuf> = std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).collect())
            .unwrap_or_default();
        path_entries.extend([
            std::path::PathBuf::from(&home_dir).join(".deno/bin"),
            std::path::PathBuf::from(&home_dir).join(".bun/bin"),
            std::path::PathBuf::from("/opt/homebrew/bin"),
            std::path::PathBuf::from("/usr/local/bin"),
        ]);
        let extended_path = std::env::join_paths(path_entries)
            .unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default());

        let spawn_ytdlp = |path: &Path| {
            let mut cmd = Command::new(path);
            cmd.args(&args)
                .env("HOME", &home_dir)
                .env("PATH", &extended_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            cmd.hide_window();
            cmd.spawn()
        };
        let mut active_binary_label = binary_path_str.clone();
        let process_result = match spawn_ytdlp(&binary_path) {
            Ok(process) => Ok(process),
            Err(primary_error) => {
                if let Some(fallback_path) =
                    get_bundled_ytdlp_fallback_path(&binary_path, selected_is_bundled)
                {
                    add_log_internal(
                        "info",
                        &format!(
                            "Selected yt-dlp could not start; retrying bundled binary at {}",
                            fallback_path.display()
                        ),
                        None,
                        Some(&url),
                    )
                    .ok();
                    match spawn_ytdlp(&fallback_path) {
                        Ok(process) => {
                            binary_path = fallback_path;
                            active_binary_label =
                                format!("{} (bundled: true)", binary_path.display());
                            Ok(process)
                        }
                        Err(fallback_error) => Err(format!(
                            "selected binary failed ({primary_error}); bundled fallback failed ({fallback_error})"
                        )),
                    }
                } else {
                    Err(primary_error.to_string())
                }
            }
        };

        let process = match process_result {
            Ok(process) => process,
            Err(error) => {
                if emit_failed_workflow {
                    enqueue_failed_workflow(
                        &app,
                        &failed_workflow_steps,
                        &id,
                        trigger_source.clone(),
                        &sanitized_path,
                        Some(format.clone()),
                        Some(quality.clone()),
                        &url,
                        title.clone(),
                        thumbnail.clone(),
                        history_id.clone(),
                        trigger_time_range.clone(),
                        &download_kind,
                    );
                }
                return Err(BackendError::from_message(format!(
                    "Failed to start yt-dlp: {}",
                    error
                ))
                .to_wire_string());
            }
        };

        enqueue_before_start_workflow(
            &app,
            &before_start_steps,
            &id,
            trigger_source.clone(),
            &sanitized_path,
            Some(format.clone()),
            Some(quality.clone()),
            &url,
            title.clone(),
            thumbnail.clone(),
            history_id.clone(),
            trigger_time_range.clone(),
            &download_kind,
        );

        let core_fallback = CoreDownloadFallback {
            binary_path,
            binary_label: active_binary_label,
            args: args.clone(),
            home_dir,
            path: extended_path,
        };

        return handle_tokio_download(
            app,
            id,
            process,
            quality,
            format,
            url,
            should_log_stderr,
            title,
            thumbnail,
            source,
            download_sections,
            history_id.clone(),
            filepath_tmp.clone(),
            sanitized_path.clone(),
            completed_workflow_steps.clone(),
            failed_workflow_steps.clone(),
            emit_failed_workflow,
            download_kind.clone(),
            item_prefix.clone(),
            auto_collection_names.clone(),
            auto_organize_collections_enabled,
            split_embedded_chapters,
            collision_suffix,
            Some(core_fallback),
        )
        .await;
    }

    let ytdlp_source = get_ytdlp_source(&app).await;
    if ytdlp_source == DependencySource::System {
        if emit_failed_workflow {
            enqueue_failed_workflow(
                &app,
                &failed_workflow_steps,
                &id,
                trigger_source.clone(),
                &sanitized_path,
                Some(format.clone()),
                Some(quality.clone()),
                &url,
                title.clone(),
                thumbnail.clone(),
                history_id.clone(),
                trigger_time_range.clone(),
                &download_kind,
            );
        }
        return Err(BackendError::new(
            crate::types::code::YTDLP_SYSTEM_NOT_FOUND,
            system_ytdlp_not_found_message(),
        )
        .to_wire_string());
    }

    // Fallback to sidecar
    let sidecar_result = app.shell().sidecar("yt-dlp");

    match sidecar_result {
        Ok(sidecar) => {
            let (mut rx, child) = match sidecar.args(&args).spawn() {
                Ok(result) => result,
                Err(error) => {
                    if emit_failed_workflow {
                        enqueue_failed_workflow(
                            &app,
                            &failed_workflow_steps,
                            &id,
                            trigger_source.clone(),
                            &sanitized_path,
                            Some(format.clone()),
                            Some(quality.clone()),
                            &url,
                            title.clone(),
                            thumbnail.clone(),
                            history_id.clone(),
                            trigger_time_range.clone(),
                            &download_kind,
                        );
                    }
                    return Err(BackendError::from_message(format!(
                        "Failed to start bundled yt-dlp: {}",
                        error
                    ))
                    .to_wire_string());
                }
            };

            enqueue_before_start_workflow(
                &app,
                &before_start_steps,
                &id,
                trigger_source.clone(),
                &sanitized_path,
                Some(format.clone()),
                Some(quality.clone()),
                &url,
                title.clone(),
                thumbnail.clone(),
                history_id.clone(),
                trigger_time_range.clone(),
                &download_kind,
            );

            // Only use frontend title if it's not a URL (placeholder)
            let metadata_title: Option<String> = title.clone().filter(|t| !t.starts_with("http"));
            let mut current_title = metadata_title.clone();
            let mut current_index: Option<u32> = None;
            let mut total_count: Option<u32> = None;
            let mut total_filesize: u64 = 0;
            let mut current_stream_size: Option<u64> = None;
            let mut final_filepath: Option<String> = None;
            let mut printed_filepaths: Vec<String> = Vec::new();
            let mut recent_output: VecDeque<String> = VecDeque::new();

            let quality_display = match quality.as_str() {
                "8k" => Some("8K".to_string()),
                "4k" => Some("4K".to_string()),
                "2k" => Some("2K".to_string()),
                "1080" => Some("1080p".to_string()),
                "720" => Some("720p".to_string()),
                "480" => Some("480p".to_string()),
                "360" => Some("360p".to_string()),
                "audio" => Some("Audio".to_string()),
                "best" => Some("Best".to_string()),
                _ => None,
            };

            while let Some(event) = rx.recv().await {
                if CANCEL_FLAG.load(Ordering::SeqCst) {
                    child.kill().ok();
                    kill_all_download_processes();
                    return Err(BackendError::from_message("Download cancelled").to_wire_string());
                }

                match event {
                    CommandEvent::Stdout(line_bytes) => {
                        let line = decode_process_output(&line_bytes);
                        push_recent_output(&mut recent_output, &line);

                        // Parse playlist item info
                        if line.contains("Downloading item") {
                            if let Some(re) =
                                regex::Regex::new(r"Downloading item (\d+) of (\d+)").ok()
                            {
                                if let Some(caps) = re.captures(&line) {
                                    current_index =
                                        caps.get(1).and_then(|m| m.as_str().parse().ok());
                                    total_count = caps.get(2).and_then(|m| m.as_str().parse().ok());
                                }
                            }
                        }

                        // Extract title from [download] messages
                        // Handles both: "Destination: /path/file.mp4" and "/path/file.mp4 has already been downloaded"
                        if line.contains("[download]")
                            && (line.contains("Destination:")
                                || line.contains("has already been downloaded")
                                || line.contains("[ExtractAudio]"))
                        {
                            let path_sep = if line.contains('\\') { '\\' } else { '/' };
                            if let Some(start) = line.rfind(path_sep) {
                                let filename = &line[start + 1..];
                                current_title =
                                    title_from_download_filename(current_title, filename);
                            }
                        }

                        if let Some(filepath) = filepath_from_download_output(&line) {
                            final_filepath = Some(filepath);
                        }

                        // Capture final filepath
                        let trimmed = line.trim();
                        if !trimmed.is_empty()
                            && !trimmed.starts_with('[')
                            && !trimmed.starts_with("Deleting")
                            && !trimmed.starts_with("WARNING")
                            && !trimmed.starts_with("ERROR")
                            && is_media_filepath(trimmed)
                        {
                            final_filepath = Some(trimmed.to_string());
                        }

                        // Parse filesize
                        if line.contains(" of ")
                            && (line.contains("MiB")
                                || line.contains("GiB")
                                || line.contains("KiB"))
                        {
                            if let Some(re) =
                                regex::Regex::new(r"of\s+(\d+(?:\.\d+)?)\s*(GiB|MiB|KiB)").ok()
                            {
                                if let Some(caps) = re.captures(&line) {
                                    if let (Some(num), Some(unit)) = (caps.get(1), caps.get(2)) {
                                        if let Ok(size) = num.as_str().parse::<f64>() {
                                            let size_bytes = match unit.as_str() {
                                                "GiB" => (size * 1024.0 * 1024.0 * 1024.0) as u64,
                                                "MiB" => (size * 1024.0 * 1024.0) as u64,
                                                "KiB" => (size * 1024.0) as u64,
                                                _ => size as u64,
                                            };
                                            if current_stream_size != Some(size_bytes) {
                                                if let Some(prev_size) = current_stream_size {
                                                    total_filesize += prev_size;
                                                }
                                                current_stream_size = Some(size_bytes);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Parse progress
                        if let Some((percent, speed, eta, pi, pc, downloaded_size, elapsed_time)) =
                            parse_progress(&line)
                        {
                            if pi.is_some() {
                                current_index = pi;
                            }
                            if pc.is_some() {
                                total_count = pc;
                            }

                            let progress = DownloadProgress {
                                id: id.clone(),
                                percent,
                                speed,
                                eta,
                                status: "downloading".to_string(),
                                title: current_title.clone(),
                                thumbnail: thumbnail.clone(),
                                source: effective_source(&source, &url),
                                playlist_index: current_index,
                                playlist_count: total_count,
                                filesize: None,
                                resolution: None,
                                format_ext: None,
                                error_message: None,
                                error_code: None,
                                error_params: None,
                                history_id: None,
                                filepath: None,
                                downloaded_size,
                                elapsed_time,
                            };
                            app.emit("download-progress", progress).ok();
                        }
                    }
                    CommandEvent::Stderr(bytes) => {
                        let stderr_line = decode_process_output(&bytes);
                        let stderr_line = stderr_line.trim().to_string();
                        push_recent_output(&mut recent_output, &stderr_line);

                        if let Some((percent, speed, eta, pi, pc, downloaded_size, elapsed_time)) =
                            parse_progress(&stderr_line)
                        {
                            if pi.is_some() {
                                current_index = pi;
                            }
                            if pc.is_some() {
                                total_count = pc;
                            }

                            let progress = DownloadProgress {
                                id: id.clone(),
                                percent,
                                speed,
                                eta,
                                status: "downloading".to_string(),
                                title: current_title.clone(),
                                thumbnail: thumbnail.clone(),
                                source: effective_source(&source, &url),
                                playlist_index: current_index,
                                playlist_count: total_count,
                                filesize: None,
                                resolution: None,
                                format_ext: None,
                                error_message: None,
                                error_code: None,
                                error_params: None,
                                history_id: None,
                                filepath: None,
                                downloaded_size,
                                elapsed_time,
                            };
                            app.emit("download-progress", progress).ok();
                        }

                        if should_log_stderr && !stderr_line.is_empty() {
                            add_log_internal("stderr", &stderr_line, None, Some(&url)).ok();
                        }

                        if let Some(filepath) = filepath_from_download_output(&stderr_line) {
                            final_filepath = Some(filepath);
                        }
                    }
                    CommandEvent::Error(err) => {
                        let error = BackendError::from_message(format!("Process error: {}", err));
                        add_log_internal("error", error.message(), None, Some(&url)).ok();
                        if emit_failed_workflow {
                            enqueue_failed_workflow(
                                &app,
                                &failed_workflow_steps,
                                &id,
                                effective_source(&source, &url),
                                &sanitized_path,
                                Some(format.clone()),
                                quality_display.clone().or_else(|| Some(quality.clone())),
                                &url,
                                current_title.clone(),
                                thumbnail.clone(),
                                history_id.clone(),
                                extract_time_range(&download_sections),
                                &download_kind,
                            );
                        }
                        return Err(error.to_wire_string());
                    }
                    CommandEvent::Terminated(status) => {
                        if CANCEL_FLAG.load(Ordering::SeqCst) {
                            add_log_internal(
                                "info",
                                "Download cancelled by user",
                                None,
                                Some(&url),
                            )
                            .ok();
                            return Err(
                                BackendError::from_message("Download cancelled").to_wire_string()
                            );
                        }

                        // Primary filepath source: read from --print-to-file temp file (UTF-8)
                        if let Ok(contents) = std::fs::read_to_string(&filepath_tmp) {
                            printed_filepaths = parse_printed_filepaths(&contents);
                            if let Some(path) = printed_filepaths.first() {
                                final_filepath = Some(path.clone());
                            }
                        }
                        std::fs::remove_file(&filepath_tmp).ok();

                        if status.code == Some(0) {
                            let chapter_filepaths =
                                parse_split_chapter_filepaths(recent_output.iter());
                            let final_path_exists = final_filepath
                                .as_ref()
                                .map(|path| Path::new(path).is_file())
                                .unwrap_or(false);
                            if !final_path_exists {
                                final_filepath = newest_media_filepath_in_dir(&sanitized_path);
                            }
                            let output_paths = output_filepaths(
                                &printed_filepaths,
                                &chapter_filepaths,
                                &final_filepath,
                            );
                            let actual_filesize = output_paths
                                .first()
                                .and_then(|fp| std::fs::metadata(fp).ok())
                                .map(|m| m.len());

                            let reported_filesize = actual_filesize.or_else(|| {
                                if let Some(last_size) = current_stream_size {
                                    Some(total_filesize + last_size)
                                } else if total_filesize > 0 {
                                    Some(total_filesize)
                                } else {
                                    None
                                }
                            });

                            let display_title = display_title_for_download(
                                metadata_title.clone(),
                                current_title.clone(),
                                &final_filepath,
                                total_count,
                            );
                            let output_collection_names = build_output_collection_names(
                                &auto_collection_names,
                                auto_organize_collections_enabled,
                                split_embedded_chapters && output_paths.len() > 1,
                                display_title.as_deref(),
                            );
                            let completed_thumbnail = resolve_completed_thumbnail(
                                &app,
                                &url,
                                thumbnail.clone(),
                                &output_paths,
                            )
                            .await;

                            // Log success
                            let success_msg = format!(
                                "Downloaded: {}",
                                display_title
                                    .clone()
                                    .unwrap_or_else(|| "Unknown".to_string())
                            );
                            let details = format!(
                                "Size: {} · Quality: {} · Format: {}",
                                reported_filesize
                                    .map(format_size)
                                    .unwrap_or_else(|| "Unknown".to_string()),
                                quality_display.clone().unwrap_or_else(|| quality.clone()),
                                format.clone()
                            );
                            add_log_internal("success", &success_msg, Some(&details), Some(&url))
                                .ok();

                            let progress_history_id = save_download_outputs_to_history(
                                &output_paths,
                                history_id.as_ref(),
                                &url,
                                display_title.clone(),
                                completed_thumbnail.clone(),
                                source.clone(),
                                quality_display.clone(),
                                &format,
                                reported_filesize,
                                &download_sections,
                                &output_collection_names,
                            );

                            let progress = DownloadProgress {
                                id: id.clone(),
                                percent: 100.0,
                                speed: String::new(),
                                eta: String::new(),
                                status: "finished".to_string(),
                                title: display_title.clone(),
                                thumbnail: completed_thumbnail.clone(),
                                source: effective_source(&source, &url),
                                playlist_index: current_index,
                                playlist_count: total_count,
                                filesize: reported_filesize,
                                resolution: quality_display.clone(),
                                format_ext: Some(format.clone()),
                                error_message: None,
                                error_code: None,
                                error_params: None,
                                history_id: progress_history_id.clone(),
                                filepath: final_filepath.clone(),
                                downloaded_size: None,
                                elapsed_time: None,
                            };
                            app.emit("download-progress", progress).ok();
                            for (index, filepath) in output_paths.iter().enumerate() {
                                let file_filesize = std::fs::metadata(filepath)
                                    .ok()
                                    .map(|m| m.len())
                                    .or(if index == 0 { reported_filesize } else { None });
                                let plugin_title = if index == 0 {
                                    display_title.clone()
                                } else {
                                    title_from_filepath(filepath).or_else(|| display_title.clone())
                                };
                                run_completed_plugins(
                                    &app,
                                    &completed_workflow_steps,
                                    &id,
                                    effective_source(&source, &url),
                                    filepath,
                                    file_filesize,
                                    Some(format.clone()),
                                    quality_display.clone().or_else(|| Some(quality.clone())),
                                    &url,
                                    plugin_title,
                                    completed_thumbnail.clone(),
                                    if index == 0 {
                                        progress_history_id.clone()
                                    } else {
                                        None
                                    },
                                    extract_time_range(&download_sections),
                                    &download_kind,
                                )
                                .await;
                            }
                            return Ok(());
                        } else {
                            if CANCEL_FLAG.load(Ordering::SeqCst) {
                                let error = download_cancelled_error();
                                add_log_internal("info", error.message(), None, Some(&url)).ok();
                                return Err(error.to_wire_string());
                            }

                            let recent_lines: Vec<String> = recent_output.iter().cloned().collect();
                            let error = build_download_error_message(status.code, &recent_lines);
                            add_log_internal("error", error.message(), None, Some(&url)).ok();

                            // Emit error progress so frontend can display error message
                            let progress = DownloadProgress {
                                id: id.clone(),
                                percent: 0.0,
                                speed: String::new(),
                                eta: String::new(),
                                status: "error".to_string(),
                                title: current_title.clone(),
                                thumbnail: thumbnail.clone(),
                                source: effective_source(&source, &url),
                                playlist_index: current_index,
                                playlist_count: total_count,
                                filesize: None,
                                resolution: None,
                                format_ext: None,
                                error_message: Some(error.message().to_string()),
                                error_code: Some(error.code().to_string()),
                                error_params: error.params().cloned(),
                                history_id: None,
                                filepath: None,
                                downloaded_size: None,
                                elapsed_time: None,
                            };
                            app.emit("download-progress", progress).ok();

                            if emit_failed_workflow && !failed_workflow_steps.is_empty() {
                                let payload = build_trigger_payload(
                                    &id,
                                    effective_source(&source, &url),
                                    "download.failed",
                                    &sanitized_path,
                                    None,
                                    Some(format.clone()),
                                    quality_display.clone().or_else(|| Some(quality.clone())),
                                    &url,
                                    current_title.clone(),
                                    thumbnail.clone(),
                                    history_id.clone(),
                                    extract_time_range(&download_sections),
                                    &download_kind,
                                );
                                let _ = enqueue_post_download_workflow(
                                    &app,
                                    failed_workflow_steps.clone(),
                                    payload,
                                );
                            }

                            return Err(error.to_wire_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        Err(_) => {
            if ytdlp_source == DependencySource::App {
                if emit_failed_workflow {
                    enqueue_failed_workflow(
                        &app,
                        &failed_workflow_steps,
                        &id,
                        trigger_source.clone(),
                        &sanitized_path,
                        Some(format.clone()),
                        Some(quality.clone()),
                        &url,
                        title.clone(),
                        thumbnail.clone(),
                        history_id.clone(),
                        trigger_time_range.clone(),
                        &download_kind,
                    );
                }
                return Err(BackendError::new(
                    crate::types::code::YTDLP_APP_NOT_FOUND,
                    "App-managed yt-dlp not found. Please install it from Settings > Dependencies.",
                )
                .with_retryable(false)
                .to_wire_string());
            }

            // Fallback to system yt-dlp
            let mut cmd = Command::new("yt-dlp");
            cmd.args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            cmd.hide_window();

            let process = match cmd.spawn() {
                Ok(process) => process,
                Err(error) => {
                    if emit_failed_workflow {
                        enqueue_failed_workflow(
                            &app,
                            &failed_workflow_steps,
                            &id,
                            trigger_source,
                            &sanitized_path,
                            Some(format.clone()),
                            Some(quality.clone()),
                            &url,
                            title.clone(),
                            thumbnail.clone(),
                            history_id.clone(),
                            trigger_time_range,
                            &download_kind,
                        );
                    }
                    return Err(BackendError::from_message(format!(
                        "Failed to start yt-dlp: {}",
                        error
                    ))
                    .to_wire_string());
                }
            };

            enqueue_before_start_workflow(
                &app,
                &before_start_steps,
                &id,
                effective_source(&source, &url),
                &sanitized_path,
                Some(format.clone()),
                Some(quality.clone()),
                &url,
                title.clone(),
                thumbnail.clone(),
                history_id.clone(),
                extract_time_range(&download_sections),
                &download_kind,
            );

            handle_tokio_download(
                app,
                id,
                process,
                quality,
                format,
                url,
                should_log_stderr,
                title,
                thumbnail,
                source,
                download_sections,
                history_id.clone(),
                filepath_tmp,
                sanitized_path,
                completed_workflow_steps,
                failed_workflow_steps,
                emit_failed_workflow,
                download_kind,
                item_prefix,
                auto_collection_names,
                auto_organize_collections_enabled,
                split_embedded_chapters,
                collision_suffix,
                None,
            )
            .await
        }
    }
}

async fn handle_tokio_download(
    app: AppHandle,
    id: String,
    mut process: tokio::process::Child,
    quality: String,
    format: String,
    url: String,
    should_log_stderr: bool,
    title: Option<String>,
    thumbnail: Option<String>,
    source: Option<String>,
    download_sections: Option<String>,
    history_id: Option<String>,
    filepath_tmp: std::path::PathBuf,
    output_directory: String,
    completed_workflow_steps: Vec<PluginWorkflowStepSnapshot>,
    failed_workflow_steps: Vec<PluginWorkflowStepSnapshot>,
    emit_failed_workflow: bool,
    download_kind: String,
    item_prefix: String,
    auto_collection_names: Vec<String>,
    auto_organize_collections_enabled: bool,
    split_embedded_chapters: bool,
    collision_suffix: Option<String>,
    core_fallback: Option<CoreDownloadFallback>,
) -> Result<(), String> {
    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| BackendError::from_message("Failed to get stdout").to_wire_string())?;
    let stderr = process.stderr.take();
    let mut stdout_reader = BufReader::new(stdout);

    // Only use frontend title if it's not a URL (placeholder)
    let metadata_title: Option<String> = title.filter(|t| !t.starts_with("http"));
    let mut current_title = metadata_title.clone();
    let mut current_index: Option<u32> = None;
    let mut total_count: Option<u32> = None;
    let mut total_filesize: u64 = 0;
    let mut current_stream_size: Option<u64> = None;
    let mut final_filepath: Option<String> = None;
    let mut printed_filepaths: Vec<String> = Vec::new();
    let recent_output = Arc::new(Mutex::new(VecDeque::new()));
    let stderr_filepath: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let quality_display = match quality.as_str() {
        "8k" => Some("8K".to_string()),
        "4k" => Some("4K".to_string()),
        "2k" => Some("2K".to_string()),
        "1080" => Some("1080p".to_string()),
        "720" => Some("720p".to_string()),
        "480" => Some("480p".to_string()),
        "360" => Some("360p".to_string()),
        "audio" => Some("Audio".to_string()),
        "best" => Some("Best".to_string()),
        _ => None,
    };

    // Spawn task to read stderr in parallel (for live stream progress)
    let stderr_app = app.clone();
    let stderr_id = id.clone();
    let stderr_url = url.clone();
    let stderr_thumbnail = thumbnail.clone();
    let stderr_source = effective_source(&source, &url);
    let stderr_recent_output = recent_output.clone();
    let stderr_fp_clone = stderr_filepath.clone();
    let stderr_task = if let Some(stderr_handle) = stderr {
        Some(tokio::spawn(async move {
            let mut stderr_reader = BufReader::new(stderr_handle);
            let mut line_buf = Vec::new();
            loop {
                line_buf.clear();
                match stderr_reader.read_until(b'\n', &mut line_buf).await {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
                while line_buf.last().map_or(false, |&b| b == b'\n' || b == b'\r') {
                    line_buf.pop();
                }
                let line = decode_process_output(&line_buf);

                if CANCEL_FLAG.load(Ordering::SeqCst) {
                    break;
                }
                push_recent_output_shared(&stderr_recent_output, &line);

                // On Windows, yt-dlp may print --print after_move:filepath to stderr.
                // Capture it here as a fallback in case stdout doesn't contain the path.
                let t = line.trim();
                if !t.is_empty() && !t.starts_with('[') && is_media_filepath(t) {
                    if let Ok(mut guard) = stderr_fp_clone.lock() {
                        *guard = Some(t.to_string());
                    }
                }

                if let Some(filepath) = filepath_from_download_output(&line) {
                    if let Ok(mut guard) = stderr_fp_clone.lock() {
                        *guard = Some(filepath);
                    }
                }

                // Capture audio filepath from [ExtractAudio] Destination lines in stderr
                // e.g. "[ExtractAudio] Destination: C:\Users\...\song.mp3"
                if line.contains("[ExtractAudio]") && line.contains("Destination:") {
                    if let Some(pos) = line.find("Destination:") {
                        let path = line[pos + "Destination:".len()..].trim();
                        if !path.is_empty() {
                            if let Ok(mut guard) = stderr_fp_clone.lock() {
                                *guard = Some(path.to_string());
                            }
                        }
                    }
                }

                // Parse progress from stderr (live streams output here)
                if let Some((percent, speed, eta, pi, pc, downloaded_size, elapsed_time)) =
                    parse_progress(&line)
                {
                    let progress = DownloadProgress {
                        id: stderr_id.clone(),
                        percent,
                        speed,
                        eta,
                        status: "downloading".to_string(),
                        title: None,
                        thumbnail: stderr_thumbnail.clone(),
                        source: stderr_source.clone(),
                        playlist_index: pi,
                        playlist_count: pc,
                        filesize: None,
                        resolution: None,
                        format_ext: None,
                        error_message: None,
                        error_code: None,
                        error_params: None,
                        history_id: None,
                        filepath: None,
                        downloaded_size,
                        elapsed_time,
                    };
                    stderr_app.emit("download-progress", progress).ok();
                }

                // Log stderr if enabled
                if should_log_stderr && !line.trim().is_empty() {
                    add_log_internal("stderr", line.trim(), None, Some(&stderr_url)).ok();
                }
            }
        }))
    } else {
        None
    };

    // Read stdout — use raw byte reading + decode_process_output to handle
    // non-UTF-8 encodings (e.g. GBK on Chinese Windows).
    let mut stdout_line_buf = Vec::new();
    loop {
        stdout_line_buf.clear();
        match stdout_reader.read_until(b'\n', &mut stdout_line_buf).await {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }
        while stdout_line_buf
            .last()
            .map_or(false, |&b| b == b'\n' || b == b'\r')
        {
            stdout_line_buf.pop();
        }
        let line = decode_process_output(&stdout_line_buf);

        if CANCEL_FLAG.load(Ordering::SeqCst) {
            process.kill().await.ok();
            kill_all_download_processes();
            return Err(BackendError::from_message("Download cancelled").to_wire_string());
        }
        push_recent_output_shared(&recent_output, &line);

        // Parse progress and emit events
        if let Some((percent, speed, eta, pi, pc, downloaded_size, elapsed_time)) =
            parse_progress(&line)
        {
            if pi.is_some() {
                current_index = pi;
            }
            if pc.is_some() {
                total_count = pc;
            }

            let progress = DownloadProgress {
                id: id.clone(),
                percent,
                speed,
                eta,
                status: "downloading".to_string(),
                title: current_title.clone(),
                thumbnail: thumbnail.clone(),
                source: effective_source(&source, &url),
                playlist_index: current_index,
                playlist_count: total_count,
                filesize: None,
                resolution: None,
                format_ext: None,
                error_message: None,
                error_code: None,
                error_params: None,
                history_id: None,
                filepath: None,
                downloaded_size,
                elapsed_time,
            };
            app.emit("download-progress", progress).ok();
        }

        // Extract title from [download] messages
        // Handles both: "Destination: /path/file.mp4" and "/path/file.mp4 has already been downloaded"
        if line.contains("[download]")
            && (line.contains("Destination:") || line.contains("has already been downloaded"))
        {
            let path_sep = if line.contains('\\') { '\\' } else { '/' };
            if let Some(start) = line.rfind(path_sep) {
                let filename = &line[start + 1..];
                current_title = title_from_download_filename(current_title, filename);
            }
        }

        if let Some(filepath) = filepath_from_download_output(&line) {
            final_filepath = Some(filepath);
        }

        // Capture final filepath
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('[') && is_media_filepath(trimmed) {
            final_filepath = Some(trimmed.to_string());
        }

        // Capture audio filepath from [ExtractAudio] Destination lines
        // e.g. "[ExtractAudio] Destination: C:\Users\...\song.mp3"
        if line.contains("[ExtractAudio]") && line.contains("Destination:") {
            if let Some(pos) = line.find("Destination:") {
                let path = line[pos + "Destination:".len()..].trim();
                if !path.is_empty() {
                    final_filepath = Some(path.to_string());
                }
            }
        }

        // Parse filesize
        if line.contains(" of ") && (line.contains("MiB") || line.contains("GiB")) {
            if let Some(re) = regex::Regex::new(r"of\s+(\d+(?:\.\d+)?)\s*(GiB|MiB|KiB)").ok() {
                if let Some(caps) = re.captures(&line) {
                    if let (Some(num), Some(unit)) = (caps.get(1), caps.get(2)) {
                        if let Ok(size) = num.as_str().parse::<f64>() {
                            let size_bytes = match unit.as_str() {
                                "GiB" => (size * 1024.0 * 1024.0 * 1024.0) as u64,
                                "MiB" => (size * 1024.0 * 1024.0) as u64,
                                "KiB" => (size * 1024.0) as u64,
                                _ => size as u64,
                            };
                            if current_stream_size != Some(size_bytes) {
                                if let Some(prev_size) = current_stream_size {
                                    total_filesize += prev_size;
                                }
                                current_stream_size = Some(size_bytes);
                            }
                        }
                    }
                }
            }
        }
    }

    // Wait for stderr task to finish reading all lines.
    if let Some(task) = stderr_task {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), task).await;
    }

    // Wait for process to fully exit before reading the temp file.
    // yt-dlp writes --print-to-file after_move:filepath near process exit;
    // reading before wait() can race and miss the path.
    let status = match process.wait().await {
        Ok(status) => status,
        Err(error) => {
            if emit_failed_workflow {
                enqueue_failed_workflow(
                    &app,
                    &failed_workflow_steps,
                    &id,
                    effective_source(&source, &url),
                    &output_directory,
                    Some(format.clone()),
                    quality_display.clone().or_else(|| Some(quality.clone())),
                    &url,
                    current_title.clone(),
                    thumbnail.clone(),
                    history_id.clone(),
                    extract_time_range(&download_sections),
                    &download_kind,
                );
            }
            return Err(
                BackendError::from_message(format!("Process error: {}", error)).to_wire_string(),
            );
        }
    };

    // Primary filepath source: read from the --print-to-file temp file (UTF-8).
    // This is reliable on all platforms, especially Windows with non-UTF-8 locales
    // where stdout encoding (GBK) corrupts Unicode characters in file paths.
    if let Ok(contents) = std::fs::read_to_string(&filepath_tmp) {
        printed_filepaths = parse_printed_filepaths(&contents);
        if let Some(path) = printed_filepaths.first() {
            final_filepath = Some(path.clone());
        }
    }
    // Clean up the temp file
    std::fs::remove_file(&filepath_tmp).ok();

    // Fallback: if the temp file didn't yield a filepath, try stdout/stderr captures
    if final_filepath.is_none() {
        if let Ok(guard) = stderr_filepath.lock() {
            if guard.is_some() {
                final_filepath = guard.clone();
            }
        }
    }

    if status.success() {
        let recent_lines = recent_output_snapshot(&recent_output);
        let chapter_filepaths = parse_split_chapter_filepaths(recent_lines.iter());
        let final_path_exists = final_filepath
            .as_ref()
            .map(|path| Path::new(path).is_file())
            .unwrap_or(false);
        if !final_path_exists {
            final_filepath = newest_media_filepath_in_dir(&output_directory);
        }
        let output_paths =
            output_filepaths(&printed_filepaths, &chapter_filepaths, &final_filepath);
        let actual_filesize = output_paths
            .first()
            .and_then(|fp| std::fs::metadata(fp).ok())
            .map(|m| m.len());

        let reported_filesize = actual_filesize.or_else(|| {
            if let Some(last_size) = current_stream_size {
                Some(total_filesize + last_size)
            } else if total_filesize > 0 {
                Some(total_filesize)
            } else {
                None
            }
        });

        let display_title =
            display_title_for_download(metadata_title, current_title, &final_filepath, total_count);
        let output_collection_names = build_output_collection_names(
            &auto_collection_names,
            auto_organize_collections_enabled,
            split_embedded_chapters && output_paths.len() > 1,
            display_title.as_deref(),
        );
        let completed_thumbnail =
            resolve_completed_thumbnail(&app, &url, thumbnail.clone(), &output_paths).await;

        let success_msg = format!(
            "Downloaded: {}",
            display_title
                .clone()
                .unwrap_or_else(|| "Unknown".to_string())
        );
        let details = format!(
            "Size: {} · Quality: {} · Format: {}",
            reported_filesize
                .map(format_size)
                .unwrap_or_else(|| "Unknown".to_string()),
            quality_display.clone().unwrap_or_else(|| quality.clone()),
            format.clone()
        );
        add_log_internal("success", &success_msg, Some(&details), Some(&url)).ok();

        let progress_history_id = save_download_outputs_to_history(
            &output_paths,
            history_id.as_ref(),
            &url,
            display_title.clone(),
            completed_thumbnail.clone(),
            source.clone(),
            quality_display.clone(),
            &format,
            reported_filesize,
            &download_sections,
            &output_collection_names,
        );

        let progress = DownloadProgress {
            id: id.clone(),
            percent: 100.0,
            speed: String::new(),
            eta: String::new(),
            status: "finished".to_string(),
            title: display_title.clone(),
            thumbnail: completed_thumbnail.clone(),
            source: effective_source(&source, &url),
            playlist_index: current_index,
            playlist_count: total_count,
            filesize: reported_filesize,
            resolution: quality_display.clone(),
            format_ext: Some(format.clone()),
            error_message: None,
            error_code: None,
            error_params: None,
            history_id: progress_history_id.clone(),
            filepath: final_filepath.clone(),
            downloaded_size: None,
            elapsed_time: None,
        };
        app.emit("download-progress", progress).ok();
        for (index, filepath) in output_paths.iter().enumerate() {
            let file_filesize = std::fs::metadata(filepath)
                .ok()
                .map(|m| m.len())
                .or(if index == 0 { reported_filesize } else { None });
            let plugin_title = if index == 0 {
                display_title.clone()
            } else {
                title_from_filepath(filepath).or_else(|| display_title.clone())
            };
            run_completed_plugins(
                &app,
                &completed_workflow_steps,
                &id,
                effective_source(&source, &url),
                filepath,
                file_filesize,
                Some(format.clone()),
                quality_display.clone().or_else(|| Some(quality.clone())),
                &url,
                plugin_title,
                completed_thumbnail.clone(),
                if index == 0 {
                    progress_history_id.clone()
                } else {
                    None
                },
                extract_time_range(&download_sections),
                &download_kind,
            )
            .await;
        }
        Ok(())
    } else {
        let recent_lines = recent_output_snapshot(&recent_output);
        if let Some(fallback) = core_fallback.as_ref() {
            if should_core_fallback_facebook_reel(&url, &recent_lines) {
                let fallback_args = facebook_reel_core_fallback_args(
                    &fallback.args,
                    &url,
                    &output_directory,
                    &item_prefix,
                    current_title.as_deref(),
                    collision_suffix.as_deref(),
                );
                add_log_internal(
                    "info",
                    "Retrying Facebook Reel in core without cookies",
                    None,
                    Some(&url),
                )
                .ok();
                add_log_internal(
                    "command",
                    &format!(
                        "[{}] yt-dlp {}",
                        fallback.binary_label,
                        fallback_args.join(" ")
                    ),
                    None,
                    Some(&url),
                )
                .ok();
                std::fs::remove_file(&filepath_tmp).ok();
                let fallback_metadata = if current_title.is_some() && thumbnail.is_some() {
                    CoreFallbackMetadata::default()
                } else {
                    probe_core_facebook_reel_metadata(fallback, &url)
                        .await
                        .unwrap_or_default()
                };
                let fallback_title = fallback_metadata.title.or_else(|| current_title.clone());
                let fallback_thumbnail = fallback_metadata.thumbnail.or_else(|| thumbnail.clone());

                match spawn_core_download(fallback, &fallback_args) {
                    Ok(process) => {
                        return Box::pin(handle_tokio_download(
                            app,
                            id,
                            process,
                            quality,
                            format,
                            url,
                            should_log_stderr,
                            fallback_title,
                            fallback_thumbnail,
                            source,
                            download_sections,
                            history_id,
                            filepath_tmp,
                            output_directory,
                            completed_workflow_steps,
                            failed_workflow_steps,
                            emit_failed_workflow,
                            download_kind,
                            item_prefix,
                            auto_collection_names.clone(),
                            auto_organize_collections_enabled,
                            split_embedded_chapters,
                            collision_suffix,
                            None,
                        ))
                        .await;
                    }
                    Err(error) => {
                        add_log_internal("error", &error, None, Some(&url)).ok();
                    }
                }
            }
        }

        if CANCEL_FLAG.load(Ordering::SeqCst) {
            let error = download_cancelled_error();
            add_log_internal("info", error.message(), None, Some(&url)).ok();
            return Err(error.to_wire_string());
        }

        let error = build_download_error_message(status.code(), &recent_lines);
        add_log_internal("error", error.message(), None, Some(&url)).ok();

        // Emit error progress so frontend can display error message
        let progress = DownloadProgress {
            id: id.clone(),
            percent: 0.0,
            speed: String::new(),
            eta: String::new(),
            status: "error".to_string(),
            title: current_title.clone(),
            thumbnail: thumbnail.clone(),
            source: effective_source(&source, &url),
            playlist_index: current_index,
            playlist_count: total_count,
            filesize: None,
            resolution: None,
            format_ext: None,
            error_message: Some(error.message().to_string()),
            error_code: Some(error.code().to_string()),
            error_params: error.params().cloned(),
            history_id: None,
            filepath: None,
            downloaded_size: None,
            elapsed_time: None,
        };
        app.emit("download-progress", progress).ok();

        if emit_failed_workflow && !failed_workflow_steps.is_empty() {
            let payload = build_trigger_payload(
                &id,
                effective_source(&source, &url),
                "download.failed",
                &output_directory,
                None,
                Some(format.clone()),
                quality_display.clone().or_else(|| Some(quality.clone())),
                &url,
                current_title.clone(),
                thumbnail.clone(),
                history_id.clone(),
                extract_time_range(&download_sections),
                &download_kind,
            );
            let _ = enqueue_post_download_workflow(&app, failed_workflow_steps, payload);
        }

        Err(error.to_wire_string())
    }
}

#[tauri::command]
pub async fn stop_download() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    kill_all_download_processes();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    kill_all_download_processes();
    Ok(())
}

fn detect_source(url: &str) -> Option<String> {
    if url.contains("youtube.com") || url.contains("youtu.be") {
        Some("youtube".to_string())
    } else if url.contains("tiktok.com") {
        Some("tiktok".to_string())
    } else if url.contains("facebook.com") || url.contains("fb.watch") {
        Some("facebook".to_string())
    } else if url.contains("instagram.com") {
        Some("instagram".to_string())
    } else if url.contains("twitter.com") || url.contains("x.com") {
        Some("twitter".to_string())
    } else if url.contains("bilibili.com") || url.contains("b23.tv") {
        Some("bilibili".to_string())
    } else if url.contains("youku.com") {
        Some("youku".to_string())
    } else {
        Some("other".to_string())
    }
}

fn effective_source(source: &Option<String>, url: &str) -> Option<String> {
    source
        .as_deref()
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && !value.eq_ignore_ascii_case("direct")
                && !value.eq_ignore_ascii_case("other")
        })
        .map(str::to_string)
        .or_else(|| detect_source(url))
}

fn generate_thumbnail_url(url: &str) -> Option<String> {
    if url.contains("youtube.com") || url.contains("youtu.be") {
        let video_id = if url.contains("v=") {
            url.split("v=").nth(1).and_then(|s| s.split('&').next())
        } else if url.contains("youtu.be/") {
            url.split("youtu.be/")
                .nth(1)
                .and_then(|s| s.split('?').next())
        } else {
            None
        };
        video_id.map(|id| format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", id))
    } else {
        None
    }
}

async fn resolve_completed_thumbnail(
    app: &AppHandle,
    url: &str,
    thumbnail: Option<String>,
    output_paths: &[String],
) -> Option<String> {
    if thumbnail
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return thumbnail;
    }

    if is_facebook_reel_url(url) {
        if let Some(filepath) = output_paths.iter().find(|path| Path::new(path).is_file()) {
            match super::generate_video_thumbnail(app.clone(), filepath.clone()).await {
                Ok(generated) => return Some(generated),
                Err(error) => {
                    add_log_internal(
                        "info",
                        "Facebook Reel thumbnail fallback unavailable",
                        Some(&error),
                        Some(url),
                    )
                    .ok();
                }
            }
        }
    }

    generate_thumbnail_url(url)
}

fn thumbnail_file_extension(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/avif" => Some("avif"),
        _ => None,
    }
}

fn is_thumbnail_cache_file_name(file_name: &str) -> bool {
    let Some((stem, extension)) = file_name.rsplit_once('.') else {
        return false;
    };
    stem.len() == 16
        && stem.chars().all(|c| c.is_ascii_hexdigit())
        && matches!(
            extension.to_ascii_lowercase().as_str(),
            "jpg" | "png" | "webp" | "gif" | "avif"
        )
}

fn should_delete_thumbnail_cache_file(
    file_name: &str,
    modified_at: SystemTime,
    now: SystemTime,
    referenced_files: &HashSet<String>,
) -> bool {
    is_thumbnail_cache_file_name(file_name)
        && !referenced_files.contains(file_name)
        && matches!(
            now.duration_since(modified_at),
            Ok(age) if age >= REMOTE_THUMBNAIL_CACHE_MAX_AGE
        )
}

fn collect_thumbnail_reference_values() -> Result<Vec<String>, String> {
    let conn = crate::database::get_db()?;
    let mut values = Vec::new();
    for sql in [
        "SELECT thumbnail FROM history WHERE thumbnail IS NOT NULL",
        "SELECT thumbnail FROM followed_channels WHERE thumbnail IS NOT NULL",
        "SELECT thumbnail FROM channel_videos WHERE thumbnail IS NOT NULL",
        "SELECT items_json FROM download_queues WHERE items_json IS NOT NULL",
    ] {
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("Failed to read thumbnail references: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("Failed to read thumbnail references: {e}"))?;
        for row in rows {
            values.push(row.map_err(|e| format!("Failed to read thumbnail reference: {e}"))?);
        }
    }
    Ok(values)
}

fn referenced_thumbnail_cache_files(file_names: &[String]) -> Result<HashSet<String>, String> {
    let reference_values = collect_thumbnail_reference_values()?;
    Ok(file_names
        .iter()
        .filter(|file_name| {
            reference_values
                .iter()
                .any(|value| value.contains(*file_name))
        })
        .cloned()
        .collect())
}

fn cleanup_stale_thumbnail_cache(
    thumbnail_dir: &Path,
    now: SystemTime,
    protected_files: &[String],
) -> Result<usize, String> {
    if !thumbnail_dir.exists() {
        return Ok(0);
    }

    let mut cache_files = Vec::new();
    for entry in std::fs::read_dir(thumbnail_dir)
        .map_err(|e| format!("Failed to read thumbnail cache: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read thumbnail cache entry: {e}"))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !is_thumbnail_cache_file_name(&file_name) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read thumbnail cache metadata: {e}"))?;
        if !metadata.is_file() {
            continue;
        }
        let modified_at = metadata.modified().unwrap_or(UNIX_EPOCH);
        cache_files.push((entry.path(), file_name, modified_at));
    }

    let file_names: Vec<_> = cache_files
        .iter()
        .map(|(_, file_name, _)| file_name.clone())
        .collect();
    let mut referenced_files = referenced_thumbnail_cache_files(&file_names)?;
    referenced_files.extend(protected_files.iter().cloned());
    let mut removed = 0;
    for (path, file_name, modified_at) in cache_files {
        if !should_delete_thumbnail_cache_file(&file_name, modified_at, now, &referenced_files) {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Failed to remove stale thumbnail cache: {e}")),
        }
    }
    Ok(removed)
}

#[tauri::command]
pub async fn cache_remote_thumbnail(app: AppHandle, url: String) -> Result<String, String> {
    let parsed_url = reqwest::Url::parse(&url).map_err(|_| "Invalid thumbnail URL".to_string())?;
    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err("Thumbnail URL must use HTTP or HTTPS".to_string());
    }

    let client = reqwest::Client::builder()
        .user_agent("Youwee/0.17.2")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Failed to create thumbnail client: {e}"))?;
    let response = client
        .get(parsed_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download thumbnail: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Thumbnail request failed: {e}"))?;

    if response
        .content_length()
        .is_some_and(|length| length > REMOTE_THUMBNAIL_MAX_BYTES as u64)
    {
        return Err("Thumbnail is too large".to_string());
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let extension = thumbnail_file_extension(content_type)
        .ok_or_else(|| "Thumbnail response is not a supported image".to_string())?;

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Failed to read thumbnail: {e}"))?;
        if bytes.len() + chunk.len() > REMOTE_THUMBNAIL_MAX_BYTES {
            return Err("Thumbnail is too large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let thumbnail_dir = app_data_dir.join("thumbnails");
    tokio::fs::create_dir_all(&thumbnail_dir)
        .await
        .map_err(|e| format!("Failed to create thumbnail cache: {e}"))?;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let cache_path = thumbnail_dir.join(format!("{:016x}.{extension}", hasher.finish()));
    if !cache_path.exists() {
        tokio::fs::write(&cache_path, bytes)
            .await
            .map_err(|e| format!("Failed to cache thumbnail: {e}"))?;
    }

    let protected_file = cache_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .into_iter()
        .collect::<Vec<_>>();
    match cleanup_stale_thumbnail_cache(&thumbnail_dir, SystemTime::now(), &protected_file) {
        Ok(removed) if removed > 0 => log::info!("Removed {removed} stale thumbnail cache files"),
        Ok(_) => {}
        Err(e) => log::warn!("Failed to clean thumbnail cache: {e}"),
    }

    Ok(cache_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_template_uses_safe_custom_filename_template() {
        assert_eq!(
            build_output_template(Some("%(uploader)s - %(title)s".to_string()), ""),
            "%(uploader)s - %(title)s.%(ext)s"
        );
        assert_eq!(
            build_output_template(Some("../escape/%(title)s.%(ext)s".to_string()), ""),
            "%(title)s.%(ext)s"
        );
        assert_eq!(
            build_output_template(Some("  ".to_string()), ""),
            "%(title)s.%(ext)s"
        );
        assert_eq!(output_path_arg("C:/Downloads/"), "home:C:/Downloads");
        assert_eq!(output_path_arg(r"C:\Downloads\"), r"home:C:\Downloads");
    }

    #[test]
    fn explicit_duplicate_downloads_get_a_unique_output_suffix() {
        let suffix = output_collision_suffix(Some("unique"), "12345678-90ab").unwrap();
        assert_eq!(suffix.as_deref(), Some("copy-12345678"));
        assert_eq!(
            apply_output_collision_suffix("%(title)s.%(ext)s".to_string(), suffix.as_deref()),
            "%(title)s [copy-12345678].%(ext)s"
        );
        assert_eq!(
            output_collision_suffix(Some("overwrite"), "job").unwrap(),
            None
        );
        assert!(output_collision_suffix(Some("invalid"), "job").is_err());
    }

    #[test]
    fn output_template_numbers_playlist_and_regular_queue_items() {
        let expanded_playlist_prefix =
            build_item_prefix(true, false, Some(3), Some(120), true, Some(7), Some(42));
        assert_eq!(expanded_playlist_prefix, "003 - ");

        let direct_playlist_prefix =
            build_item_prefix(true, true, None, Some(120), true, Some(7), Some(42));
        assert_eq!(direct_playlist_prefix, "%(playlist_index)03d - ");

        let regular_queue_prefix =
            build_item_prefix(true, false, None, None, true, Some(7), Some(42));
        assert_eq!(regular_queue_prefix, "07 - ");
        assert_eq!(
            build_output_template(Some("%(title)s.%(ext)s".to_string()), &regular_queue_prefix,),
            "07 - %(title)s.%(ext)s"
        );
    }

    #[test]
    fn windows_output_template_limits_plain_titles_for_intermediate_files() {
        let output_directory = r"\\?\C:\Users\Administrator\Downloads";
        let template = constrain_title_template_for_output(
            build_output_template(Some(DEFAULT_FILENAME_TEMPLATE.to_string()), ""),
            output_directory,
        );
        let explicit_precision = constrain_title_template_for_output(
            "%(title).120B.%(ext)s".to_string(),
            output_directory,
        );

        if cfg!(target_os = "windows") {
            assert_eq!(template, "%(title).180B.%(ext)s");
        } else {
            assert_eq!(template, DEFAULT_FILENAME_TEMPLATE);
        }
        assert_eq!(explicit_precision, "%(title).120B.%(ext)s");
    }

    #[test]
    fn auto_collection_names_are_default_off() {
        assert!(build_auto_collection_names(false, Some("Playlist")).is_empty());
    }

    #[test]
    fn auto_collection_names_trim_empty_playlist_names() {
        assert!(build_auto_collection_names(true, Some("   ")).is_empty());
        assert_eq!(
            build_auto_collection_names(true, Some("  Playlist  ")),
            vec!["Playlist".to_string()]
        );
    }

    #[test]
    fn chapter_template_uses_item_and_chapter_numbers() {
        assert_eq!(
            build_chapter_output_template("03 - ", true),
            "03 - %(section_number)02d - %(section_title)s.%(ext)s"
        );
        assert_eq!(
            build_chapter_output_template("", false),
            "%(section_title)s.%(ext)s"
        );
    }

    #[test]
    fn printed_filepaths_preserve_multiple_outputs() {
        let filepaths = parse_printed_filepaths(
            "C:/Downloads/Video.mp4\nC:/Downloads/01 - Intro.mp4\nC:/Downloads/01 - Intro.mp4\n",
        );

        assert_eq!(
            filepaths,
            vec![
                "C:/Downloads/Video.mp4".to_string(),
                "C:/Downloads/01 - Intro.mp4".to_string()
            ]
        );
    }

    #[test]
    fn download_output_parser_finds_media_filepaths() {
        assert_eq!(
            filepath_from_download_output(
                r#"[Merger] Merging formats into "\\?\C:\Downloads\Long Folder\Video.mp4""#
            )
            .as_deref(),
            Some(r"\\?\C:\Downloads\Long Folder\Video.mp4")
        );
        assert_eq!(
            filepath_from_download_output(
                r"[download] Destination: \\?\C:\Downloads\Long Folder\Video.mp4"
            )
            .as_deref(),
            Some(r"\\?\C:\Downloads\Long Folder\Video.mp4")
        );
        assert_eq!(
            filepath_from_download_output(
                r"[download] Destination: \\?\C:\Downloads\Long Folder\Video.jpg"
            ),
            None
        );
    }

    #[test]
    fn newest_media_filepath_scans_output_directory() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("youwee-output-scan-{nonce}"));
        std::fs::create_dir_all(&root).expect("test output directory should be created");
        let ignored = root.join("thumbnail.jpg");
        let media = root.join("video.mp4");
        std::fs::write(&ignored, b"thumbnail").expect("thumbnail should be written");
        std::fs::write(&media, b"video").expect("media should be written");

        assert_eq!(
            newest_media_filepath_in_dir(&root.to_string_lossy()).as_deref(),
            Some(media.to_string_lossy().as_ref())
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn split_chapter_filepaths_are_parsed_from_ytdlp_output() {
        let lines = vec![
            "[SplitChapters] Chapter 001; Destination: C:/Downloads/01 - Intro.mp4".to_string(),
            "[SplitChapters] Chapter 002; Destination: C:/Downloads/02 - Setup.mp4".to_string(),
            "[info] Writing '%(filepath)s' to: C:/Downloads/paths.txt".to_string(),
        ];

        assert_eq!(
            parse_split_chapter_filepaths(lines.iter()),
            vec![
                "C:/Downloads/01 - Intro.mp4".to_string(),
                "C:/Downloads/02 - Setup.mp4".to_string()
            ]
        );
    }

    #[test]
    fn output_filepaths_merge_main_and_split_chapters() {
        assert_eq!(
            output_filepaths(
                &["C:/Downloads/Video.mp4".to_string()],
                &[
                    "C:/Downloads/01 - Intro.mp4".to_string(),
                    "C:/Downloads/02 - Setup.mp4".to_string()
                ],
                &None,
            ),
            vec![
                "C:/Downloads/Video.mp4".to_string(),
                "C:/Downloads/01 - Intro.mp4".to_string(),
                "C:/Downloads/02 - Setup.mp4".to_string()
            ]
        );
    }

    #[test]
    fn split_collection_names_add_parent_when_enabled() {
        assert_eq!(
            build_output_collection_names(&[], true, true, Some("Video")),
            vec!["Video".to_string()]
        );
        assert_eq!(
            build_output_collection_names(&["Playlist".to_string()], true, true, Some("playlist")),
            vec!["Playlist".to_string()]
        );
        assert!(build_output_collection_names(&[], false, true, Some("Video")).is_empty());
    }

    #[test]
    fn output_directory_can_be_grouped_by_source() {
        assert_eq!(
            build_output_directory(
                "C:/Downloads",
                "https://www.youtube.com/watch?v=abc",
                Some(true)
            ),
            "C:/Downloads/youtube"
        );
        assert_eq!(
            build_output_directory("C:/Downloads/", "https://fb.watch/demo", Some(true)),
            "C:/Downloads/facebook"
        );
        assert_eq!(
            build_output_directory("C:/Downloads", "https://example.com/video", Some(true)),
            "C:/Downloads/other"
        );
        assert_eq!(
            build_output_directory("C:/Downloads", "https://www.youtube.com/watch?v=abc", None),
            "C:/Downloads"
        );
    }

    #[test]
    fn overwrite_args_preserve_default_and_allow_skip_existing() {
        let mut default_args = Vec::new();
        push_overwrite_args(&mut default_args, None);
        assert_eq!(default_args, vec!["--force-overwrites"]);

        let mut skip_args = Vec::new();
        push_overwrite_args(&mut skip_args, Some(true));
        assert_eq!(skip_args, vec!["--no-overwrites"]);
    }

    #[test]
    fn process_exit_errors_include_exit_code_param() {
        let error = build_download_error_message(Some(15), &["ERROR: interrupted".to_string()]);
        let wire = error.to_wire();

        assert_eq!(wire.code, crate::types::code::PROCESS_EXIT_NON_ZERO);
        assert_eq!(
            wire.params
                .and_then(|params| params.get("exitCode").cloned()),
            Some(serde_json::Value::from(15))
        );
    }

    #[test]
    fn cancelled_download_error_is_non_retryable() {
        let wire = download_cancelled_error().to_wire();

        assert_eq!(wire.code, crate::types::code::DOWNLOAD_CANCELLED);
        assert_eq!(wire.message, "Download cancelled");
        assert_eq!(wire.retryable, Some(false));
    }

    #[test]
    fn filename_safety_args_follow_platform() {
        let mut args = Vec::new();
        push_filename_safety_args(&mut args, None);

        if cfg!(target_os = "windows") {
            assert_eq!(args, vec!["--windows-filenames", "--trim-filenames", "180"]);
        } else {
            assert_eq!(args, vec!["--trim-filenames", "180"]);
        }
    }

    #[test]
    fn youtube_extractor_args_merge_player_client_and_actual_js() {
        assert_eq!(
            build_youtube_extractor_args(true, Some("web_safari")).as_deref(),
            Some("youtube:player_client=web_safari,player_js_version=actual")
        );
        assert_eq!(
            build_youtube_extractor_args(false, Some("web_safari")).as_deref(),
            Some("youtube:player_client=web_safari")
        );
        assert_eq!(
            build_youtube_extractor_args(true, Some("auto")).as_deref(),
            Some("youtube:player_js_version=actual")
        );
        assert_eq!(build_youtube_extractor_args(false, Some("bad")), None);
    }

    #[test]
    fn facebook_reel_core_fallback_is_scoped_to_parse_failures() {
        let lines = vec!["ERROR: [facebook] 1889836315019111: Cannot parse data".to_string()];

        assert!(should_core_fallback_facebook_reel(
            "https://www.facebook.com/reel/1889836315019111",
            &lines
        ));
        assert!(!should_core_fallback_facebook_reel(
            "https://www.facebook.com/watch?v=1889836315019111",
            &lines
        ));
        assert!(!should_core_fallback_facebook_reel(
            "https://www.facebook.com/reelsomething/1889836315019111",
            &lines
        ));
        assert!(!should_core_fallback_facebook_reel(
            "https://www.youtube.com/watch?v=abc",
            &lines
        ));
        assert!(!should_core_fallback_facebook_reel(
            "https://www.tiktok.com/@sample/video/123",
            &lines
        ));
        assert!(!should_core_fallback_facebook_reel(
            "https://www.instagram.com/reel/ABC123/",
            &lines
        ));
    }

    #[test]
    fn facebook_reel_core_fallback_args_remove_cookies_and_aria2() {
        let args = vec![
            "--newline",
            "--cookies-from-browser",
            "firefox",
            "--downloader",
            "aria2c",
            "--downloader-args",
            "aria2c:-x 16",
            "--paths",
            "home:C:/Old",
            "-o",
            "%(title)s.%(ext)s",
            "--",
            "https://www.facebook.com/reel/1889836315019111",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

        let fallback_args = facebook_reel_core_fallback_args(
            &args,
            "https://www.facebook.com/reel/1889836315019111",
            "C:/Downloads",
            "02 - ",
            None,
            None,
        );

        assert!(!fallback_args
            .iter()
            .any(|arg| arg == "--cookies-from-browser"));
        assert!(!fallback_args.iter().any(|arg| arg == "firefox"));
        assert!(!fallback_args.iter().any(|arg| arg == "aria2c:-x 16"));
        assert_eq!(
            fallback_args
                .windows(2)
                .filter(|pair| pair[0] == "--downloader" && pair[1] == "native")
                .count(),
            1
        );
        assert_eq!(
            fallback_args
                .windows(2)
                .filter(|pair| pair[0] == "--impersonate" && pair[1] == "chrome")
                .count(),
            1
        );
        let separator = fallback_args.iter().position(|arg| arg == "--").unwrap();
        let impersonate = fallback_args
            .iter()
            .position(|arg| arg == "--impersonate")
            .unwrap();
        assert!(impersonate < separator);
        let output = fallback_args
            .windows(2)
            .find(|pair| pair[0] == "-o")
            .map(|pair| pair[1].as_str())
            .unwrap();
        assert_eq!(output, "02 - facebook-com-reel-1889836315019111.%(ext)s");
        let output_path = fallback_args
            .windows(2)
            .find(|pair| pair[0] == "--paths")
            .map(|pair| pair[1].as_str())
            .unwrap();
        assert_eq!(output_path, "home:C:/Downloads");
    }

    #[test]
    fn facebook_reel_recovered_title_is_reused_for_metadata_and_core_output() {
        let mut args = vec![
            "-o".to_string(),
            "%(title)s.%(ext)s".to_string(),
            "--".to_string(),
            "https://www.facebook.com/reel/123".to_string(),
        ];
        push_facebook_title_replacement_args(
            &mut args,
            "https://www.facebook.com/reel/123",
            Some("Tiêu đề thật $5"),
        );
        let replacement = args
            .windows(4)
            .find(|window| window[0] == "--replace-in-metadata")
            .expect("metadata replacement args");
        assert_eq!(replacement[1], "title");
        let title_pattern = regex::Regex::new(&replacement[2]).expect("valid title pattern");
        assert!(title_pattern.is_match("Video"));
        assert!(title_pattern
            .is_match("4 comments | Tool Text-to-Speech ElevenLabs cho ae làm content"));
        assert!(!title_pattern.is_match("Một tiêu đề thật"));
        assert_eq!(replacement[3], "Tiêu đề thật \\$5");

        let fallback_args = facebook_reel_core_fallback_args(
            &args,
            "https://www.facebook.com/reel/123",
            "C:/Downloads",
            "",
            Some("Tiêu đề thật $5"),
            Some("copy-12345678"),
        );
        let output = fallback_args
            .windows(2)
            .find(|pair| pair[0] == "-o")
            .map(|pair| pair[1].as_str())
            .unwrap();
        assert_eq!(output, "%(title)s.%(ext)s");
    }

    #[test]
    fn core_fallback_metadata_prefers_title_and_best_thumbnail() {
        let json = serde_json::json!({
            "title": "Public Reel Title",
            "thumbnails": [
                { "url": "http://example.com/small.jpg" },
                { "url": "https://example.com/large.jpg" }
            ]
        });

        let metadata = metadata_from_info_json(&json);

        assert_eq!(metadata.title.as_deref(), Some("Public Reel Title"));
        assert_eq!(
            metadata.thumbnail.as_deref(),
            Some("https://example.com/large.jpg")
        );
    }

    #[test]
    fn core_fallback_metadata_normalizes_top_level_thumbnail() {
        let json = serde_json::json!({
            "title": "Public Reel Title",
            "thumbnail": "http://example.com/thumb.jpg"
        });

        let metadata = metadata_from_info_json(&json);

        assert_eq!(
            metadata.thumbnail.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
    }

    #[test]
    fn core_fallback_metadata_does_not_replace_recovered_title_with_placeholder() {
        let json = serde_json::json!({
            "title": "Video",
            "thumbnail": "https://example.com/thumb.jpg"
        });

        let metadata = metadata_from_info_json(&json);

        assert_eq!(metadata.title, None);
        assert_eq!(
            metadata.thumbnail.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
    }

    #[test]
    fn effective_source_normalizes_placeholder_sources_for_known_platforms() {
        for (url, source) in [
            ("https://www.facebook.com/reel/1889836315019111", "facebook"),
            ("https://www.tiktok.com/@sample/video/123", "tiktok"),
            ("https://www.instagram.com/reel/ABC123/", "instagram"),
        ] {
            for placeholder in ["direct", "other"] {
                assert_eq!(
                    effective_source(&Some(placeholder.to_string()), url).as_deref(),
                    Some(source),
                    "{placeholder} should normalize {url}",
                );
            }
        }
    }

    #[test]
    fn recognizes_supported_thumbnail_content_types() {
        assert_eq!(thumbnail_file_extension("image/jpeg"), Some("jpg"));
        assert_eq!(
            thumbnail_file_extension("image/webp; charset=binary"),
            Some("webp")
        );
        assert_eq!(thumbnail_file_extension("text/html"), None);
    }

    #[test]
    fn stale_thumbnail_cleanup_only_targets_old_unreferenced_cache_files() {
        let now = UNIX_EPOCH + REMOTE_THUMBNAIL_CACHE_MAX_AGE + Duration::from_secs(1);
        let old = UNIX_EPOCH;
        let recent = now - Duration::from_secs(60);
        let referenced_files = HashSet::from(["1111111111111111.jpg".to_string()]);

        assert!(!should_delete_thumbnail_cache_file(
            "1111111111111111.jpg",
            old,
            now,
            &referenced_files
        ));
        assert!(!should_delete_thumbnail_cache_file(
            "2222222222222222.jpg",
            recent,
            now,
            &referenced_files
        ));
        assert!(should_delete_thumbnail_cache_file(
            "2222222222222222.jpg",
            old,
            now,
            &referenced_files
        ));
        assert!(!should_delete_thumbnail_cache_file(
            "notes.txt",
            old,
            now,
            &referenced_files
        ));
    }

    #[test]
    fn detects_facebook_reel_fallback_stem() {
        assert!(is_facebook_reel_fallback_stem(
            "facebook-com-reel-1889836315019111.f1428297389333728a"
        ));
        assert!(!is_facebook_reel_fallback_stem("Video"));
    }

    #[test]
    fn download_filename_does_not_overwrite_recovered_facebook_reel_title() {
        let title = title_from_download_filename(
            Some("Recovered Facebook title".to_string()),
            "facebook-com-reel-1889836315019111.f1428297389333728a.mp4",
        );

        assert_eq!(title.as_deref(), Some("Recovered Facebook title"));
    }

    #[test]
    fn download_filename_still_sets_normal_titles() {
        let title = title_from_download_filename(None, "Video.mp4");

        assert_eq!(title.as_deref(), Some("Video"));
    }

    #[test]
    fn single_download_display_title_keeps_unicode_metadata_title() {
        let title = display_title_for_download(
            Some("Machiot - Đổi Nhạc Nền Vol. 2".to_string()),
            Some("Machiot - i Nhc Nn Vol. 2".to_string()),
            &None,
            None,
        );

        assert_eq!(title.as_deref(), Some("Machiot - Đổi Nhạc Nền Vol. 2"));
    }

    #[test]
    fn single_download_display_title_prefers_utf8_filepath_over_stdout_title() {
        let filepath = Some("Machiot - Đổi Nhạc Nền Vol. 2.mp4".to_string());
        let title = display_title_for_download(
            None,
            Some("Machiot - i Nhc Nn Vol. 2".to_string()),
            &filepath,
            None,
        );

        assert_eq!(title.as_deref(), Some("Machiot - Đổi Nhạc Nền Vol. 2"));
    }

    #[test]
    fn playlist_display_title_keeps_current_item_title() {
        let filepath = Some("Playlist name.mp4".to_string());
        let title = display_title_for_download(
            Some("Playlist name".to_string()),
            Some("Video item title".to_string()),
            &filepath,
            Some(3),
        );

        assert_eq!(title.as_deref(), Some("Video item title"));
    }
}
