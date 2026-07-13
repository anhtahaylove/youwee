use crate::database::{
    TikTokLiveJob, TikTokLiveJobStatus, TikTokLiveRecordMode, TikTokLiveWatchEntry,
    TikTokLiveWatchStatus, add_log_internal, delete_tiktok_live_job_internal,
    delete_tiktok_live_watch_entry_internal, get_due_tiktok_live_watchlist_internal,
    get_tiktok_live_job_internal, get_tiktok_live_jobs_internal,
    get_tiktok_live_recorder_limit_internal, get_tiktok_live_watch_entry_by_active_job_internal,
    get_tiktok_live_watch_entry_by_target_internal, get_tiktok_live_watch_entry_internal,
    get_tiktok_live_watchlist_internal, save_tiktok_live_job_internal,
    save_tiktok_live_watch_entry_internal, set_tiktok_live_recorder_limit_internal,
    update_tiktok_live_watch_entry_internal, upsert_history_with_id_internal,
};
use crate::services::{
    get_ffmpeg_path, parse_ytdlp_error, run_ytdlp_json_with_cookies, should_skip_cookies_for_url,
};
use crate::types::{BackendError, code};
use crate::utils::{firefox_profiles_ini_path, resolve_firefox_profile_for_cookies};
use chrono::{Datelike, Local, Timelike, Utc};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    LazyLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep, timeout};

static ACTIVE_RECORDINGS: LazyLock<
    Mutex<HashMap<String, Option<tokio::sync::oneshot::Sender<()>>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
static TIKTOK_LIVE_WATCHLIST_ACTIVE: AtomicBool = AtomicBool::new(false);
static TIKTOK_LIVE_MAX_RECORDINGS: AtomicUsize = AtomicUsize::new(1);

const METADATA_FETCH_ATTEMPTS: u32 = 3;
const METADATA_RETRY_BASE_DELAY_MS: u64 = 750;
const RECONNECT_MAX_RETRIES: u32 = 20;
const RECONNECT_DELAY_MAX_SECONDS: u32 = 5;
const RECONNECT_DELAY_TOTAL_MAX_SECONDS: u32 = 120;
const STREAM_URL_REFRESH_ATTEMPTS: u32 = 3;
const RECORDING_SEGMENT_EXTENSION: &str = "mkv";
const WATCHLIST_LOOP_TICK_SECONDS: u64 = 10;
const WATCHLIST_MIN_POLL_SECONDS: u32 = 30;
const WATCHLIST_MAX_POLL_SECONDS: u32 = 3600;
const WATCHLIST_DEFAULT_COOLDOWN_SECONDS: u32 = 3600;
const WATCHLIST_MAX_COOLDOWN_SECONDS: u32 = 604_800;
const WATCHLIST_MAX_BACKOFF_SECONDS: u32 = 1800;
const WATCHLIST_PAUSED_CHECK_AT: i64 = 253_402_300_799;
const TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT: usize = 4;
const TIKTOK_LIVE_ALREADY_ACTIVE_MESSAGE: &str =
    "This TikTok Live recording job is already active.";
const TIKTOK_LIVE_ONE_ROOM_MESSAGE: &str =
    "The TikTok Live recorder is at its configured room limit.";

fn clamp_tiktok_live_recording_limit(value: Option<usize>) -> usize {
    value
        .unwrap_or(1)
        .clamp(1, TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT)
}

fn configured_tiktok_live_recording_limit() -> usize {
    clamp_tiktok_live_recording_limit(Some(TIKTOK_LIVE_MAX_RECORDINGS.load(Ordering::SeqCst)))
}

fn apply_tiktok_live_recording_limit(value: usize) -> usize {
    let limit = clamp_tiktok_live_recording_limit(Some(value));
    TIKTOK_LIVE_MAX_RECORDINGS.store(limit, Ordering::SeqCst);
    limit
}

pub fn load_tiktok_live_recorder_config_after_restart() -> Result<usize, String> {
    let limit = get_tiktok_live_recorder_limit_internal()?
        .map(apply_tiktok_live_recording_limit)
        .unwrap_or_else(configured_tiktok_live_recording_limit);
    Ok(limit)
}

fn clamp_watchlist_cooldown(seconds: Option<u32>) -> u32 {
    seconds
        .unwrap_or(WATCHLIST_DEFAULT_COOLDOWN_SECONDS)
        .min(WATCHLIST_MAX_COOLDOWN_SECONDS)
}

fn tiktok_live_resource_warning(
    active_recordings: usize,
    recording_limit: usize,
) -> Option<&'static str> {
    if active_recordings > 1 {
        Some("multiRoomActive")
    } else if recording_limit > 1 {
        Some("limitHigh")
    } else {
        None
    }
}

fn normalize_schedule_days(raw: Option<String>) -> Option<String> {
    let mut days: Vec<u32> = raw
        .unwrap_or_default()
        .split(',')
        .filter_map(|value| value.trim().parse::<u32>().ok())
        .filter(|day| *day < 7)
        .collect();
    days.sort_unstable();
    days.dedup();
    (!days.is_empty()).then(|| {
        days.into_iter()
            .map(|day| day.to_string())
            .collect::<Vec<_>>()
            .join(",")
    })
}

fn normalize_schedule_minute(value: Option<u32>) -> Option<u32> {
    value.filter(|minute| *minute < 24 * 60)
}

fn schedule_days_contains(schedule_days: Option<&str>, weekday: u32) -> bool {
    schedule_days
        .map(|days| {
            days.split(',')
                .filter_map(|value| value.trim().parse::<u32>().ok())
                .any(|day| day == weekday)
        })
        .unwrap_or(true)
}

fn schedule_window_contains(start: Option<u32>, end: Option<u32>, minute: u32) -> bool {
    match (start, end) {
        (Some(start), Some(end)) if start < end => (start..end).contains(&minute),
        (Some(start), Some(end)) if start > end => minute >= start || minute < end,
        (Some(start), None) => minute >= start,
        (None, Some(end)) => minute < end,
        _ => true,
    }
}

fn watch_entry_allows_auto_record_now(entry: &TikTokLiveWatchEntry) -> bool {
    if !entry.schedule_enabled {
        return true;
    }
    let now = Local::now();
    let weekday = now.weekday().num_days_from_monday();
    let minute = now.hour() * 60 + now.minute();
    schedule_days_contains(entry.schedule_days.as_deref(), weekday)
        && schedule_window_contains(
            entry.schedule_start_minute,
            entry.schedule_end_minute,
            minute,
        )
}

async fn tiktok_live_recorder_at_limit() -> bool {
    ACTIVE_RECORDINGS.lock().await.len() >= configured_tiktok_live_recording_limit()
}

async fn reserve_tiktok_live_recording(
    job_id: &str,
) -> Result<tokio::sync::oneshot::Receiver<()>, String> {
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let mut recordings = ACTIVE_RECORDINGS.lock().await;
    if recordings.contains_key(job_id) {
        return Err(
            BackendError::from_message(TIKTOK_LIVE_ALREADY_ACTIVE_MESSAGE).to_wire_string(),
        );
    }
    if recordings.len() >= configured_tiktok_live_recording_limit() {
        return Err(BackendError::from_message(TIKTOK_LIVE_ONE_ROOM_MESSAGE).to_wire_string());
    }
    recordings.insert(job_id.to_string(), Some(cancel_tx));
    Ok(cancel_rx)
}

async fn release_tiktok_live_recording(job_id: &str) {
    ACTIVE_RECORDINGS.lock().await.remove(job_id);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TikTokLiveTargetKind {
    Url,
    Username,
    RoomId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TikTokLiveTarget {
    kind: TikTokLiveTargetKind,
    input: String,
    username: Option<String>,
    room_id: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveVariant {
    pub format_id: String,
    pub ext: Option<String>,
    pub protocol: Option<String>,
    pub quality: Option<String>,
    pub resolution: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub tbr: Option<f64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveInspectResult {
    pub input: String,
    pub target_url: String,
    pub session_id: Option<String>,
    pub title: String,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub is_live: Option<bool>,
    pub live_status: Option<String>,
    pub variants: Vec<TikTokLiveVariant>,
    pub selected_variant: Option<TikTokLiveVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveRecordResult {
    pub job_id: String,
    pub history_id: String,
    pub filepath: String,
    pub title: String,
    pub filesize: Option<u64>,
    pub partial: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveRecoveryJob {
    pub id: String,
    pub target: String,
    pub title: String,
    pub output_dir: String,
    pub status: TikTokLiveJobStatus,
    pub segment_count: usize,
    pub has_media: bool,
    pub refresh_count: u32,
    pub reconnect_count: u32,
    pub started_at: i64,
    pub updated_at: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveRecorderConfig {
    pub max_concurrent_recordings: usize,
    pub active_recordings: usize,
    pub hard_limit: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveTelemetrySnapshot {
    pub active_recordings: usize,
    pub max_concurrent_recordings: usize,
    pub watched_streamers: usize,
    pub enabled_watchers: usize,
    pub recoverable_jobs: usize,
    pub total_segments: u64,
    pub total_refreshes: u64,
    pub total_reconnects: u64,
    pub total_recorded_bytes: u64,
    pub resource_warning: Option<&'static str>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTikTokLiveWatchEntryInput {
    pub id: Option<String>,
    pub input: String,
    pub enabled: Option<bool>,
    pub auto_record: Option<bool>,
    pub output_dir: String,
    pub preferred_quality: Option<String>,
    pub preferred_transport: Option<String>,
    pub duration_seconds: Option<u32>,
    pub cookie_mode: Option<String>,
    pub cookie_browser: Option<String>,
    pub cookie_browser_profile: Option<String>,
    pub cookie_file_path: Option<String>,
    pub poll_interval_seconds: Option<u32>,
    pub record_mode: Option<TikTokLiveRecordMode>,
    pub cooldown_seconds: Option<u32>,
    pub filename_template: Option<String>,
    pub schedule_enabled: Option<bool>,
    pub schedule_days: Option<String>,
    pub schedule_start_minute: Option<u32>,
    pub schedule_end_minute: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TikTokLiveWatchlistUpdatedEvent {
    watch_id: String,
}

impl From<&TikTokLiveJob> for TikTokLiveRecoveryJob {
    fn from(job: &TikTokLiveJob) -> Self {
        Self {
            id: job.id.clone(),
            target: job.target_input.clone(),
            title: job.title.clone(),
            output_dir: job.output_dir.clone(),
            status: job.status,
            segment_count: recoverable_segment_paths(job).len(),
            has_media: job_has_recoverable_media(job),
            refresh_count: job.refresh_count,
            reconnect_count: job.reconnect_count,
            started_at: job.started_at,
            updated_at: job.updated_at,
            error_message: job.error_message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TikTokLiveStatusEvent {
    job_id: String,
    state: String,
    attempt: Option<u32>,
    total: Option<u32>,
    auto_reconnect: Option<bool>,
}

#[derive(Debug, Clone)]
struct TikTokLiveFormat {
    variant: TikTokLiveVariant,
    url: String,
    http_headers: serde_json::Map<String, serde_json::Value>,
}

const TIKTOK_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

fn clamp_watchlist_poll_interval(seconds: Option<u32>) -> u32 {
    seconds
        .unwrap_or(60)
        .clamp(WATCHLIST_MIN_POLL_SECONDS, WATCHLIST_MAX_POLL_SECONDS)
}

fn watchlist_backoff_seconds(entry_id: &str, base_seconds: u32, attempt: u32) -> u32 {
    let exponent = attempt.saturating_sub(1).min(5);
    let delay_cap = WATCHLIST_MAX_BACKOFF_SECONDS.max(base_seconds);
    let delay = base_seconds.saturating_mul(1u32 << exponent).min(delay_cap);
    let jitter_window = (base_seconds / 5).max(1);
    let hash = entry_id.bytes().fold(0u32, |value, byte| {
        value.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    delay
        .saturating_add(hash.wrapping_add(attempt) % jitter_window)
        .min(delay_cap)
}

fn watch_status_represents_live_session(status: TikTokLiveWatchStatus) -> bool {
    matches!(
        status,
        TikTokLiveWatchStatus::Online
            | TikTokLiveWatchStatus::Recording
            | TikTokLiveWatchStatus::Recoverable
    )
}

fn same_watch_target(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn should_auto_record_watch_entry(
    previous_status: TikTokLiveWatchStatus,
    entry: &TikTokLiveWatchEntry,
    is_live: bool,
    session_id: Option<&str>,
    now: i64,
) -> bool {
    if !entry.enabled || !entry.auto_record || !is_live || entry.active_job_id.is_some() {
        return false;
    }

    match entry.record_mode {
        TikTokLiveRecordMode::ManualOnly => false,
        TikTokLiveRecordMode::OncePerLive => {
            !watch_status_represents_live_session(previous_status)
                && session_id.is_none_or(|id| entry.last_session_id.as_deref() != Some(id))
        }
        TikTokLiveRecordMode::AlwaysAfterCooldown => entry
            .last_recording_at
            .is_none_or(|last| now.saturating_sub(last) >= i64::from(entry.cooldown_seconds)),
    }
}

fn emit_watchlist_updated(app: &AppHandle, watch_id: &str) {
    app.emit(
        "tiktok-live-watchlist-updated",
        TikTokLiveWatchlistUpdatedEvent {
            watch_id: watch_id.to_string(),
        },
    )
    .ok();
}

fn persist_watch_entry(app: &AppHandle, entry: &mut TikTokLiveWatchEntry) -> Result<(), String> {
    entry.updated_at = Utc::now().timestamp();
    save_tiktok_live_watch_entry_internal(entry)?;
    emit_watchlist_updated(app, &entry.id);
    Ok(())
}

fn persist_existing_watch_entry(
    app: &AppHandle,
    entry: &mut TikTokLiveWatchEntry,
) -> Result<bool, String> {
    entry.updated_at = Utc::now().timestamp();
    let updated = update_tiktok_live_watch_entry_internal(entry)?;
    if updated {
        emit_watchlist_updated(app, &entry.id);
    }
    Ok(updated)
}

fn schedule_watch_entry(entry: &mut TikTokLiveWatchEntry, now: i64, use_backoff: bool) {
    let delay = if use_backoff {
        watchlist_backoff_seconds(
            &entry.id,
            entry.poll_interval_seconds,
            entry.backoff_attempt,
        )
    } else {
        entry.poll_interval_seconds
    };
    entry.next_check_at = now.saturating_add(i64::from(delay));
}

fn mark_watch_entry_live_but_busy(entry: &mut TikTokLiveWatchEntry, now: i64) {
    entry.status = TikTokLiveWatchStatus::Online;
    entry.backoff_attempt = 0;
    entry.last_error = Some("recordingBusy".to_string());
    schedule_watch_entry(entry, now, false);
}

fn watch_entry_label(entry: &TikTokLiveWatchEntry) -> String {
    entry
        .username
        .as_deref()
        .map(|username| format!("@{username}"))
        .unwrap_or_else(|| entry.target_input.clone())
}

fn notify_tiktok_live_watchlist(message: String) {
    crate::services::telegram::send_notification(message);
}

fn normalize_tiktok_username(value: &str) -> Option<String> {
    let username = value.trim().trim_start_matches('@');
    if username.len() < 2 || username.len() > 32 {
        return None;
    }

    username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .then(|| username.to_string())
}

fn parse_tiktok_live_target(input: &str) -> Result<TikTokLiveTarget, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("TikTok Live input is empty".to_string());
    }

    if trimmed.chars().all(|c| c.is_ascii_digit()) && trimmed.len() >= 5 {
        return Ok(TikTokLiveTarget {
            kind: TikTokLiveTargetKind::RoomId,
            input: trimmed.to_string(),
            username: None,
            room_id: Some(trimmed.to_string()),
            url: None,
        });
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let mut parsed =
            reqwest::Url::parse(trimmed).map_err(|_| "Invalid TikTok Live URL".to_string())?;
        let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
        if host != "tiktok.com" && !host.ends_with(".tiktok.com") {
            return Err("Only TikTok Live URLs are supported".to_string());
        }

        let username = parsed
            .path_segments()
            .and_then(|mut segments| segments.find(|segment| segment.starts_with('@')))
            .and_then(normalize_tiktok_username);
        let url = if let Some(username) = username.as_deref() {
            format!("https://www.tiktok.com/@{username}/live")
        } else {
            parsed.set_query(None);
            parsed.set_fragment(None);
            parsed.to_string()
        };

        return Ok(TikTokLiveTarget {
            kind: TikTokLiveTargetKind::Url,
            input: url.clone(),
            username,
            room_id: None,
            url: Some(url),
        });
    }

    let username = normalize_tiktok_username(trimmed)
        .ok_or_else(|| "Invalid TikTok username or Live URL".to_string())?;
    Ok(TikTokLiveTarget {
        kind: TikTokLiveTargetKind::Username,
        input: trimmed.to_string(),
        username: Some(username.clone()),
        room_id: None,
        url: Some(format!("https://www.tiktok.com/@{username}/live")),
    })
}

fn tiktok_room_info_url(room_id: &str) -> String {
    format!("https://webcast.tiktok.com/webcast/room/info/?aid=1988&room_id={room_id}")
}

fn tiktok_target_url(target: &TikTokLiveTarget) -> Option<String> {
    target
        .url
        .clone()
        .or_else(|| target.room_id.as_deref().map(tiktok_room_info_url))
}

fn sanitize_filename_part(value: &str, fallback: &str) -> String {
    let cleaned: String = value
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_matches([' ', '.']);

    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.chars().take(120).collect()
    }
}

fn output_path_for_recording(output_dir: &Path, title: &str) -> PathBuf {
    output_path_for_recording_with_template(output_dir, title, None, None)
}

fn output_path_for_recording_with_template(
    output_dir: &Path,
    title: &str,
    username: Option<&str>,
    template: Option<&str>,
) -> PathBuf {
    let timestamp = chrono::Local::now();
    let raw_name = template
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|template| {
            template
                .replace("{title}", title)
                .replace("{username}", username.unwrap_or("tiktok-live"))
                .replace("{date}", &timestamp.format("%Y%m%d").to_string())
                .replace("{time}", &timestamp.format("%H%M%S").to_string())
        })
        .unwrap_or_else(|| format!("{}_{}", title, timestamp.format("%Y%m%d_%H%M%S")));
    let title = sanitize_filename_part(&raw_name, "TikTok LIVE");
    let title = title.strip_suffix(".mp4").unwrap_or(&title);
    output_dir.join(format!("{title}.mp4"))
}

fn portable_file_stem(path: &Path) -> String {
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("TikTok LIVE")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("TikTok LIVE");
    filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(filename)
        .to_string()
}

fn segment_path_for_recording(output_path: &Path, index: usize) -> PathBuf {
    let stem = portable_file_stem(output_path);
    output_path.with_file_name(format!(
        "{stem}.part-{index:03}.{RECORDING_SEGMENT_EXTENSION}"
    ))
}

fn recoverable_output_path_for_recording(output_path: &Path) -> PathBuf {
    output_path.with_extension(RECORDING_SEGMENT_EXTENSION)
}

fn media_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(RECORDING_SEGMENT_EXTENSION)
        .to_ascii_lowercase()
}

fn concat_list_path_for_recording(output_path: &Path) -> PathBuf {
    let stem = portable_file_stem(output_path);
    output_path.with_file_name(format!("{stem}.ffconcat"))
}

fn path_has_media(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn existing_segment_paths(job: &TikTokLiveJob) -> Vec<PathBuf> {
    job.segment_paths
        .iter()
        .map(PathBuf::from)
        .filter(|path| path_has_media(path))
        .collect()
}

fn recoverable_segment_paths(job: &TikTokLiveJob) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = job.final_path.as_deref().map(PathBuf::from).filter(|path| {
        path_has_media(path) && path.extension().and_then(|value| value.to_str()) != Some("mp4")
    }) {
        paths.push(path);
    }
    for path in existing_segment_paths(job) {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn persisted_history_id(job_id: &str) -> String {
    format!("tiktok-live:{job_id}")
}

fn initial_live_title(target: &TikTokLiveTarget) -> String {
    target
        .username
        .as_deref()
        .map(|username| format!("TikTok LIVE @{username}"))
        .unwrap_or_else(|| "TikTok LIVE".to_string())
}

fn resolve_recording_output_dir(app: &AppHandle, requested: &str) -> Result<PathBuf, String> {
    if requested.trim().is_empty() {
        app.path().download_dir().map_err(|error| {
            BackendError::from_message(format!("Failed to resolve Downloads folder: {error}"))
                .to_wire_string()
        })
    } else {
        Ok(PathBuf::from(requested.trim()))
    }
}

fn save_job_status(job: &mut TikTokLiveJob, status: TikTokLiveJobStatus) -> Result<(), String> {
    job.status = status;
    job.touch();
    save_tiktok_live_job_internal(job)
}

fn recovery_error_message() -> String {
    "TikTok Live recording stopped unexpectedly. Review Logs for details.".to_string()
}

fn job_has_recoverable_media(job: &TikTokLiveJob) -> bool {
    !existing_segment_paths(job).is_empty()
        || job
            .final_path
            .as_deref()
            .is_some_and(|path| path_has_media(Path::new(path)))
        || job
            .output_path
            .as_deref()
            .is_some_and(|path| path_has_media(Path::new(path)))
}

fn job_recorded_bytes(job: &TikTokLiveJob) -> u64 {
    job.final_path
        .as_deref()
        .and_then(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .or_else(|| {
            let bytes = recoverable_segment_paths(job)
                .iter()
                .filter_map(|path| fs::metadata(path).ok().map(|metadata| metadata.len()))
                .sum::<u64>();
            (bytes > 0).then_some(bytes)
        })
        .unwrap_or(0)
}

fn recording_output_name_is_safe(path: &Path) -> bool {
    if path.extension().and_then(|value| value.to_str()) != Some("mp4") {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some((_, timestamp)) = stem.rsplit_once('_') else {
        return false;
    };
    timestamp.len() == 6
        && timestamp
            .chars()
            .all(|character| character.is_ascii_digit())
        && stem
            .strip_suffix(timestamp)
            .and_then(|prefix| prefix.strip_suffix('_'))
            .and_then(|prefix| prefix.rsplit_once('_'))
            .is_some_and(|(_, date)| {
                date.len() == 8 && date.chars().all(|character| character.is_ascii_digit())
            })
}

fn job_owned_paths(job: &TikTokLiveJob) -> Result<Vec<PathBuf>, String> {
    let output_dir = Path::new(&job.output_dir);
    let output_path = job
        .output_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| "TikTok Live job has no generated output path.".to_string())?;
    if output_path.parent() != Some(output_dir) || !recording_output_name_is_safe(&output_path) {
        return Err("Refusing to remove an unrecognized TikTok Live output path.".to_string());
    }

    let output_stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "TikTok Live output filename is invalid.".to_string())?;
    let mut paths = vec![
        output_path.clone(),
        concat_list_path_for_recording(&output_path),
    ];
    for value in &job.segment_paths {
        let path = PathBuf::from(value);
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let valid_name = (filename.starts_with(&format!("{output_stem}.part-"))
            && filename.ends_with(&format!(".{RECORDING_SEGMENT_EXTENSION}")))
            || path == recoverable_output_path_for_recording(&output_path);
        if path.parent() != Some(output_dir) || !valid_name {
            return Err("Refusing to remove an unrecognized TikTok Live segment path.".to_string());
        }
        paths.push(path);
    }
    if let Some(value) = job.final_path.as_deref() {
        let path = PathBuf::from(value);
        let valid_final = path == output_path
            || path == recoverable_output_path_for_recording(&output_path)
            || job
                .segment_paths
                .iter()
                .any(|segment| Path::new(segment) == path);
        if path.parent() != Some(output_dir) || !valid_final {
            return Err("Refusing to remove an unrecognized TikTok Live final path.".to_string());
        }
        paths.push(path);
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Reconcile stale active jobs after the database is initialized.
/// Signed stream URLs and request headers are intentionally never persisted.
pub fn reconcile_tiktok_live_jobs_after_restart() -> Result<usize, String> {
    let mut reconciled = 0usize;
    for mut job in get_tiktok_live_jobs_internal()?
        .into_iter()
        .filter(|job| job.status.is_restart_candidate())
    {
        job.segment_paths = existing_segment_paths(&job)
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        job.error_message = Some(
            if job_has_recoverable_media(&job) {
                job.status = TikTokLiveJobStatus::Recoverable;
                "Recording was interrupted when Youwee closed. The saved media can be continued or finalized."
            } else {
                job.status = TikTokLiveJobStatus::Interrupted;
                "Recording was interrupted before any recoverable media was written."
            }
            .to_string(),
        );
        job.touch();
        save_tiktok_live_job_internal(&job)?;
        reconciled += 1;
    }
    Ok(reconciled)
}

fn ffconcat_content(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| {
            let escaped = path
                .to_string_lossy()
                .replace('\\', "/")
                .replace('\'', "'\\''");
            format!("file '{escaped}'")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn remaining_recording_seconds(started_at: Instant, duration_seconds: Option<u32>) -> Option<u32> {
    duration_seconds.map(|total| {
        total.saturating_sub(started_at.elapsed().as_secs().min(u64::from(u32::MAX)) as u32)
    })
}

fn quality_rank(quality: Option<&str>) -> i64 {
    match quality.unwrap_or_default().to_ascii_lowercase().as_str() {
        "origin" => 100,
        "uhd_60" => 105,
        "uhd" => 90,
        "hd_60" => 80,
        "hd" => 70,
        "sd" => 60,
        "ld" => 50,
        "ao" => 10,
        _ => 0,
    }
}

fn transport_rank(protocol: Option<&str>, ext: Option<&str>) -> i64 {
    let protocol = protocol.unwrap_or_default().to_ascii_lowercase();
    let ext = ext.unwrap_or_default().to_ascii_lowercase();
    if protocol.contains("flv") || ext == "flv" {
        30
    } else if protocol.contains("hls") || protocol.contains("m3u8") || ext == "m3u8" {
        20
    } else if protocol.contains("lls") {
        10
    } else if protocol.contains("http") || protocol.contains("https") {
        5
    } else {
        0
    }
}

fn variant_score(variant: &TikTokLiveVariant) -> i64 {
    let pixels = i64::from(variant.width.unwrap_or(0)) * i64::from(variant.height.unwrap_or(0));
    let bitrate = variant.tbr.unwrap_or(0.0).round() as i64;
    let quality_score = quality_rank(variant.quality.as_deref()) * 10_000;
    let protocol_score = transport_rank(variant.protocol.as_deref(), variant.ext.as_deref());

    pixels * 1_000 + quality_score + bitrate + protocol_score
}

fn format_score(format: &TikTokLiveFormat) -> i64 {
    variant_score(&format.variant)
}

fn tiktok_live_variant_label(variant: &TikTokLiveVariant) -> String {
    let mut parts = vec![variant.format_id.clone()];
    if let Some(resolution) = variant
        .resolution
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(resolution.to_string());
    }
    if let Some(fps) = variant.fps.filter(|value| *value > 0.0) {
        parts.push(if fps.fract() == 0.0 {
            format!("{} FPS", fps as u32)
        } else {
            format!("{fps:.2} FPS")
        });
    }
    if let Some(codec) = variant.vcodec.as_deref().filter(|value| !value.is_empty()) {
        parts.push(codec.to_string());
    }
    if let Some(transport) = variant
        .protocol
        .as_deref()
        .or(variant.ext.as_deref())
        .filter(|value| !value.is_empty())
    {
        parts.push(transport.to_ascii_uppercase());
    }
    if let Some(tbr) = variant.tbr.filter(|value| *value > 0.0) {
        parts.push(format!("{} kbps", tbr.round() as u32));
    }
    parts.join(" · ")
}

fn has_video_variant(variant: &TikTokLiveVariant) -> bool {
    let has_dimensions = variant.width.unwrap_or(0) > 0 && variant.height.unwrap_or(0) > 0;
    let has_resolution = variant
        .resolution
        .as_deref()
        .is_some_and(|resolution| resolution.contains('x'));
    let has_vcodec = variant
        .vcodec
        .as_deref()
        .is_some_and(|codec| !codec.eq_ignore_ascii_case("none"));
    let audio_only_quality = variant
        .quality
        .as_deref()
        .is_some_and(|quality| quality.eq_ignore_ascii_case("ao"));

    !audio_only_quality && (has_dimensions || has_resolution || has_vcodec)
}

fn has_audio_variant(variant: &TikTokLiveVariant) -> bool {
    variant
        .acodec
        .as_deref()
        .map(|codec| !codec.eq_ignore_ascii_case("none"))
        .unwrap_or(true)
}

fn is_video_audio_variant(variant: &TikTokLiveVariant) -> bool {
    has_video_variant(variant) && has_audio_variant(variant)
}

fn select_best_variant<'a>(
    variants: impl Iterator<Item = &'a TikTokLiveVariant> + Clone,
) -> Option<&'a TikTokLiveVariant> {
    variants
        .clone()
        .filter(|variant| is_video_audio_variant(variant))
        .max_by_key(|variant| variant_score(variant))
        .or_else(|| variants.max_by_key(|variant| variant_score(variant)))
}

fn select_best_format<'a>(
    formats: impl Iterator<Item = &'a TikTokLiveFormat> + Clone,
) -> Option<&'a TikTokLiveFormat> {
    formats
        .clone()
        .filter(|format| is_video_audio_variant(&format.variant))
        .max_by_key(|format| format_score(format))
        .or_else(|| formats.max_by_key(|format| format_score(format)))
}

fn text_matches_filter(value: Option<&str>, filter: &str) -> bool {
    value
        .map(|value| {
            value
                .to_ascii_lowercase()
                .contains(&filter.to_ascii_lowercase())
        })
        .unwrap_or(false)
}

fn matches_quality_filter(variant: &TikTokLiveVariant, filter: &Option<String>) -> bool {
    let Some(filter) = filter.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if filter.eq_ignore_ascii_case("auto") {
        return true;
    }

    if filter.eq_ignore_ascii_case("origin")
        && variant
            .quality
            .as_deref()
            .is_some_and(|quality| quality.eq_ignore_ascii_case("uhd_60"))
    {
        return true;
    }

    text_matches_filter(variant.quality.as_deref(), filter)
        || text_matches_filter(variant.note.as_deref(), filter)
        || text_matches_filter(Some(&variant.format_id), filter)
}

fn matches_transport(variant: &TikTokLiveVariant, filter: &Option<String>) -> bool {
    let Some(filter) = filter.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if filter.eq_ignore_ascii_case("auto") {
        return true;
    }

    let filter = filter.to_ascii_lowercase();
    let protocol = variant
        .protocol
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let ext = variant
        .ext
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    protocol.contains(&filter)
        || ext.contains(&filter)
        || (filter == "hls" && (protocol.contains("m3u8") || ext == "m3u8"))
}

fn select_variant(
    variants: &[TikTokLiveVariant],
    preferred_quality: &Option<String>,
    preferred_transport: &Option<String>,
) -> Option<TikTokLiveVariant> {
    select_best_variant(
        variants
            .iter()
            .filter(|variant| matches_quality_filter(variant, preferred_quality))
            .filter(|variant| matches_transport(variant, preferred_transport)),
    )
    .cloned()
    .or_else(|| select_best_variant(variants.iter()).cloned())
}

fn select_format(
    formats: &[TikTokLiveFormat],
    preferred_quality: &Option<String>,
    preferred_transport: &Option<String>,
) -> Option<TikTokLiveFormat> {
    select_best_format(
        formats
            .iter()
            .filter(|format| matches_quality_filter(&format.variant, preferred_quality))
            .filter(|format| matches_transport(&format.variant, preferred_transport)),
    )
    .cloned()
    .or_else(|| select_best_format(formats.iter()).cloned())
}

fn variant_from_ytdlp_format(format: &serde_json::Value) -> Option<TikTokLiveVariant> {
    let format_id = format.get("format_id")?.as_str()?.to_string();

    Some(TikTokLiveVariant {
        format_id,
        ext: format
            .get("ext")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        protocol: format
            .get("protocol")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        quality: format
            .get("quality")
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_f64().map(|n| n.to_string()))
            })
            .or_else(|| {
                format
                    .get("format_note")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            }),
        resolution: format
            .get("resolution")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        width: format
            .get("width")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        height: format
            .get("height")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        fps: format.get("fps").and_then(|v| v.as_f64()),
        vcodec: format
            .get("vcodec")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        acodec: format
            .get("acodec")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        tbr: format.get("tbr").and_then(|v| v.as_f64()),
        note: format
            .get("format_note")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

fn format_from_ytdlp_format(format: &serde_json::Value) -> Option<TikTokLiveFormat> {
    let variant = variant_from_ytdlp_format(format)?;
    let url = format.get("url")?.as_str()?.to_string();
    let http_headers = format
        .get("http_headers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    Some(TikTokLiveFormat {
        variant,
        url,
        http_headers,
    })
}

fn parse_resolution(resolution: &str) -> (Option<u32>, Option<u32>) {
    let mut parts = resolution.split('x').map(str::trim);
    let width = parts.next().and_then(|value| value.parse().ok());
    let height = parts.next().and_then(|value| value.parse().ok());
    if parts.next().is_some() {
        return (None, None);
    }

    (width, height)
}

fn json_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|value| match value {
        serde_json::Value::String(value) if !value.trim().is_empty() => Some(value.to_string()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn json_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| match value {
        serde_json::Value::Number(value) => value.as_f64(),
        serde_json::Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

fn sdk_params_from_value(
    value: Option<&serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    match value {
        Some(serde_json::Value::Object(map)) => map.clone(),
        Some(serde_json::Value::String(raw)) => serde_json::from_str(raw).unwrap_or_default(),
        _ => serde_json::Map::new(),
    }
}

fn infer_tiktok_stream_fps(params: &serde_json::Map<String, serde_json::Value>) -> Option<f64> {
    json_f64(
        params
            .get("fps")
            .or_else(|| params.get("FPS"))
            .or_else(|| params.get("frame_rate"))
            .or_else(|| params.get("frameRate")),
    )
}

fn formats_from_tiktok_stream_data(stream_data: &serde_json::Value) -> Vec<TikTokLiveFormat> {
    let Some(data) = stream_data.get("data").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut formats = Vec::new();
    for (quality, quality_obj) in data {
        let Some(main) = quality_obj.get("main").and_then(|value| value.as_object()) else {
            continue;
        };
        let params = sdk_params_from_value(main.get("sdk_params"));
        let resolution = json_string(params.get("resolution"));
        let (width, height) = resolution
            .as_deref()
            .map(parse_resolution)
            .unwrap_or((None, None));
        let tbr =
            json_f64(params.get("vbitrate").or_else(|| params.get("v_bit_rate"))).map(|value| {
                if value > 10_000.0 {
                    value / 1000.0
                } else {
                    value
                }
            });
        let vcodec = json_string(params.get("VCodec").or_else(|| params.get("v_codec")));

        for transport in ["hls", "flv", "lls"] {
            let Some(url) = main
                .get(transport)
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };

            formats.push(TikTokLiveFormat {
                variant: TikTokLiveVariant {
                    format_id: format!("{quality}-{transport}"),
                    ext: Some(if transport == "flv" { "flv" } else { "m3u8" }.to_string()),
                    protocol: Some(transport.to_string()),
                    quality: Some(quality.to_string()),
                    resolution: resolution.clone(),
                    width,
                    height,
                    fps: infer_tiktok_stream_fps(&params),
                    vcodec: vcodec.clone(),
                    acodec: None,
                    tbr,
                    note: json_string(params.get("stream_suffix")),
                },
                url: url.to_string(),
                http_headers: serde_json::Map::new(),
            });
        }
    }

    formats
}

fn stream_data_from_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(raw) => serde_json::from_str(raw).ok(),
        value @ serde_json::Value::Object(_) if value.get("data").is_some() => Some(value.clone()),
        value @ serde_json::Value::Object(_) => value
            .get("stream_data")
            .and_then(stream_data_from_value)
            .or_else(|| Some(value.clone()).filter(|value| value.get("data").is_some())),
        _ => None,
    }
}

fn push_tiktok_stream_data_pointer(
    json: &serde_json::Value,
    path: &str,
    values: &mut Vec<serde_json::Value>,
    seen: &mut HashSet<String>,
) {
    let Some(stream_data) = json.pointer(path).and_then(stream_data_from_value) else {
        return;
    };
    let key = serde_json::to_string(&stream_data).unwrap_or_default();
    if seen.insert(key) {
        values.push(stream_data);
    }
}

fn tiktok_stream_data_values_from_json(json: &serde_json::Value) -> Vec<serde_json::Value> {
    const STREAM_DATA_POINTERS: &[&str] = &[
        "/stream_url/live_core_sdk_data/pull_data/stream_data",
        "/live_core_sdk_data/pull_data/stream_data",
        "/data/stream_url/live_core_sdk_data/pull_data/stream_data",
        "/LiveRoom/liveRoomUserInfo/liveRoom/streamData/pull_data/stream_data",
        "/LiveRoom/liveRoomUserInfo/liveRoom/hevcStreamData/pull_data/stream_data",
    ];

    let mut values = Vec::new();
    let mut seen = HashSet::new();
    for path in STREAM_DATA_POINTERS {
        push_tiktok_stream_data_pointer(json, path, &mut values, &mut seen);
    }

    if let Some(hydration_values) = json
        .get("_youwee_page_hydration")
        .and_then(|value| value.as_array())
    {
        for hydration in hydration_values {
            for path in STREAM_DATA_POINTERS {
                push_tiktok_stream_data_pointer(hydration, path, &mut values, &mut seen);
            }
        }
    }

    values
}

fn formats_from_tiktok_stream_data_values(json: &serde_json::Value) -> Vec<TikTokLiveFormat> {
    let mut formats = Vec::new();
    let mut seen_urls = HashSet::new();

    for stream_data in tiktok_stream_data_values_from_json(json) {
        for format in formats_from_tiktok_stream_data(&stream_data) {
            if seen_urls.insert(format.url.clone()) {
                formats.push(format);
            }
        }
    }

    formats
}

fn formats_from_legacy_room_stream_urls(json: &serde_json::Value) -> Vec<TikTokLiveFormat> {
    let stream_url = json.pointer("/data/stream_url");
    let mut formats = Vec::new();
    for quality in ["FULL_HD1", "HD1", "SD2", "SD1"] {
        let Some(url) = stream_url
            .and_then(|value| value.pointer(&format!("/flv_pull_url/{quality}")))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        formats.push(TikTokLiveFormat {
            variant: TikTokLiveVariant {
                format_id: format!("legacy-{quality}"),
                ext: Some("flv".to_string()),
                protocol: Some("flv".to_string()),
                quality: Some(quality.to_ascii_lowercase()),
                resolution: None,
                width: None,
                height: None,
                fps: None,
                vcodec: None,
                acodec: None,
                tbr: None,
                note: Some("legacy".to_string()),
            },
            url: url.to_string(),
            http_headers: serde_json::Map::new(),
        });
    }

    if formats.is_empty() {
        if let Some(url) = stream_url
            .and_then(|value| value.get("rtmp_pull_url"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        {
            formats.push(TikTokLiveFormat {
                variant: TikTokLiveVariant {
                    format_id: "legacy-rtmp".to_string(),
                    ext: None,
                    protocol: Some("rtmp".to_string()),
                    quality: None,
                    resolution: None,
                    width: None,
                    height: None,
                    fps: None,
                    vcodec: None,
                    acodec: None,
                    tbr: None,
                    note: Some("legacy".to_string()),
                },
                url: url.to_string(),
                http_headers: serde_json::Map::new(),
            });
        }
    }

    formats
}

fn variants_from_ytdlp_json(json: &serde_json::Value) -> Vec<TikTokLiveVariant> {
    formats_from_ytdlp_json(json)
        .into_iter()
        .map(|format| format.variant)
        .collect()
}

fn formats_from_ytdlp_json(json: &serde_json::Value) -> Vec<TikTokLiveFormat> {
    let mut formats: Vec<TikTokLiveFormat> = json
        .get("formats")
        .and_then(|v| v.as_array())
        .map(|formats| {
            formats
                .iter()
                .filter_map(format_from_ytdlp_format)
                .collect()
        })
        .unwrap_or_default();

    let mut seen_urls: HashSet<String> = formats.iter().map(|format| format.url.clone()).collect();
    formats.extend(
        formats_from_tiktok_stream_data_values(json)
            .into_iter()
            .filter(|format| seen_urls.insert(format.url.clone())),
    );

    if formats.is_empty() {
        formats_from_legacy_room_stream_urls(json)
    } else {
        formats
    }
}

fn header_value(
    headers: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<String> {
    headers.iter().find_map(|(key, value)| {
        key.eq_ignore_ascii_case(name)
            .then(|| value.as_str().map(str::to_string))
            .flatten()
    })
}

fn insert_header_if_missing(
    headers: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
    value: &str,
) {
    if header_value(headers, name).is_none() {
        headers.insert(
            name.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}

fn tiktok_ffmpeg_headers(
    headers: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut headers = headers.clone();
    insert_header_if_missing(&mut headers, "User-Agent", TIKTOK_BROWSER_USER_AGENT);
    insert_header_if_missing(&mut headers, "Referer", "https://www.tiktok.com/");
    insert_header_if_missing(&mut headers, "Origin", "https://www.tiktok.com");
    headers
}

fn ffmpeg_header_block(headers: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let mut lines = Vec::new();
    for name in ["Origin", "Referer", "Cookie"] {
        if let Some(value) = header_value(headers, name).filter(|value| !value.trim().is_empty()) {
            lines.push(format!("{name}: {value}"));
        }
    }

    (!lines.is_empty()).then(|| format!("{}\r\n", lines.join("\r\n")))
}

fn cookie_domain_matches(host: &str, cookie_domain: &str) -> bool {
    let host = host.trim_start_matches('.').to_ascii_lowercase();
    let domain = cookie_domain.trim_start_matches('.').to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}")) || domain.ends_with(&format!(".{host}"))
}

fn tiktok_cookie_header_from_netscape_file(path: &str, target_host: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let now = chrono::Utc::now().timestamp();
    let cookies: Vec<String> = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 7 {
                return None;
            }
            let domain = parts[0];
            if !cookie_domain_matches(target_host, domain) {
                return None;
            }
            let expires = parts[4].parse::<i64>().unwrap_or(0);
            if expires != 0 && expires <= now {
                return None;
            }
            Some(format!("{}={}", parts[5], parts[6]))
        })
        .collect();

    (!cookies.is_empty()).then(|| cookies.join("; "))
}

fn firefox_cookie_db_path(selected_profile: &str) -> Option<PathBuf> {
    let profile = resolve_firefox_profile_for_cookies(selected_profile);
    let profile_path = PathBuf::from(&profile);
    let path = if profile_path.is_absolute() {
        profile_path
    } else {
        firefox_profiles_ini_path()?
            .parent()?
            .join("Profiles")
            .join(profile)
    };
    Some(path.join("cookies.sqlite"))
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}-{suffix}", path.to_string_lossy()))
}

fn copy_sqlite_with_sidecars(source: &Path, dest: &Path) -> bool {
    if fs::copy(source, dest).is_err() {
        return false;
    }

    for suffix in ["wal", "shm"] {
        let source_sidecar = sqlite_sidecar_path(source, suffix);
        if source_sidecar.exists() {
            let dest_sidecar = sqlite_sidecar_path(dest, suffix);
            fs::copy(source_sidecar, dest_sidecar).ok();
        }
    }

    true
}

fn remove_sqlite_copy(path: &Path) {
    fs::remove_file(path).ok();
    for suffix in ["wal", "shm"] {
        fs::remove_file(sqlite_sidecar_path(path, suffix)).ok();
    }
}

fn read_firefox_cookie_header(db_path: &Path, target_host: &str) -> Option<String> {
    let temp_path = std::env::temp_dir().join(format!(
        "youwee-tiktok-cookies-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let db_to_read = if copy_sqlite_with_sidecars(db_path, &temp_path) {
        temp_path.as_path()
    } else {
        db_path
    };

    let result = (|| {
        let conn = Connection::open(db_to_read).ok()?;
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn
            .prepare("SELECT host, name, value FROM moz_cookies WHERE (expiry = 0 OR expiry > ?1)")
            .ok()?;
        let rows = stmt
            .query_map([now], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .ok()?;
        let cookies: Vec<String> = rows
            .filter_map(Result::ok)
            .filter(|(domain, _, _)| cookie_domain_matches(target_host, domain))
            .map(|(_, name, value)| format!("{name}={value}"))
            .collect();
        (!cookies.is_empty()).then(|| cookies.join("; "))
    })();

    remove_sqlite_copy(&temp_path);
    result
}

fn tiktok_cookie_header(
    target_url: &str,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
) -> Option<String> {
    if cookie_skip_patterns
        .map(|patterns| should_skip_cookies_for_url(target_url, patterns))
        .unwrap_or(false)
    {
        return None;
    }

    let host = reqwest::Url::parse(target_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "www.tiktok.com".to_string());

    match cookie_mode.unwrap_or("off") {
        "file" => cookie_file_path
            .filter(|path| !path.trim().is_empty())
            .and_then(|path| tiktok_cookie_header_from_netscape_file(path, &host)),
        "browser" => match (cookie_browser, cookie_browser_profile) {
            (Some(browser), Some(profile)) if browser.eq_ignore_ascii_case("firefox") => {
                firefox_cookie_db_path(profile)
                    .and_then(|path| read_firefox_cookie_header(&path, &host))
            }
            _ => None,
        },
        _ => None,
    }
}

fn emit_tiktok_live_status(
    app: &AppHandle,
    job_id: Option<&str>,
    state: &str,
    attempt: Option<u32>,
    total: Option<u32>,
    auto_reconnect: Option<bool>,
) {
    let Some(job_id) = job_id else {
        return;
    };

    app.emit(
        "tiktok-live-status",
        TikTokLiveStatusEvent {
            job_id: job_id.to_string(),
            state: state.to_string(),
            attempt,
            total,
            auto_reconnect,
        },
    )
    .ok();
}

fn should_retry_metadata_error(error: &str) -> bool {
    let wire = BackendError::from_message(error).to_wire();
    wire.retryable.unwrap_or(false)
        || wire.code == code::PARSE_FAILED
        || wire.code == code::BACKEND_UNKNOWN && wire.message.contains("yt-dlp command failed")
}

fn metadata_retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(METADATA_RETRY_BASE_DELAY_MS * u64::from(attempt))
}

fn append_reconnect_args(args: &mut Vec<String>, enabled: bool) {
    if !enabled {
        return;
    }

    args.extend([
        "-reconnect".to_string(),
        "1".to_string(),
        "-reconnect_streamed".to_string(),
        "1".to_string(),
        "-reconnect_on_network_error".to_string(),
        "1".to_string(),
        "-reconnect_on_http_error".to_string(),
        "408,429,5xx".to_string(),
        "-reconnect_max_retries".to_string(),
        RECONNECT_MAX_RETRIES.to_string(),
        "-reconnect_delay_max".to_string(),
        RECONNECT_DELAY_MAX_SECONDS.to_string(),
        "-reconnect_delay_total_max".to_string(),
        RECONNECT_DELAY_TOTAL_MAX_SECONDS.to_string(),
    ]);
}

fn build_ffmpeg_record_args(
    selected: &TikTokLiveFormat,
    cookie_header: Option<&str>,
    duration_seconds: Option<u32>,
    auto_reconnect: bool,
    output_path: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
    ];
    if let Some(seconds) = duration_seconds.filter(|seconds| *seconds > 0) {
        args.extend(["-t".to_string(), seconds.to_string()]);
    }

    let mut selected_headers = selected.http_headers.clone();
    if let Some(cookie) = cookie_header.filter(|value| !value.trim().is_empty()) {
        insert_header_if_missing(&mut selected_headers, "Cookie", cookie);
    }
    let ffmpeg_headers = tiktok_ffmpeg_headers(&selected_headers);
    if let Some(user_agent) =
        header_value(&ffmpeg_headers, "User-Agent").filter(|value| !value.trim().is_empty())
    {
        args.extend(["-user_agent".to_string(), user_agent]);
    }
    if let Some(referer) =
        header_value(&ffmpeg_headers, "Referer").filter(|value| !value.trim().is_empty())
    {
        args.extend(["-referer".to_string(), referer]);
    }
    if let Some(headers) = ffmpeg_header_block(&ffmpeg_headers) {
        args.extend(["-headers".to_string(), headers]);
    }
    append_reconnect_args(&mut args, auto_reconnect);
    args.extend([
        "-i".to_string(),
        selected.url.clone(),
        "-c".to_string(),
        "copy".to_string(),
        "-f".to_string(),
        "matroska".to_string(),
        "-cluster_time_limit".to_string(),
        "2000".to_string(),
        output_path.to_string_lossy().to_string(),
    ]);
    args
}

async fn remove_recording_paths(paths: &[PathBuf]) {
    for path in paths {
        tokio::fs::remove_file(path).await.ok();
    }
}

async fn preserve_single_segment(segment_path: &Path, output_path: &Path) -> PathBuf {
    tokio::fs::remove_file(output_path).await.ok();
    let recoverable_path = recoverable_output_path_for_recording(output_path);
    if !recoverable_path.exists()
        && tokio::fs::rename(segment_path, &recoverable_path)
            .await
            .is_ok()
    {
        recoverable_path
    } else {
        segment_path.to_path_buf()
    }
}

async fn finalization_failure(
    segment_paths: &[PathBuf],
    output_path: &Path,
) -> Result<(PathBuf, bool), String> {
    tokio::fs::remove_file(output_path).await.ok();
    if segment_paths.len() == 1 {
        return Ok((
            preserve_single_segment(&segment_paths[0], output_path).await,
            true,
        ));
    }

    Err(BackendError::from_message(
        "TikTok Live segments could not be merged. All recorded segments were preserved for recovery.",
    )
    .to_wire_string())
}

async fn finalize_recording_segments(
    ffmpeg_path: &Path,
    segment_paths: &[PathBuf],
    output_path: &Path,
    cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
) -> Result<(PathBuf, bool), String> {
    if segment_paths.is_empty() {
        return Err(BackendError::from_message(
            "TikTok Live recording produced no media segments.",
        )
        .to_wire_string());
    }

    let concat_path =
        (segment_paths.len() > 1).then(|| concat_list_path_for_recording(output_path));
    if let Some(path) = concat_path.as_ref() {
        if tokio::fs::write(path, ffconcat_content(segment_paths))
            .await
            .is_err()
        {
            return finalization_failure(segment_paths, output_path).await;
        }
    }

    let mut cmd = Command::new(ffmpeg_path);
    cmd.args(["-hide_banner", "-nostdin", "-y", "-fflags", "+genpts"]);
    if let Some(path) = concat_path.as_ref() {
        cmd.args(["-f", "concat", "-safe", "0", "-i"]).arg(path);
    } else {
        cmd.arg("-i").arg(&segment_paths[0]);
    }
    cmd.args([
        "-c",
        "copy",
        "-avoid_negative_ts",
        "make_zero",
        "-movflags",
        "+faststart",
    ])
    .arg(output_path)
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    crate::utils::CommandExt::hide_window(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(_) => {
            if let Some(path) = concat_path.as_ref() {
                tokio::fs::remove_file(path).await.ok();
            }
            return finalization_failure(segment_paths, output_path).await;
        }
    };

    let status = tokio::select! {
        status = child.wait() => status.ok(),
        _ = &mut *cancel_rx => {
            child.kill().await.ok();
            tokio::fs::remove_file(output_path).await.ok();
            if let Some(path) = concat_path.as_ref() {
                tokio::fs::remove_file(path).await.ok();
            }
            remove_recording_paths(segment_paths).await;
            return Err(BackendError::from_message("TikTok Live recording cancelled.").to_wire_string());
        }
    };

    let merged = status.is_some_and(|status| status.success())
        && tokio::fs::metadata(output_path)
            .await
            .is_ok_and(|metadata| metadata.len() > 0);
    if let Some(path) = concat_path.as_ref() {
        tokio::fs::remove_file(path).await.ok();
    }

    if merged {
        remove_recording_paths(segment_paths).await;
        Ok((output_path.to_path_buf(), false))
    } else {
        finalization_failure(segment_paths, output_path).await
    }
}

async fn complete_tiktok_live_job(
    job: &mut TikTokLiveJob,
    final_path: PathBuf,
    partial: bool,
    duration: Option<u64>,
) -> Result<TikTokLiveRecordResult, String> {
    let filepath = final_path.to_string_lossy().to_string();
    let filesize = tokio::fs::metadata(&final_path)
        .await
        .ok()
        .map(|metadata| metadata.len());
    if filesize == Some(0) || filesize.is_none() {
        return Err(BackendError::from_message(
            "TikTok Live final media file is missing or empty.",
        )
        .to_wire_string());
    }

    job.final_path = Some(filepath.clone());
    job.status = TikTokLiveJobStatus::Finalizing;
    job.touch();
    save_tiktok_live_job_internal(job)?;

    let history_id = job
        .history_id
        .clone()
        .unwrap_or_else(|| persisted_history_id(&job.id));
    upsert_history_with_id_internal(
        history_id.clone(),
        job.target_url.clone(),
        job.title.clone(),
        job.thumbnail.clone(),
        filepath.clone(),
        filesize,
        duration,
        job.format_id.clone(),
        Some(media_extension(&final_path)),
        Some("tiktok-live".to_string()),
        None,
    )?;

    job.history_id = Some(history_id.clone());
    job.status = if partial {
        TikTokLiveJobStatus::Partial
    } else {
        TikTokLiveJobStatus::Completed
    };
    job.completed_at = Some(Utc::now().timestamp());
    job.error_message = None;
    job.touch();
    save_tiktok_live_job_internal(job)?;

    Ok(TikTokLiveRecordResult {
        job_id: job.id.clone(),
        history_id,
        filepath,
        title: job.title.clone(),
        filesize,
        partial,
    })
}

async fn fetch_tiktok_live_json(
    app: &AppHandle,
    target_url: &str,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
    proxy_url: Option<&str>,
) -> Result<serde_json::Value, String> {
    let base_args = [
        "--dump-json",
        "--no-download",
        "--no-playlist",
        "--ignore-no-formats-error",
        "--no-warnings",
        "--socket-timeout",
        "20",
        "--",
        target_url,
    ];

    let output = match timeout(
        Duration::from_secs(45),
        run_ytdlp_json_with_cookies(
            app,
            &base_args,
            cookie_mode,
            cookie_browser,
            cookie_browser_profile,
            cookie_file_path,
            cookie_skip_patterns,
            proxy_url,
        ),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err("Timed out inspecting TikTok Live metadata.".to_string()),
    }
    .map_err(|error| {
        if crate::types::parse_wire_error_string(&error).is_some() {
            return error;
        }
        parse_ytdlp_error(&error)
            .unwrap_or_else(|| BackendError::from_message(error))
            .to_wire_string()
    })?;

    serde_json::from_str(&output).map_err(|e| {
        BackendError::from_message(format!("Failed to parse TikTok Live metadata: {e}"))
            .to_wire_string()
    })
}

fn decode_basic_html_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x22;", "\"")
        .replace("&#X22;", "\"")
        .replace("&amp;", "&")
}

fn script_json_from_html(html: &str, script_id: &str) -> Option<serde_json::Value> {
    let quoted = format!("id=\"{script_id}\"");
    let single_quoted = format!("id='{script_id}'");
    let id_pos = html.find(&quoted).or_else(|| html.find(&single_quoted))?;
    html[..id_pos].rfind("<script")?;
    let content_start = html[id_pos..].find('>')? + id_pos + 1;
    let content_end = html[content_start..].find("</script>")? + content_start;
    let body = html[content_start..content_end].trim();

    serde_json::from_str(body)
        .or_else(|_| serde_json::from_str(&decode_basic_html_entities(body)))
        .ok()
        .filter(|json| !tiktok_stream_data_values_from_json(json).is_empty())
}

fn tiktok_page_hydration_values_from_html(html: &str) -> Vec<serde_json::Value> {
    ["SIGI_STATE", "__UNIVERSAL_DATA_FOR_REHYDRATION__"]
        .into_iter()
        .filter_map(|script_id| script_json_from_html(html, script_id))
        .collect()
}

async fn fetch_tiktok_live_page_hydration_jsons(
    target_url: &str,
    cookie_header: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(TIKTOK_BROWSER_USER_AGENT),
    );
    headers.insert(REFERER, HeaderValue::from_static("https://www.tiktok.com/"));
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.tiktok.com"));
    if let Some(cookie) = cookie_header.filter(|value| !value.trim().is_empty()) {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.insert(COOKIE, value);
        }
    }

    let mut client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30));
    if let Some(proxy) = proxy_url.filter(|value| !value.trim().is_empty()) {
        client = client.proxy(reqwest::Proxy::all(proxy).map_err(|e| {
            BackendError::from_message(format!("Invalid proxy URL: {e}")).to_wire_string()
        })?);
    }

    let response = client
        .build()
        .map_err(|e| {
            BackendError::from_message(format!("Failed to build TikTok page client: {e}"))
                .to_wire_string()
        })?
        .get(target_url)
        .send()
        .await
        .map_err(|e| {
            BackendError::from_message(format!("Failed to fetch TikTok Live page: {e}"))
                .to_wire_string()
        })?;

    if !response.status().is_success() {
        return Err(BackendError::from_message(format!(
            "TikTok Live page request failed with status {}",
            response.status()
        ))
        .to_wire_string());
    }

    let html = response.text().await.map_err(|e| {
        BackendError::from_message(format!("Failed to read TikTok Live page: {e}")).to_wire_string()
    })?;
    Ok(tiktok_page_hydration_values_from_html(&html))
}

fn append_tiktok_page_hydration_jsons(
    json: &mut serde_json::Value,
    values: Vec<serde_json::Value>,
) {
    let Some(object) = json.as_object_mut() else {
        return;
    };
    if values.is_empty() {
        return;
    }

    object.insert(
        "_youwee_page_hydration".to_string(),
        serde_json::Value::Array(values),
    );
}

#[allow(clippy::too_many_arguments)]
async fn augment_tiktok_json_with_page_hydration(
    json: &mut serde_json::Value,
    target_url: &str,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
    proxy_url: Option<&str>,
) {
    let is_live_page = reqwest::Url::parse(target_url)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?.to_ascii_lowercase();
            Some(
                (host == "tiktok.com" || host.ends_with(".tiktok.com"))
                    && url.path().contains("/@")
                    && url.path().contains("/live"),
            )
        })
        .unwrap_or(false);
    if !is_live_page {
        return;
    }

    let cookie_header = tiktok_cookie_header(
        target_url,
        cookie_mode,
        cookie_browser,
        cookie_browser_profile,
        cookie_file_path,
        cookie_skip_patterns,
    );
    match fetch_tiktok_live_page_hydration_jsons(target_url, cookie_header.as_deref(), proxy_url)
        .await
    {
        Ok(values) if !values.is_empty() => append_tiktok_page_hydration_jsons(json, values),
        Ok(_) => {}
        Err(error) => {
            let message = BackendError::from_message(&error).message().to_string();
            add_log_internal(
                "info",
                &format!("TikTok Live page quality fallback unavailable: {message}"),
                None,
                Some(target_url),
            )
            .ok();
        }
    }
}

async fn fetch_tiktok_room_info_json(
    room_id: &str,
    cookie_header: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(TIKTOK_BROWSER_USER_AGENT),
    );
    headers.insert(REFERER, HeaderValue::from_static("https://www.tiktok.com/"));
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.tiktok.com"));
    if let Some(cookie) = cookie_header.filter(|value| !value.trim().is_empty()) {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.insert(COOKIE, value);
        }
    }

    let mut client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30));
    if let Some(proxy) = proxy_url.filter(|value| !value.trim().is_empty()) {
        client = client.proxy(reqwest::Proxy::all(proxy).map_err(|e| {
            BackendError::from_message(format!("Invalid proxy URL: {e}")).to_wire_string()
        })?);
    }

    let url = tiktok_room_info_url(room_id);
    let response = client
        .build()
        .map_err(|e| {
            BackendError::from_message(format!("Failed to build TikTok client: {e}"))
                .to_wire_string()
        })?
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            BackendError::from_message(format!("Failed to fetch TikTok room info: {e}"))
                .to_wire_string()
        })?;

    if !response.status().is_success() {
        return Err(BackendError::from_message(format!(
            "TikTok room info request failed with status {}",
            response.status()
        ))
        .to_wire_string());
    }

    response.json().await.map_err(|e| {
        BackendError::from_message(format!("Failed to parse TikTok room info: {e}"))
            .to_wire_string()
    })
}

#[allow(clippy::too_many_arguments)]
async fn fetch_tiktok_target_json(
    app: &AppHandle,
    target: &TikTokLiveTarget,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
    proxy_url: Option<&str>,
) -> Result<(serde_json::Value, String), String> {
    if let Some(room_id) = target.room_id.as_deref() {
        let url = tiktok_room_info_url(room_id);
        let cookie_header = tiktok_cookie_header(
            &url,
            cookie_mode,
            cookie_browser,
            cookie_browser_profile,
            cookie_file_path,
            cookie_skip_patterns,
        );
        let room_json =
            fetch_tiktok_room_info_json(room_id, cookie_header.as_deref(), proxy_url).await?;
        if let Some(username) = room_owner_username(&room_json) {
            let live_url = format!("https://www.tiktok.com/@{username}/live");
            if let Ok(mut json) = fetch_tiktok_live_json(
                app,
                &live_url,
                cookie_mode,
                cookie_browser,
                cookie_browser_profile,
                cookie_file_path,
                cookie_skip_patterns,
                proxy_url,
            )
            .await
            {
                augment_tiktok_json_with_page_hydration(
                    &mut json,
                    &live_url,
                    cookie_mode,
                    cookie_browser,
                    cookie_browser_profile,
                    cookie_file_path,
                    cookie_skip_patterns,
                    proxy_url,
                )
                .await;
                return Ok((json, live_url));
            }
        }
        return Ok((room_json, url));
    }

    let target_url = target
        .url
        .clone()
        .ok_or_else(|| BackendError::from_message("Missing TikTok Live URL").to_wire_string())?;
    let mut json = fetch_tiktok_live_json(
        app,
        &target_url,
        cookie_mode,
        cookie_browser,
        cookie_browser_profile,
        cookie_file_path,
        cookie_skip_patterns,
        proxy_url,
    )
    .await?;
    augment_tiktok_json_with_page_hydration(
        &mut json,
        &target_url,
        cookie_mode,
        cookie_browser,
        cookie_browser_profile,
        cookie_file_path,
        cookie_skip_patterns,
        proxy_url,
    )
    .await;
    Ok((json, target_url))
}

#[allow(clippy::too_many_arguments)]
async fn fetch_tiktok_target_json_with_retry(
    app: &AppHandle,
    target: &TikTokLiveTarget,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
    proxy_url: Option<&str>,
    job_id: Option<&str>,
) -> Result<(serde_json::Value, String), String> {
    let target_url = tiktok_target_url(target).unwrap_or_else(|| target.input.clone());
    let mut last_error = None;

    for attempt in 1..=METADATA_FETCH_ATTEMPTS {
        match fetch_tiktok_target_json(
            app,
            target,
            cookie_mode,
            cookie_browser,
            cookie_browser_profile,
            cookie_file_path,
            cookie_skip_patterns,
            proxy_url,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error)
                if attempt < METADATA_FETCH_ATTEMPTS && should_retry_metadata_error(&error) =>
            {
                let next_attempt = attempt + 1;
                add_log_internal(
                    "info",
                    &format!(
                        "Retrying TikTok Live metadata ({next_attempt}/{METADATA_FETCH_ATTEMPTS})"
                    ),
                    None,
                    Some(&target_url),
                )
                .ok();
                emit_tiktok_live_status(
                    app,
                    job_id,
                    "metadata-retry",
                    Some(next_attempt),
                    Some(METADATA_FETCH_ATTEMPTS),
                    None,
                );
                last_error = Some(error);
                sleep(metadata_retry_delay(attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        BackendError::from_message("Failed to fetch TikTok Live metadata.").to_wire_string()
    }))
}

fn string_at(json: &serde_json::Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        json.pointer(path)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    })
}

fn scalar_string_at(json: &serde_json::Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        let value = json.pointer(path)?;
        value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(str::to_string)
            .or_else(|| value.as_i64().map(|number| number.to_string()))
            .or_else(|| value.as_u64().map(|number| number.to_string()))
    })
}

fn tiktok_live_session_id(
    json: &serde_json::Value,
    target_url: &str,
    title: &str,
) -> Option<String> {
    scalar_string_at(
        json,
        &[
            "/id",
            "/room_id",
            "/display_id",
            "/webpage_url_basename",
            "/data/id",
            "/data/room_id",
            "/data/live_room_id",
        ],
    )
    .or_else(|| {
        let title = title.trim();
        (!title.is_empty()).then(|| format!("{target_url}#{title}"))
    })
}

fn room_owner_username(json: &serde_json::Value) -> Option<String> {
    string_at(json, &["/data/owner/display_id"])
        .or_else(|| string_at(json, &["/data/owner/unique_id"]))
        .and_then(|value| normalize_tiktok_username(&value))
}

fn tiktok_live_title(json: &serde_json::Value, username: Option<&str>) -> String {
    string_at(json, &["/title", "/data/title"])
        .or_else(|| username.map(|value| format!("TikTok LIVE @{value}")))
        .or_else(|| {
            string_at(json, &["/data/owner/display_id"])
                .map(|value| format!("TikTok LIVE @{value}"))
        })
        .unwrap_or_else(|| "TikTok LIVE".to_string())
}

fn tiktok_live_metadata_is_offline(json: &serde_json::Value) -> bool {
    json.get("is_live").and_then(|value| value.as_bool()) == Some(false)
        || json
            .get("live_status")
            .and_then(|value| value.as_str())
            .is_some_and(|status| {
                matches!(
                    status.to_ascii_lowercase().as_str(),
                    "offline" | "not_live" | "ended"
                )
            })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn inspect_tiktok_live(
    app: AppHandle,
    job_id: Option<String>,
    input: String,
    preferred_quality: Option<String>,
    preferred_transport: Option<String>,
    cookie_mode: Option<String>,
    cookie_browser: Option<String>,
    cookie_browser_profile: Option<String>,
    cookie_file_path: Option<String>,
    cookie_skip_patterns: Option<Vec<String>>,
    proxy_url: Option<String>,
) -> Result<TikTokLiveInspectResult, String> {
    let target = parse_tiktok_live_target(&input)
        .map_err(|e| BackendError::from_message(e).to_wire_string())?;
    let target_url = tiktok_target_url(&target)
        .ok_or_else(|| BackendError::from_message("Missing TikTok Live target").to_wire_string())?;

    add_log_internal(
        "info",
        "Inspecting TikTok Live stream metadata",
        None,
        Some(&target_url),
    )
    .ok();

    let (json, target_url) = fetch_tiktok_target_json_with_retry(
        &app,
        &target,
        cookie_mode.as_deref(),
        cookie_browser.as_deref(),
        cookie_browser_profile.as_deref(),
        cookie_file_path.as_deref(),
        cookie_skip_patterns.as_deref(),
        proxy_url.as_deref(),
        job_id.as_deref(),
    )
    .await
    .inspect_err(|error| {
        let message = BackendError::from_message(error).message().to_string();
        add_log_internal("error", &message, None, Some(&target_url)).ok();
    })?;

    let variants = variants_from_ytdlp_json(&json);
    let selected_variant = select_variant(&variants, &preferred_quality, &preferred_transport);
    let title = tiktok_live_title(&json, target.username.as_deref());
    let session_id = (!variants.is_empty())
        .then(|| tiktok_live_session_id(&json, &target_url, &title))
        .flatten();

    Ok(TikTokLiveInspectResult {
        input: target.input,
        target_url,
        session_id,
        title,
        uploader: string_at(
            &json,
            &[
                "/uploader",
                "/data/owner/display_id",
                "/data/owner/nickname",
            ],
        )
        .or(target.username),
        thumbnail: string_at(
            &json,
            &[
                "/thumbnail",
                "/data/cover/url_list/0",
                "/data/owner/avatar_thumb/url_list/0",
            ],
        ),
        is_live: json
            .get("is_live")
            .and_then(|v| v.as_bool())
            .or(Some(!variants.is_empty())),
        live_status: json
            .get("live_status")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        variants,
        selected_variant,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn record_tiktok_live(
    app: AppHandle,
    job_id: String,
    input: String,
    output_dir: String,
    duration_seconds: Option<u32>,
    preferred_quality: Option<String>,
    preferred_transport: Option<String>,
    cookie_mode: Option<String>,
    cookie_browser: Option<String>,
    cookie_browser_profile: Option<String>,
    cookie_file_path: Option<String>,
    cookie_skip_patterns: Option<Vec<String>>,
    proxy_url: Option<String>,
    auto_reconnect: Option<bool>,
) -> Result<TikTokLiveRecordResult, String> {
    record_tiktok_live_inner(
        app,
        job_id,
        input,
        output_dir,
        duration_seconds,
        preferred_quality,
        preferred_transport,
        cookie_mode,
        cookie_browser,
        cookie_browser_profile,
        cookie_file_path,
        cookie_skip_patterns,
        proxy_url,
        auto_reconnect,
        None,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn record_tiktok_live_inner(
    app: AppHandle,
    job_id: String,
    input: String,
    output_dir: String,
    duration_seconds: Option<u32>,
    preferred_quality: Option<String>,
    preferred_transport: Option<String>,
    cookie_mode: Option<String>,
    cookie_browser: Option<String>,
    cookie_browser_profile: Option<String>,
    cookie_file_path: Option<String>,
    cookie_skip_patterns: Option<Vec<String>>,
    proxy_url: Option<String>,
    auto_reconnect: Option<bool>,
    filename_template: Option<String>,
    reserved_cancel_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<TikTokLiveRecordResult, String> {
    let target = parse_tiktok_live_target(&input)
        .map_err(|error| BackendError::from_message(error).to_wire_string())?;
    let target_url = tiktok_target_url(&target)
        .ok_or_else(|| BackendError::from_message("Missing TikTok Live target").to_wire_string())?;
    let auto_reconnect = auto_reconnect.unwrap_or(true);
    let output_dir = resolve_recording_output_dir(&app, &output_dir)?;
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|error| {
            BackendError::from_message(format!("Failed to create output folder: {error}"))
                .to_wire_string()
        })?;

    let existing_job = get_tiktok_live_job_internal(&job_id)?;
    if existing_job
        .as_ref()
        .is_some_and(|job| !job.status.can_resume())
    {
        return Err(BackendError::from_message(
            "This TikTok Live recording job cannot be resumed.",
        )
        .to_wire_string());
    }

    let now = Utc::now().timestamp();
    let mut job = existing_job.unwrap_or_else(|| TikTokLiveJob {
        id: job_id.clone(),
        target_input: target.input.clone(),
        target_url: target_url.clone(),
        username: target.username.clone(),
        title: initial_live_title(&target),
        thumbnail: None,
        output_dir: output_dir.to_string_lossy().to_string(),
        output_path: None,
        final_path: None,
        preferred_quality: preferred_quality.clone(),
        preferred_transport: preferred_transport.clone(),
        duration_seconds,
        cookie_mode: cookie_mode.clone(),
        cookie_browser: cookie_browser.clone(),
        cookie_browser_profile: cookie_browser_profile.clone(),
        cookie_file_path: cookie_file_path.clone(),
        auto_reconnect,
        status: TikTokLiveJobStatus::Preparing,
        segment_paths: Vec::new(),
        refresh_count: 0,
        reconnect_count: 0,
        format_id: None,
        history_id: Some(persisted_history_id(&job_id)),
        error_message: None,
        started_at: now,
        updated_at: now,
        completed_at: None,
    });
    job.target_input = target.input.clone();
    job.target_url = target_url;
    job.username = target.username.clone();
    job.output_dir = output_dir.to_string_lossy().to_string();
    job.preferred_quality = preferred_quality.clone();
    job.preferred_transport = preferred_transport.clone();
    job.duration_seconds = duration_seconds;
    job.cookie_mode = cookie_mode.clone();
    job.cookie_browser = cookie_browser.clone();
    job.cookie_browser_profile = cookie_browser_profile.clone();
    job.cookie_file_path = cookie_file_path.clone();
    job.auto_reconnect = auto_reconnect;
    job.segment_paths = recoverable_segment_paths(&job)
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    job.final_path = None;
    job.completed_at = None;
    job.error_message = None;

    let mut cancel_rx = match reserved_cancel_rx {
        Some(cancel_rx) => cancel_rx,
        None => reserve_tiktok_live_recording(&job_id).await?,
    };
    if let Err(error) = save_job_status(&mut job, TikTokLiveJobStatus::Preparing) {
        release_tiktok_live_recording(&job_id).await;
        return Err(error);
    }

    let result: Result<TikTokLiveRecordResult, String> = async {
        let (mut json, mut target_url) = tokio::select! {
            result = fetch_tiktok_target_json_with_retry(
                &app,
                &target,
                cookie_mode.as_deref(),
                cookie_browser.as_deref(),
                cookie_browser_profile.as_deref(),
                cookie_file_path.as_deref(),
                cookie_skip_patterns.as_deref(),
                proxy_url.as_deref(),
                Some(&job_id),
            ) => result?,
            _ = &mut cancel_rx => {
                return Err(BackendError::from_message("TikTok Live recording cancelled.").to_wire_string());
            }
        };

        let formats = formats_from_ytdlp_json(&json);
        let mut selected = select_format(&formats, &preferred_quality, &preferred_transport)
            .ok_or_else(|| {
                BackendError::from_message("No TikTok Live stream variants found.")
                    .to_wire_string()
            })?;
        let ffmpeg_path = get_ffmpeg_path(&app)
            .await
            .ok_or_else(|| BackendError::from_message("FFmpeg not found.").to_wire_string())?;

        job.target_url = target_url.clone();
        job.title = tiktok_live_title(&json, target.username.as_deref());
        job.thumbnail = string_at(
            &json,
            &[
                "/thumbnail",
                "/data/cover/url_list/0",
                "/data/owner/avatar_thumb/url_list/0",
            ],
        );
        job.format_id = Some(tiktok_live_variant_label(&selected.variant));
        let output_path = job
            .output_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                output_path_for_recording_with_template(
                    &output_dir,
                    &job.title,
                    job.username.as_deref(),
                    filename_template.as_deref(),
                )
            });
        job.output_path = Some(output_path.to_string_lossy().to_string());
        save_job_status(&mut job, TikTokLiveJobStatus::Recording)?;

        let started_at = Instant::now();
        let mut segment_paths = existing_segment_paths(&job);
        let mut segment_number = segment_paths.len() + 1;
        let mut session_refresh_attempts = 0u32;
        let mut partial = !segment_paths.is_empty();

        loop {
            let remaining_seconds = remaining_recording_seconds(started_at, duration_seconds);
            if duration_seconds.is_some() && remaining_seconds == Some(0) {
                break;
            }

            let mut segment_path = segment_path_for_recording(&output_path, segment_number);
            while segment_path.exists() {
                segment_number += 1;
                segment_path = segment_path_for_recording(&output_path, segment_number);
            }
            job.segment_paths
                .push(segment_path.to_string_lossy().to_string());
            save_job_status(&mut job, TikTokLiveJobStatus::Recording)?;

            let cookie_header = tiktok_cookie_header(
                &target_url,
                cookie_mode.as_deref(),
                cookie_browser.as_deref(),
                cookie_browser_profile.as_deref(),
                cookie_file_path.as_deref(),
                cookie_skip_patterns.as_deref(),
            );
            let args = build_ffmpeg_record_args(
                &selected,
                cookie_header.as_deref(),
                remaining_seconds,
                auto_reconnect,
                &segment_path,
            );

            add_log_internal(
                "info",
                &format!(
                    "Recording TikTok Live segment {segment_number}: {} ({}, auto-reconnect: {})",
                    job.title, selected.variant.format_id, auto_reconnect
                ),
                None,
                Some(&target_url),
            )
            .ok();

            let mut cmd = Command::new(&ffmpeg_path);
            cmd.args(&args).stdout(Stdio::null()).stderr(Stdio::null());
            crate::utils::CommandExt::hide_window(&mut cmd);
            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(error) if !segment_paths.is_empty() => {
                    job.segment_paths.retain(|path| path != &segment_path.to_string_lossy());
                    add_log_internal(
                        "info",
                        &format!("Could not start the next TikTok Live segment: {error}"),
                        None,
                        Some(&target_url),
                    )
                    .ok();
                    partial = true;
                    break;
                }
                Err(error) => {
                    job.segment_paths.retain(|path| path != &segment_path.to_string_lossy());
                    return Err(BackendError::from_message(format!(
                        "Failed to start FFmpeg: {error}"
                    ))
                    .to_wire_string());
                }
            };

            emit_tiktok_live_status(
                &app,
                Some(&job_id),
                "recording",
                None,
                None,
                Some(auto_reconnect),
            );

            let status = tokio::select! {
                status = child.wait() => status.ok(),
                _ = &mut cancel_rx => {
                    child.kill().await.ok();
                    tokio::fs::remove_file(&segment_path).await.ok();
                    remove_recording_paths(&segment_paths).await;
                    tokio::fs::remove_file(&output_path).await.ok();
                    job.segment_paths.clear();
                    return Err(BackendError::from_message("TikTok Live recording cancelled.").to_wire_string());
                }
            };

            let segment_size = tokio::fs::metadata(&segment_path)
                .await
                .ok()
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            if segment_size > 0 {
                segment_paths.push(segment_path.clone());
            } else {
                tokio::fs::remove_file(&segment_path).await.ok();
                job.segment_paths
                    .retain(|path| path != &segment_path.to_string_lossy());
            }
            job.segment_paths = segment_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect();
            save_tiktok_live_job_internal(&job)?;

            if status.is_some_and(|status| status.success()) {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "FFmpeg completed without recording TikTok Live media.",
                    )
                    .to_wire_string());
                }
                break;
            }

            if duration_seconds.is_some()
                && remaining_recording_seconds(started_at, duration_seconds) == Some(0)
            {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "TikTok Live recording ended without media.",
                    )
                    .to_wire_string());
                }
                break;
            }

            if !auto_reconnect {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(format!(
                        "FFmpeg exited with code: {:?}",
                        status.and_then(|status| status.code())
                    ))
                    .to_wire_string());
                }
                partial = true;
                break;
            }

            if session_refresh_attempts >= STREAM_URL_REFRESH_ATTEMPTS {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "TikTok Live stream URL refresh attempts were exhausted without media.",
                    )
                    .to_wire_string());
                }
                partial = true;
                break;
            }

            session_refresh_attempts += 1;
            job.refresh_count += 1;
            job.reconnect_count += 1;
            save_job_status(&mut job, TikTokLiveJobStatus::Reconnecting)?;
            emit_tiktok_live_status(
                &app,
                Some(&job_id),
                "refreshing-stream",
                Some(session_refresh_attempts),
                Some(STREAM_URL_REFRESH_ATTEMPTS),
                Some(auto_reconnect),
            );
            add_log_internal(
                "info",
                &format!(
                    "Refreshing TikTok Live signed stream URL ({session_refresh_attempts}/{STREAM_URL_REFRESH_ATTEMPTS})"
                ),
                None,
                Some(&target_url),
            )
            .ok();

            let refreshed = tokio::select! {
                result = fetch_tiktok_target_json_with_retry(
                    &app,
                    &target,
                    cookie_mode.as_deref(),
                    cookie_browser.as_deref(),
                    cookie_browser_profile.as_deref(),
                    cookie_file_path.as_deref(),
                    cookie_skip_patterns.as_deref(),
                    proxy_url.as_deref(),
                    Some(&job_id),
                ) => result,
                _ = &mut cancel_rx => {
                    remove_recording_paths(&segment_paths).await;
                    tokio::fs::remove_file(&output_path).await.ok();
                    job.segment_paths.clear();
                    return Err(BackendError::from_message("TikTok Live recording cancelled.").to_wire_string());
                }
            };

            let (refreshed_json, refreshed_target_url) = match refreshed {
                Ok(result) => result,
                Err(error) if !segment_paths.is_empty() => {
                    let backend_error = BackendError::from_message(&error);
                    let stream_ended = backend_error.code() == code::TIKTOK_LIVE_OFFLINE;
                    add_log_internal(
                        "info",
                        &format!("TikTok Live URL refresh stopped: {}", backend_error.message()),
                        None,
                        Some(&target_url),
                    )
                    .ok();
                    partial = !stream_ended;
                    break;
                }
                Err(error) => return Err(error),
            };

            let refreshed_formats = formats_from_ytdlp_json(&refreshed_json);
            let Some(refreshed_selected) =
                select_format(&refreshed_formats, &preferred_quality, &preferred_transport)
            else {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "No TikTok Live stream variants found after URL refresh.",
                    )
                    .to_wire_string());
                }
                partial = !tiktok_live_metadata_is_offline(&refreshed_json);
                break;
            };

            json = refreshed_json;
            target_url = refreshed_target_url;
            selected = refreshed_selected;
            job.target_url = target_url.clone();
            job.thumbnail = string_at(
                &json,
                &[
                    "/thumbnail",
                    "/data/cover/url_list/0",
                    "/data/owner/avatar_thumb/url_list/0",
                ],
            )
            .or(job.thumbnail.clone());
            job.format_id = Some(tiktok_live_variant_label(&selected.variant));
            save_job_status(&mut job, TikTokLiveJobStatus::Recording)?;
            segment_number += 1;
        }

        if segment_paths.len() > 1 {
            emit_tiktok_live_status(
                &app,
                Some(&job_id),
                "merging-segments",
                None,
                None,
                Some(auto_reconnect),
            );
        }

        job.final_path = Some(
            recoverable_output_path_for_recording(&output_path)
                .to_string_lossy()
                .to_string(),
        );
        save_job_status(&mut job, TikTokLiveJobStatus::Finalizing)?;
        let segment_count = segment_paths.len();
        let (final_path, finalization_failed) = finalize_recording_segments(
            &ffmpeg_path,
            &segment_paths,
            &output_path,
            &mut cancel_rx,
        )
        .await?;
        partial |= finalization_failed;
        job.segment_paths = existing_segment_paths(&job)
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        let result = complete_tiktok_live_job(
            &mut job,
            final_path,
            partial,
            if partial {
                None
            } else {
                duration_seconds.map(u64::from)
            },
        )
        .await?;

        add_log_internal(
            "success",
            &format!(
                "Recorded TikTok Live: {}{}",
                job.title,
                if finalization_failed {
                    format!(
                        " ({segment_count} segments preserved because automatic MP4 finalization failed)"
                    )
                } else if partial {
                    format!(
                        " (partial recording merged after {session_refresh_attempts} signed URL refresh attempts)"
                    )
                } else if segment_count > 1 {
                    format!(
                        " ({segment_count} segments merged after {session_refresh_attempts} signed URL refreshes)"
                    )
                } else {
                    String::new()
                }
            ),
            Some(&format!(
                "file={} size={} segments={} signed_url_refreshes={} reconnects={}",
                result.filepath,
                result.filesize.unwrap_or_default(),
                segment_count,
                session_refresh_attempts,
                job.reconnect_count
            )),
            Some(&job.target_url),
        )
        .ok();

        Ok(result)
    }
    .await;

    if let Err(error) = &result {
        let backend_error = BackendError::from_message(error.as_str());
        let cancelled = backend_error.code() == code::DOWNLOAD_CANCELLED;
        job.segment_paths = existing_segment_paths(&job)
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        job.status = if cancelled {
            TikTokLiveJobStatus::Cancelled
        } else if job_has_recoverable_media(&job) {
            TikTokLiveJobStatus::Recoverable
        } else {
            TikTokLiveJobStatus::Failed
        };
        job.error_message = (!cancelled).then(recovery_error_message);
        job.completed_at = cancelled.then(|| Utc::now().timestamp());
        job.touch();
        if let Err(save_error) = save_tiktok_live_job_internal(&job) {
            log::error!("Failed to persist TikTok Live failure state: {save_error}");
        }

        add_log_internal(
            if cancelled { "info" } else { "error" },
            backend_error.message(),
            None,
            Some(&job.target_url),
        )
        .ok();
    }

    release_tiktok_live_recording(&job_id).await;
    result
}

#[tauri::command]
pub fn list_tiktok_live_watchlist() -> Result<Vec<TikTokLiveWatchEntry>, String> {
    get_tiktok_live_watchlist_internal()
}

#[tauri::command]
pub async fn get_tiktok_live_recorder_config() -> Result<TikTokLiveRecorderConfig, String> {
    load_tiktok_live_recorder_config_after_restart()?;
    Ok(TikTokLiveRecorderConfig {
        max_concurrent_recordings: configured_tiktok_live_recording_limit(),
        active_recordings: ACTIVE_RECORDINGS.lock().await.len(),
        hard_limit: TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT,
    })
}

#[tauri::command]
pub async fn get_tiktok_live_telemetry() -> Result<TikTokLiveTelemetrySnapshot, String> {
    load_tiktok_live_recorder_config_after_restart()?;
    let active_recordings = ACTIVE_RECORDINGS.lock().await.len();
    let max_concurrent_recordings = configured_tiktok_live_recording_limit();
    let watch_entries = get_tiktok_live_watchlist_internal()?;
    let jobs = get_tiktok_live_jobs_internal()?;

    Ok(TikTokLiveTelemetrySnapshot {
        active_recordings,
        max_concurrent_recordings,
        watched_streamers: watch_entries.len(),
        enabled_watchers: watch_entries.iter().filter(|entry| entry.enabled).count(),
        recoverable_jobs: jobs.iter().filter(|job| job.status.can_resume()).count(),
        total_segments: jobs
            .iter()
            .map(|job| job.segment_paths.len() as u64)
            .sum::<u64>()
            .max(
                watch_entries
                    .iter()
                    .map(|entry| u64::from(entry.last_segment_count))
                    .sum(),
            ),
        total_refreshes: jobs
            .iter()
            .map(|job| u64::from(job.refresh_count))
            .sum::<u64>()
            .max(
                watch_entries
                    .iter()
                    .map(|entry| u64::from(entry.last_refresh_count))
                    .sum(),
            ),
        total_reconnects: jobs
            .iter()
            .map(|job| u64::from(job.reconnect_count))
            .sum::<u64>()
            .max(
                watch_entries
                    .iter()
                    .map(|entry| u64::from(entry.last_reconnect_count))
                    .sum(),
            ),
        total_recorded_bytes: jobs.iter().map(job_recorded_bytes).sum::<u64>().max(
            watch_entries
                .iter()
                .filter_map(|entry| entry.last_file_size)
                .sum(),
        ),
        resource_warning: tiktok_live_resource_warning(
            active_recordings,
            max_concurrent_recordings,
        ),
    })
}

#[tauri::command]
pub fn set_tiktok_live_recorder_config(
    max_concurrent_recordings: Option<usize>,
) -> Result<TikTokLiveRecorderConfig, String> {
    let previous_limit = configured_tiktok_live_recording_limit();
    let limit = clamp_tiktok_live_recording_limit(max_concurrent_recordings);
    set_tiktok_live_recorder_limit_internal(limit)?;
    apply_tiktok_live_recording_limit(limit);
    if previous_limit != limit {
        add_log_internal(
            "info",
            &format!("TikTok Live max concurrent rooms set to {limit}."),
            tiktok_live_resource_warning(0, limit),
            None,
        )
        .ok();
    }
    Ok(TikTokLiveRecorderConfig {
        max_concurrent_recordings: limit,
        active_recordings: ACTIVE_RECORDINGS
            .try_lock()
            .map(|recordings| recordings.len())
            .unwrap_or_default(),
        hard_limit: TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT,
    })
}

#[tauri::command]
pub fn save_tiktok_live_watch_entry(
    app: AppHandle,
    entry: SaveTikTokLiveWatchEntryInput,
) -> Result<TikTokLiveWatchEntry, String> {
    let target = parse_tiktok_live_target(&entry.input)
        .map_err(|error| BackendError::from_message(error).to_wire_string())?;
    let target_url = tiktok_target_url(&target)
        .ok_or_else(|| BackendError::from_message("Missing TikTok Live target").to_wire_string())?;
    let existing = entry
        .id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map(get_tiktok_live_watch_entry_internal)
        .transpose()?
        .flatten();
    if let Some(duplicate) = get_tiktok_live_watch_entry_by_target_internal(&target_url)? {
        if existing
            .as_ref()
            .is_none_or(|current| current.id != duplicate.id)
        {
            return Err(BackendError::from_message(
                "This TikTok Live account is already in the watchlist.",
            )
            .to_wire_string());
        }
    }

    let output_dir = resolve_recording_output_dir(&app, &entry.output_dir)?;
    let now = Utc::now().timestamp();
    let target_changed = existing
        .as_ref()
        .is_some_and(|current| !same_watch_target(&current.target_url, &target_url));
    if target_changed
        && existing
            .as_ref()
            .is_some_and(|current| current.active_job_id.is_some())
    {
        return Err(BackendError::from_message(
            "Finish or remove the current recording recovery before changing this watch target.",
        )
        .to_wire_string());
    }

    let mut saved = existing.unwrap_or_else(|| TikTokLiveWatchEntry {
        id: uuid::Uuid::new_v4().to_string(),
        target_input: target.input.clone(),
        target_url: target_url.clone(),
        username: target.username.clone(),
        enabled: true,
        auto_record: true,
        output_dir: output_dir.to_string_lossy().to_string(),
        preferred_quality: Some("auto".to_string()),
        preferred_transport: Some("auto".to_string()),
        duration_seconds: None,
        cookie_mode: None,
        cookie_browser: None,
        cookie_browser_profile: None,
        cookie_file_path: None,
        poll_interval_seconds: 60,
        record_mode: TikTokLiveRecordMode::OncePerLive,
        cooldown_seconds: WATCHLIST_DEFAULT_COOLDOWN_SECONDS,
        filename_template: None,
        schedule_enabled: false,
        schedule_days: None,
        schedule_start_minute: None,
        schedule_end_minute: None,
        backoff_attempt: 0,
        next_check_at: now,
        status: TikTokLiveWatchStatus::Offline,
        active_job_id: None,
        last_error: None,
        last_checked_at: None,
        last_online_at: None,
        last_recording_at: None,
        last_session_id: None,
        last_outcome: None,
        last_completed_at: None,
        last_started_job_id: None,
        last_segment_count: 0,
        last_refresh_count: 0,
        last_reconnect_count: 0,
        last_file_size: None,
        created_at: now,
        updated_at: now,
    });
    saved.target_input = target.input;
    saved.target_url = target_url;
    saved.username = target.username;
    saved.enabled = entry.enabled.unwrap_or(saved.enabled);
    saved.auto_record = entry.auto_record.unwrap_or(saved.auto_record);
    saved.output_dir = output_dir.to_string_lossy().to_string();
    saved.preferred_quality = entry.preferred_quality;
    saved.preferred_transport = entry.preferred_transport;
    saved.duration_seconds = entry.duration_seconds.filter(|seconds| *seconds > 0);
    saved.cookie_mode = entry.cookie_mode;
    saved.cookie_browser = entry.cookie_browser;
    saved.cookie_browser_profile = entry.cookie_browser_profile;
    saved.cookie_file_path = entry.cookie_file_path;
    saved.poll_interval_seconds = clamp_watchlist_poll_interval(entry.poll_interval_seconds);
    saved.record_mode = entry.record_mode.unwrap_or(saved.record_mode);
    saved.cooldown_seconds = clamp_watchlist_cooldown(entry.cooldown_seconds);
    saved.filename_template = entry
        .filename_template
        .map(|template| template.trim().to_string())
        .filter(|template| !template.is_empty());
    saved.schedule_enabled = entry.schedule_enabled.unwrap_or(saved.schedule_enabled);
    saved.schedule_days = normalize_schedule_days(entry.schedule_days);
    saved.schedule_start_minute = normalize_schedule_minute(entry.schedule_start_minute);
    saved.schedule_end_minute = normalize_schedule_minute(entry.schedule_end_minute);
    if target_changed {
        saved.status = TikTokLiveWatchStatus::Offline;
        saved.backoff_attempt = 0;
        saved.last_error = None;
        saved.last_checked_at = None;
        saved.last_online_at = None;
        saved.last_session_id = None;
    }
    if saved.enabled && saved.active_job_id.is_none() {
        saved.next_check_at = now;
    }
    persist_watch_entry(&app, &mut saved)?;
    Ok(saved)
}

#[tauri::command]
pub fn set_tiktok_live_watch_entry_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<TikTokLiveWatchEntry, String> {
    let mut entry = get_tiktok_live_watch_entry_internal(&id)?
        .ok_or_else(|| BackendError::from_message("Watchlist entry not found.").to_wire_string())?;
    entry.enabled = enabled;
    if enabled {
        entry.next_check_at = Utc::now().timestamp();
    }
    persist_watch_entry(&app, &mut entry)?;
    Ok(entry)
}

#[tauri::command]
pub async fn delete_tiktok_live_watch_entry(app: AppHandle, id: String) -> Result<(), String> {
    let entry = get_tiktok_live_watch_entry_internal(&id)?
        .ok_or_else(|| BackendError::from_message("Watchlist entry not found.").to_wire_string())?;
    if let Some(job_id) = entry.active_job_id.as_deref() {
        if entry.status == TikTokLiveWatchStatus::Recording
            || ACTIVE_RECORDINGS.lock().await.contains_key(job_id)
        {
            return Err(BackendError::from_message(
                "Stop the active TikTok Live recording before removing this watchlist entry.",
            )
            .to_wire_string());
        }
    }
    delete_tiktok_live_watch_entry_internal(&id)?;
    emit_watchlist_updated(&app, &id);
    Ok(())
}

async fn finish_watchlist_recording(
    app: &AppHandle,
    watch_id: &str,
    job_id: &str,
    result: &Result<TikTokLiveRecordResult, String>,
) {
    let Ok(Some(mut entry)) = get_tiktok_live_watch_entry_internal(watch_id) else {
        return;
    };
    if entry.active_job_id.as_deref() != Some(job_id) {
        return;
    }

    let now = Utc::now().timestamp();
    let persisted_job = get_tiktok_live_job_internal(job_id).ok().flatten();
    entry.last_started_job_id = Some(job_id.to_string());
    if let Some(job) = persisted_job.as_ref() {
        entry.last_segment_count = job.segment_paths.len() as u32;
        entry.last_refresh_count = job.refresh_count;
        entry.last_reconnect_count = job.reconnect_count;
        entry.last_completed_at = job.completed_at;
        entry.last_file_size = job
            .final_path
            .as_deref()
            .and_then(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .or_else(|| result.as_ref().ok().and_then(|value| value.filesize));
    }
    if persisted_job
        .as_ref()
        .is_some_and(|job| job.status.can_resume())
    {
        entry.status = TikTokLiveWatchStatus::Recoverable;
        entry.last_error = Some("recordingRecoverable".to_string());
        entry.last_outcome = Some("recoverable".to_string());
        entry.next_check_at = WATCHLIST_PAUSED_CHECK_AT;
    } else {
        entry.active_job_id = None;
        match result {
            Ok(record_result) => {
                entry.status = TikTokLiveWatchStatus::Online;
                entry.backoff_attempt = 0;
                entry.last_error = None;
                entry.last_outcome = Some(if record_result.partial {
                    "partial".to_string()
                } else {
                    "completed".to_string()
                });
                entry.last_completed_at = Some(now);
                entry.last_file_size = record_result.filesize.or(entry.last_file_size);
                schedule_watch_entry(&mut entry, now, false);
            }
            Err(error) => {
                let backend_error = BackendError::from_message(error);
                if backend_error.code() == code::TIKTOK_LIVE_OFFLINE {
                    entry.status = TikTokLiveWatchStatus::Offline;
                    entry.last_error = None;
                    entry.last_outcome = Some("offline".to_string());
                } else if backend_error.code() == code::DOWNLOAD_CANCELLED {
                    entry.status = TikTokLiveWatchStatus::Online;
                    entry.last_error = None;
                    entry.last_outcome = Some("cancelled".to_string());
                } else {
                    entry.status = TikTokLiveWatchStatus::Backoff;
                    entry.last_error = Some("recordingFailed".to_string());
                    entry.last_outcome = Some("failed".to_string());
                }
                entry.backoff_attempt = entry.backoff_attempt.saturating_add(1);
                schedule_watch_entry(&mut entry, now, true);
            }
        }
    }
    let notification = entry.last_outcome.as_deref().map(|outcome| {
        let icon = match outcome {
            "completed" => "✅",
            "partial" => "🟡",
            "recoverable" => "🛟",
            "cancelled" => "⏹",
            "offline" => "⚫",
            _ => "❌",
        };
        format!(
            "{icon} TikTok Live recording {outcome}: {}",
            watch_entry_label(&entry)
        )
    });
    if persist_watch_entry(app, &mut entry).is_ok() {
        if let Some(notification) = notification {
            notify_tiktok_live_watchlist(notification);
        }
    }
}

async fn finish_linked_watchlist_recording(
    app: &AppHandle,
    job_id: &str,
    result: &Result<TikTokLiveRecordResult, String>,
) {
    match get_tiktok_live_watch_entry_by_active_job_internal(job_id) {
        Ok(Some(entry)) => finish_watchlist_recording(app, &entry.id, job_id, result).await,
        Ok(None) => {}
        Err(error) => {
            log::error!("Failed to reconcile TikTok Live watchlist after recovery action: {error}")
        }
    }
}

fn detach_linked_watchlist_job(app: &AppHandle, job_id: &str) -> Result<(), String> {
    let Some(mut entry) = get_tiktok_live_watch_entry_by_active_job_internal(job_id)? else {
        return Ok(());
    };
    entry.active_job_id = None;
    entry.status = TikTokLiveWatchStatus::Online;
    entry.backoff_attempt = 0;
    entry.last_error = None;
    schedule_watch_entry(&mut entry, Utc::now().timestamp(), false);
    persist_watch_entry(app, &mut entry)
}

async fn start_watchlist_recording(
    app: &AppHandle,
    entry: &mut TikTokLiveWatchEntry,
    session_id: Option<String>,
) -> Result<String, String> {
    if !crate::services::polling::network_config_ready() {
        return Err(BackendError::from_message(
            "Network and authentication settings are still loading. Try again shortly.",
        )
        .to_wire_string());
    }
    if entry.active_job_id.is_some() {
        return Err(BackendError::from_message(
            "This TikTok Live watchlist entry already has an active or recoverable job.",
        )
        .to_wire_string());
    }

    let job_id = uuid::Uuid::new_v4().to_string();
    let cancel_rx = reserve_tiktok_live_recording(&job_id).await?;
    let now = Utc::now().timestamp();
    entry.status = TikTokLiveWatchStatus::Recording;
    entry.active_job_id = Some(job_id.clone());
    entry.last_recording_at = Some(now);
    entry.last_started_job_id = Some(job_id.clone());
    if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
        entry.last_session_id = Some(session_id);
    }
    entry.last_outcome = Some("recording".to_string());
    entry.last_completed_at = None;
    entry.last_error = None;
    schedule_watch_entry(entry, now, false);
    if let Err(error) = persist_watch_entry(app, entry) {
        release_tiktok_live_recording(&job_id).await;
        return Err(error);
    }
    notify_tiktok_live_watchlist(format!(
        "🔴 TikTok Live recording started: {}",
        watch_entry_label(entry)
    ));

    let app_handle = app.clone();
    let watch_id = entry.id.clone();
    let input = entry.target_input.clone();
    let output_dir = entry.output_dir.clone();
    let duration_seconds = entry.duration_seconds;
    let preferred_quality = entry.preferred_quality.clone();
    let preferred_transport = entry.preferred_transport.clone();
    let cookie_mode = entry.cookie_mode.clone();
    let cookie_browser = entry.cookie_browser.clone();
    let cookie_browser_profile = entry.cookie_browser_profile.clone();
    let cookie_file_path = entry.cookie_file_path.clone();
    let filename_template = entry.filename_template.clone();
    let network = crate::services::polling::get_network_config();
    let spawned_job_id = job_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = record_tiktok_live_inner(
            app_handle.clone(),
            spawned_job_id.clone(),
            input,
            output_dir,
            duration_seconds,
            preferred_quality,
            preferred_transport,
            cookie_mode,
            cookie_browser,
            cookie_browser_profile,
            cookie_file_path,
            network.cookie_skip_patterns,
            network.proxy_url,
            Some(true),
            filename_template,
            Some(cancel_rx),
        )
        .await;
        release_tiktok_live_recording(&spawned_job_id).await;
        finish_watchlist_recording(&app_handle, &watch_id, &spawned_job_id, &result).await;
    });
    Ok(job_id)
}

async fn inspect_watch_entry(
    app: &AppHandle,
    entry: &mut TikTokLiveWatchEntry,
    allow_auto_record: bool,
) -> Result<bool, String> {
    let now = Utc::now().timestamp();
    if let Some(job_id) = entry.active_job_id.as_deref() {
        if let Some(job) = get_tiktok_live_job_internal(job_id)? {
            if job.status.can_resume() {
                entry.status = TikTokLiveWatchStatus::Recoverable;
                entry.next_check_at = WATCHLIST_PAUSED_CHECK_AT;
            } else if matches!(
                job.status,
                TikTokLiveJobStatus::Preparing
                    | TikTokLiveJobStatus::Recording
                    | TikTokLiveJobStatus::Reconnecting
                    | TikTokLiveJobStatus::Finalizing
            ) {
                entry.status = TikTokLiveWatchStatus::Recording;
                schedule_watch_entry(entry, now, false);
            } else {
                entry.active_job_id = None;
                entry.status = TikTokLiveWatchStatus::Online;
                schedule_watch_entry(entry, now, false);
            }
            if !persist_existing_watch_entry(app, entry)? {
                return Ok(false);
            }
            return Ok(false);
        }
        entry.active_job_id = None;
    }

    if !crate::services::polling::network_config_ready() {
        return Err(BackendError::from_message(
            "Network and authentication settings are still loading. Try again shortly.",
        )
        .to_wire_string());
    }

    let previous_status = entry.status;
    entry.status = TikTokLiveWatchStatus::Checking;
    entry.last_error = None;
    if !persist_existing_watch_entry(app, entry)? {
        return Ok(false);
    }
    let network = crate::services::polling::get_network_config();
    let inspect_result = inspect_tiktok_live(
        app.clone(),
        Some(format!("watch:{}", entry.id)),
        entry.target_input.clone(),
        entry.preferred_quality.clone(),
        entry.preferred_transport.clone(),
        entry.cookie_mode.clone(),
        entry.cookie_browser.clone(),
        entry.cookie_browser_profile.clone(),
        entry.cookie_file_path.clone(),
        network.cookie_skip_patterns,
        network.proxy_url,
    )
    .await;
    if get_tiktok_live_watch_entry_internal(&entry.id)?.is_none() {
        return Ok(false);
    }
    let now = Utc::now().timestamp();
    entry.last_checked_at = Some(now);

    match inspect_result {
        Ok(result) if result.is_live != Some(false) && !result.variants.is_empty() => {
            entry.last_online_at = Some(now);
            entry.backoff_attempt = 0;
            entry.last_error = None;
            let should_auto_record = allow_auto_record
                && watch_entry_allows_auto_record_now(entry)
                && should_auto_record_watch_entry(
                    previous_status,
                    entry,
                    true,
                    result.session_id.as_deref(),
                    now,
                );
            if should_auto_record {
                match start_watchlist_recording(app, entry, result.session_id.clone()).await {
                    Ok(_) => return Ok(true),
                    Err(error)
                        if BackendError::from_message(&error).message()
                            == TIKTOK_LIVE_ONE_ROOM_MESSAGE =>
                    {
                        mark_watch_entry_live_but_busy(entry, now);
                    }
                    Err(error) => return Err(error),
                }
            }
            if entry.status != TikTokLiveWatchStatus::Online {
                entry.status = TikTokLiveWatchStatus::Online;
                schedule_watch_entry(entry, now, false);
            }
        }
        Ok(_) => {
            entry.status = TikTokLiveWatchStatus::Offline;
            entry.backoff_attempt = entry.backoff_attempt.saturating_add(1);
            entry.last_error = None;
            schedule_watch_entry(entry, now, true);
        }
        Err(error) => {
            let backend_error = BackendError::from_message(&error);
            entry.backoff_attempt = entry.backoff_attempt.saturating_add(1);
            if backend_error.code() == code::TIKTOK_LIVE_OFFLINE {
                entry.status = TikTokLiveWatchStatus::Offline;
                entry.last_error = None;
            } else {
                entry.status = if should_retry_metadata_error(&error) {
                    TikTokLiveWatchStatus::Backoff
                } else {
                    TikTokLiveWatchStatus::Error
                };
                entry.last_error = Some("metadataFailed".to_string());
            }
            schedule_watch_entry(entry, now, true);
        }
    }
    persist_existing_watch_entry(app, entry)?;
    Ok(false)
}

#[tauri::command]
pub async fn inspect_tiktok_live_watch_entry(
    app: AppHandle,
    id: String,
) -> Result<TikTokLiveWatchEntry, String> {
    let mut entry = get_tiktok_live_watch_entry_internal(&id)?
        .ok_or_else(|| BackendError::from_message("Watchlist entry not found.").to_wire_string())?;
    inspect_watch_entry(&app, &mut entry, false).await?;
    get_tiktok_live_watch_entry_internal(&id)?
        .ok_or_else(|| BackendError::from_message("Watchlist entry not found.").to_wire_string())
}

#[tauri::command]
pub async fn record_tiktok_live_watch_entry(
    app: AppHandle,
    id: String,
) -> Result<TikTokLiveWatchEntry, String> {
    let mut entry = get_tiktok_live_watch_entry_internal(&id)?
        .ok_or_else(|| BackendError::from_message("Watchlist entry not found.").to_wire_string())?;
    start_watchlist_recording(&app, &mut entry, None).await?;
    Ok(entry)
}

async fn poll_due_tiktok_live_watchlist(app: &AppHandle) -> Result<(), String> {
    if !crate::services::polling::network_config_ready() {
        return Ok(());
    }
    let now = Utc::now().timestamp();
    for mut entry in get_due_tiktok_live_watchlist_internal(now)? {
        if !entry.enabled {
            continue;
        }
        if inspect_watch_entry(app, &mut entry, true).await?
            && tiktok_live_recorder_at_limit().await
        {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

pub fn reconcile_tiktok_live_watchlist_after_restart() -> Result<usize, String> {
    let now = Utc::now().timestamp();
    let mut reconciled = 0usize;
    for mut entry in get_tiktok_live_watchlist_internal()? {
        let mut changed = false;
        if let Some(job_id) = entry.active_job_id.as_deref() {
            match get_tiktok_live_job_internal(job_id)? {
                Some(job) if job.status.can_resume() => {
                    entry.status = TikTokLiveWatchStatus::Recoverable;
                    entry.next_check_at = WATCHLIST_PAUSED_CHECK_AT;
                    entry.last_error = Some("restartRecoverable".to_string());
                }
                Some(job)
                    if matches!(
                        job.status,
                        TikTokLiveJobStatus::Completed
                            | TikTokLiveJobStatus::Partial
                            | TikTokLiveJobStatus::Cancelled
                    ) =>
                {
                    entry.active_job_id = None;
                    entry.status = TikTokLiveWatchStatus::Online;
                    entry.last_error = None;
                    schedule_watch_entry(&mut entry, now, false);
                }
                Some(_) => {
                    entry.status = TikTokLiveWatchStatus::Recording;
                }
                None => {
                    entry.active_job_id = None;
                    entry.status = TikTokLiveWatchStatus::Error;
                    entry.last_error = Some("missingJob".to_string());
                    entry.backoff_attempt = entry.backoff_attempt.saturating_add(1);
                    schedule_watch_entry(&mut entry, now, true);
                }
            }
            changed = true;
        } else if matches!(
            entry.status,
            TikTokLiveWatchStatus::Checking | TikTokLiveWatchStatus::Recording
        ) {
            entry.status = TikTokLiveWatchStatus::Offline;
            entry.next_check_at = now;
            changed = true;
        }
        if changed {
            entry.updated_at = now;
            save_tiktok_live_watch_entry_internal(&entry)?;
            reconciled += 1;
        }
    }
    Ok(reconciled)
}

pub fn start_tiktok_live_watchlist(app: AppHandle) {
    if TIKTOK_LIVE_WATCHLIST_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        log::info!("TikTok Live watchlist polling started");
        while TIKTOK_LIVE_WATCHLIST_ACTIVE.load(Ordering::SeqCst) {
            if let Err(error) = poll_due_tiktok_live_watchlist(&app).await {
                log::error!("TikTok Live watchlist polling failed: {error}");
            }
            sleep(Duration::from_secs(WATCHLIST_LOOP_TICK_SECONDS)).await;
        }
        log::info!("TikTok Live watchlist polling stopped");
    });
}

#[tauri::command]
pub fn list_tiktok_live_recovery_jobs() -> Result<Vec<TikTokLiveRecoveryJob>, String> {
    Ok(get_tiktok_live_jobs_internal()?
        .iter()
        .filter(|job| job.status.can_resume())
        .map(TikTokLiveRecoveryJob::from)
        .collect())
}

#[tauri::command]
pub async fn finalize_tiktok_live_recovery(
    app: AppHandle,
    job_id: String,
) -> Result<TikTokLiveRecordResult, String> {
    if ACTIVE_RECORDINGS.lock().await.contains_key(&job_id) {
        return Err(BackendError::from_message(
            "Stop the active TikTok Live recording before finalizing it.",
        )
        .to_wire_string());
    }
    let mut job = get_tiktok_live_job_internal(&job_id)?
        .ok_or_else(|| BackendError::from_message("TikTok Live job not found.").to_wire_string())?;
    if !job.status.can_resume() {
        return Err(BackendError::from_message(
            "This TikTok Live recording no longer needs recovery.",
        )
        .to_wire_string());
    }

    let output_path = job
        .output_path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| output_path_for_recording(Path::new(&job.output_dir), &job.title));
    job.output_path = Some(output_path.to_string_lossy().to_string());
    let existing_final = job
        .final_path
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| {
            path_has_media(path) && path.extension().and_then(|value| value.to_str()) == Some("mp4")
        })
        .or_else(|| path_has_media(&output_path).then(|| output_path.clone()));
    let segments = recoverable_segment_paths(&job);
    if existing_final.is_none() && segments.is_empty() {
        save_job_status(&mut job, TikTokLiveJobStatus::Interrupted)?;
        return Err(BackendError::from_message(
            "No recoverable TikTok Live media was found on disk.",
        )
        .to_wire_string());
    }

    job.final_path = Some(
        recoverable_output_path_for_recording(&output_path)
            .to_string_lossy()
            .to_string(),
    );
    save_job_status(&mut job, TikTokLiveJobStatus::Finalizing)?;
    let finalization = if let Some(path) = existing_final {
        Ok((path, false))
    } else {
        match get_ffmpeg_path(&app).await {
            Some(ffmpeg_path) => {
                let (_cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
                finalize_recording_segments(&ffmpeg_path, &segments, &output_path, &mut cancel_rx)
                    .await
            }
            None => Err(BackendError::from_message("FFmpeg not found.").to_wire_string()),
        }
    };
    let (final_path, finalization_failed) = match finalization {
        Ok(result) => result,
        Err(error) => {
            job.status = TikTokLiveJobStatus::Recoverable;
            job.error_message = Some(recovery_error_message());
            job.touch();
            save_tiktok_live_job_internal(&job).ok();
            return Err(error);
        }
    };

    job.segment_paths = existing_segment_paths(&job)
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    let result = complete_tiktok_live_job(&mut job, final_path, true, None).await;
    if result.is_err() {
        job.status = TikTokLiveJobStatus::Recoverable;
        job.error_message = Some(recovery_error_message());
        job.touch();
        save_tiktok_live_job_internal(&job).ok();
    } else {
        add_log_internal(
            "success",
            &format!(
                "Recovered TikTok Live recording: {}{}",
                job.title,
                if finalization_failed {
                    " (MKV preserved because MP4 finalization failed)"
                } else {
                    ""
                }
            ),
            result.as_ref().ok().map(|value| value.filepath.as_str()),
            Some(&job.target_url),
        )
        .ok();
    }
    finish_linked_watchlist_recording(&app, &job_id, &result).await;
    result
}

#[tauri::command]
pub async fn continue_tiktok_live_recovery(
    app: AppHandle,
    job_id: String,
    cookie_skip_patterns: Option<Vec<String>>,
    proxy_url: Option<String>,
) -> Result<TikTokLiveRecordResult, String> {
    let job = get_tiktok_live_job_internal(&job_id)?
        .ok_or_else(|| BackendError::from_message("TikTok Live job not found.").to_wire_string())?;
    if !job.status.can_resume() {
        return Err(BackendError::from_message(
            "This TikTok Live recording no longer needs recovery.",
        )
        .to_wire_string());
    }

    let result = record_tiktok_live(
        app.clone(),
        job.id,
        job.target_input,
        job.output_dir,
        job.duration_seconds,
        job.preferred_quality,
        job.preferred_transport,
        job.cookie_mode,
        job.cookie_browser,
        job.cookie_browser_profile,
        job.cookie_file_path,
        cookie_skip_patterns,
        proxy_url,
        Some(job.auto_reconnect),
    )
    .await;
    finish_linked_watchlist_recording(&app, &job_id, &result).await;
    result
}

#[tauri::command]
pub async fn delete_tiktok_live_recovery(app: AppHandle, job_id: String) -> Result<(), String> {
    if ACTIVE_RECORDINGS.lock().await.contains_key(&job_id) {
        return Err(BackendError::from_message(
            "Stop the active TikTok Live recording before deleting recovery data.",
        )
        .to_wire_string());
    }
    let job = get_tiktok_live_job_internal(&job_id)?
        .ok_or_else(|| BackendError::from_message("TikTok Live job not found.").to_wire_string())?;
    if !job.status.can_resume() {
        return Err(BackendError::from_message(
            "Completed TikTok Live recordings must be managed from Library.",
        )
        .to_wire_string());
    }

    if job.output_path.is_some() {
        for path in job_owned_paths(&job)? {
            if path.exists() {
                tokio::fs::remove_file(&path).await.map_err(|error| {
                    BackendError::from_message(format!(
                        "Failed to remove TikTok Live recovery file {}: {error}",
                        path.display()
                    ))
                    .to_wire_string()
                })?;
            }
        }
    } else if job_has_recoverable_media(&job) {
        return Err(BackendError::from_message(
            "Refusing to delete recovery media without its generated output identity.",
        )
        .to_wire_string());
    }
    delete_tiktok_live_job_internal(&job_id)?;
    detach_linked_watchlist_job(&app, &job_id)?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_tiktok_live_recording(job_id: String) -> Result<(), String> {
    let mut recordings = ACTIVE_RECORDINGS.lock().await;
    if let Some(cancel_tx) = recordings.get_mut(&job_id).and_then(Option::take) {
        cancel_tx.send(()).ok();
    }
    Ok(())
}

fn tiktok_live_status_icon(status: TikTokLiveWatchStatus) -> &'static str {
    match status {
        TikTokLiveWatchStatus::Recording => "🔴",
        TikTokLiveWatchStatus::Online => "🟢",
        TikTokLiveWatchStatus::Backoff | TikTokLiveWatchStatus::Recoverable => "🟡",
        TikTokLiveWatchStatus::Error => "❌",
        TikTokLiveWatchStatus::Checking => "🔎",
        TikTokLiveWatchStatus::Offline => "⚫",
    }
}

fn format_tiktok_live_telegram_target(entry: &TikTokLiveWatchEntry) -> String {
    entry
        .username
        .as_deref()
        .map(|username| format!("@{username}"))
        .unwrap_or_else(|| entry.target_input.clone())
}

fn format_tiktok_live_telegram_timestamp(seconds: Option<i64>) -> String {
    seconds
        .and_then(|seconds| chrono::DateTime::<Utc>::from_timestamp(seconds, 0))
        .map(|value| {
            value
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "never".to_string())
}

fn format_tiktok_live_telegram_entry(entry: &TikTokLiveWatchEntry, index: usize) -> String {
    [
        Some(format!(
            "{}. {} {}",
            index + 1,
            tiktok_live_status_icon(entry.status),
            format_tiktok_live_telegram_target(entry)
        )),
        Some(format!(
            "Status: {}{}",
            entry.status.as_str(),
            if entry.enabled { "" } else { " · disabled" }
        )),
        entry
            .last_outcome
            .as_deref()
            .map(|outcome| format!("Last outcome: {outcome}")),
        (entry.last_segment_count > 0).then(|| format!("Segments: {}", entry.last_segment_count)),
        (entry.last_reconnect_count > 0)
            .then(|| format!("Reconnects: {}", entry.last_reconnect_count)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

fn build_tiktok_live_telegram_watchlist_reply(entries: &[TikTokLiveWatchEntry]) -> String {
    if entries.is_empty() {
        return "📭 TikTok Live watchlist is empty.\nUse /tl_add @username to add one.".to_string();
    }

    [
        format!("📺 TikTok Live watchlist ({})", entries.len()),
        String::new(),
        entries
            .iter()
            .take(8)
            .enumerate()
            .map(|(index, entry)| format_tiktok_live_telegram_entry(entry, index))
            .collect::<Vec<_>>()
            .join("\n\n"),
    ]
    .join("\n")
}

fn tiktok_live_telegram_target_matches(entry: &TikTokLiveWatchEntry, target: &str) -> bool {
    let raw = target.trim();
    let normalized = raw.to_ascii_lowercase();
    let username = raw.trim_start_matches('@').to_ascii_lowercase();

    entry.id == raw
        || entry.target_input.eq_ignore_ascii_case(raw)
        || entry.target_url.eq_ignore_ascii_case(raw)
        || entry
            .username
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(&username))
        || entry
            .target_url
            .to_ascii_lowercase()
            .contains(&format!("/@{username}/live"))
        || entry.target_url.to_ascii_lowercase() == normalized
}

fn find_tiktok_live_telegram_entry(
    entries: &[TikTokLiveWatchEntry],
    target: &str,
) -> Option<TikTokLiveWatchEntry> {
    entries
        .iter()
        .find(|entry| tiktok_live_telegram_target_matches(entry, target))
        .cloned()
}

fn build_tiktok_live_telegram_status_reply(
    entries: &[TikTokLiveWatchEntry],
    config: &TikTokLiveRecorderConfig,
    target: Option<&str>,
) -> String {
    if let Some(target) = target {
        let Some(entry) = find_tiktok_live_telegram_entry(entries, target) else {
            return format!("TikTok Live target not found: {target}");
        };
        return [
            Some(format!(
                "{} {}",
                tiktok_live_status_icon(entry.status),
                format_tiktok_live_telegram_target(&entry)
            )),
            Some(format!("Status: {}", entry.status.as_str())),
            Some(format!(
                "Enabled: {}",
                if entry.enabled { "yes" } else { "no" }
            )),
            Some(format!(
                "Auto-record: {}",
                if entry.auto_record { "yes" } else { "no" }
            )),
            Some(format!(
                "Last checked: {}",
                format_tiktok_live_telegram_timestamp(entry.last_checked_at)
            )),
            Some(format!(
                "Last online: {}",
                format_tiktok_live_telegram_timestamp(entry.last_online_at)
            )),
            Some(format!(
                "Last recording: {}",
                format_tiktok_live_telegram_timestamp(entry.last_recording_at)
            )),
            entry
                .last_outcome
                .as_deref()
                .map(|outcome| format!("Last outcome: {outcome}")),
            entry
                .last_error
                .as_deref()
                .map(|error| format!("Last error: {error}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    }

    let recording = entries
        .iter()
        .filter(|entry| entry.status == TikTokLiveWatchStatus::Recording)
        .count();
    let online = entries
        .iter()
        .filter(|entry| entry.status == TikTokLiveWatchStatus::Online)
        .count();
    let enabled = entries.iter().filter(|entry| entry.enabled).count();
    [
        "📡 TikTok Live Recorder".to_string(),
        format!(
            "Active rooms: {}/{}",
            config.active_recordings, config.max_concurrent_recordings
        ),
        format!("Configured hard limit: {}", config.hard_limit),
        format!("Watchlist: {} total · {enabled} enabled", entries.len()),
        format!("Online: {online}"),
        format!("Recording: {recording}"),
    ]
    .join("\n")
}

fn find_tiktok_live_telegram_entry_by_target(
    target: &str,
) -> Result<Option<TikTokLiveWatchEntry>, String> {
    Ok(find_tiktok_live_telegram_entry(
        &get_tiktok_live_watchlist_internal()?,
        target,
    ))
}

fn ensure_tiktok_live_telegram_entry(
    app: AppHandle,
    target: &str,
) -> Result<TikTokLiveWatchEntry, String> {
    if let Some(entry) = find_tiktok_live_telegram_entry_by_target(target)? {
        return Ok(entry);
    }
    save_tiktok_live_watch_entry(
        app,
        SaveTikTokLiveWatchEntryInput {
            id: None,
            input: target.to_string(),
            enabled: None,
            auto_record: None,
            output_dir: String::new(),
            preferred_quality: Some("auto".to_string()),
            preferred_transport: Some("auto".to_string()),
            duration_seconds: None,
            cookie_mode: None,
            cookie_browser: None,
            cookie_browser_profile: None,
            cookie_file_path: None,
            poll_interval_seconds: Some(60),
            record_mode: Some(TikTokLiveRecordMode::OncePerLive),
            cooldown_seconds: Some(WATCHLIST_DEFAULT_COOLDOWN_SECONDS),
            filename_template: None,
            schedule_enabled: None,
            schedule_days: None,
            schedule_start_minute: None,
            schedule_end_minute: None,
        },
    )
}

pub async fn handle_tiktok_live_telegram_command(
    app: AppHandle,
    command: &str,
    target: Option<&str>,
) -> Result<String, String> {
    match command {
        "watchlist" => Ok(build_tiktok_live_telegram_watchlist_reply(
            &get_tiktok_live_watchlist_internal()?,
        )),
        "status" => {
            let entries = get_tiktok_live_watchlist_internal()?;
            let config = get_tiktok_live_recorder_config().await?;
            Ok(build_tiktok_live_telegram_status_reply(
                &entries, &config, target,
            ))
        }
        command => {
            let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) else {
                return Ok(format!(
                    "Missing TikTok Live target. Example: /tl_{command} @username"
                ));
            };

            match command {
                "add" => {
                    let entry = ensure_tiktok_live_telegram_entry(app, target)?;
                    Ok(format!(
                        "Added TikTok Live target: {}",
                        format_tiktok_live_telegram_target(&entry)
                    ))
                }
                "remove" => {
                    let Some(entry) = find_tiktok_live_telegram_entry_by_target(target)? else {
                        return Ok(format!("TikTok Live target not found: {target}"));
                    };
                    delete_tiktok_live_watch_entry(app, entry.id.clone()).await?;
                    Ok(format!(
                        "Removed TikTok Live target: {}",
                        format_tiktok_live_telegram_target(&entry)
                    ))
                }
                "enable" | "disable" => {
                    let Some(entry) = find_tiktok_live_telegram_entry_by_target(target)? else {
                        return Ok(format!("TikTok Live target not found: {target}"));
                    };
                    let enabled = command == "enable";
                    set_tiktok_live_watch_entry_enabled(app, entry.id.clone(), enabled)?;
                    Ok(format!(
                        "{} TikTok Live target: {}",
                        if enabled { "Enabled" } else { "Disabled" },
                        format_tiktok_live_telegram_target(&entry)
                    ))
                }
                "inspect" => {
                    let entry = ensure_tiktok_live_telegram_entry(app.clone(), target)?;
                    let updated = inspect_tiktok_live_watch_entry(app, entry.id).await?;
                    Ok(format!(
                        "Checked {}: {}{}",
                        format_tiktok_live_telegram_target(&updated),
                        updated.status.as_str(),
                        updated
                            .last_error
                            .as_deref()
                            .map(|error| format!(" ({error})"))
                            .unwrap_or_default()
                    ))
                }
                "record" => {
                    let entry = ensure_tiktok_live_telegram_entry(app.clone(), target)?;
                    record_tiktok_live_watch_entry(app, entry.id.clone()).await?;
                    Ok(format!(
                        "Started TikTok Live recording: {}",
                        format_tiktok_live_telegram_target(&entry)
                    ))
                }
                "stop" => {
                    let Some(entry) = find_tiktok_live_telegram_entry_by_target(target)? else {
                        return Ok(format!("TikTok Live target not found: {target}"));
                    };
                    let Some(job_id) = entry.active_job_id.as_deref() else {
                        return Ok(format!(
                            "No active TikTok Live recording for {}.",
                            format_tiktok_live_telegram_target(&entry)
                        ));
                    };
                    cancel_tiktok_live_recording(job_id.to_string()).await?;
                    Ok(format!(
                        "Stopping TikTok Live recording: {}",
                        format_tiktok_live_telegram_target(&entry)
                    ))
                }
                _ => Ok("Unsupported command. Use /help to see available commands.".to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static RECORDING_LIMIT_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

    #[test]
    fn parses_username_and_live_url_targets() {
        let username = parse_tiktok_live_target("@some.user").expect("username");
        assert_eq!(username.kind, TikTokLiveTargetKind::Username);
        assert_eq!(
            username.url.as_deref(),
            Some("https://www.tiktok.com/@some.user/live")
        );

        let url =
            parse_tiktok_live_target("https://www.tiktok.com/@some.user/live?token=secret#frag")
                .expect("url");
        assert_eq!(url.kind, TikTokLiveTargetKind::Url);
        assert_eq!(url.username.as_deref(), Some("some.user"));
        assert_eq!(url.input, "https://www.tiktok.com/@some.user/live");
        assert!(!url.input.contains("token=secret"));
        assert_eq!(
            url.url.as_deref(),
            Some("https://www.tiktok.com/@some.user/live")
        );
    }

    #[test]
    fn treats_profile_urls_as_live_targets() {
        let url = parse_tiktok_live_target("https://www.tiktok.com/@some.user?lang=en#profile")
            .expect("profile url");
        assert_eq!(url.kind, TikTokLiveTargetKind::Url);
        assert_eq!(url.username.as_deref(), Some("some.user"));
        assert_eq!(
            url.url.as_deref(),
            Some("https://www.tiktok.com/@some.user/live")
        );
    }

    #[test]
    fn parses_room_id_without_guessing_a_url() {
        let target = parse_tiktok_live_target("1234567890").expect("room id");
        assert_eq!(target.kind, TikTokLiveTargetKind::RoomId);
        assert_eq!(target.room_id.as_deref(), Some("1234567890"));
        assert!(target.url.is_none());
        assert_eq!(
            tiktok_target_url(&target).as_deref(),
            Some("https://webcast.tiktok.com/webcast/room/info/?aid=1988&room_id=1234567890")
        );
    }

    #[test]
    fn rejects_non_tiktok_urls() {
        let err = parse_tiktok_live_target("https://example.com/@abc/live").unwrap_err();
        assert!(err.contains("TikTok"));
    }

    #[test]
    fn ytdlp_variants_do_not_expose_signed_urls() {
        let json = serde_json::json!({
            "formats": [{
                "format_id": "hls-1080",
                "url": "https://signed.example/secret?token=abc",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "width": 1920,
                "height": 1080,
                "tbr": 4500.0,
                "format_note": "Full HD"
            }]
        });

        let variants = variants_from_ytdlp_json(&json);
        let rendered = serde_json::to_string(&variants).expect("json");
        assert_eq!(variants.len(), 1);
        assert!(!rendered.contains("signed.example"));
        assert!(!rendered.contains("token=abc"));
    }

    #[test]
    fn parses_tiktok_stream_data_variants_without_exposing_signed_urls() {
        let stream_data = serde_json::json!({
            "data": {
                "hd": {
                    "main": {
                        "hls": "https://signed.example/hd.m3u8?token=secret",
                        "flv": "https://signed.example/hd.flv?token=secret",
                        "sdk_params": "{\"resolution\":\"1280x720\",\"VCodec\":\"h264\",\"vbitrate\":2500000,\"stream_suffix\":\"main\"}"
                    }
                },
                "sd": {
                    "main": {
                        "hls": "https://signed.example/sd.m3u8?token=secret",
                        "sdk_params": {
                            "resolution": "854x480",
                            "v_codec": "h264",
                            "v_bit_rate": "1000000"
                        }
                    }
                }
            }
        });

        let variants: Vec<TikTokLiveVariant> = formats_from_tiktok_stream_data(&stream_data)
            .into_iter()
            .map(|format| format.variant)
            .collect();
        let rendered = serde_json::to_string(&variants).expect("json");

        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].format_id, "hd-hls");
        assert_eq!(variants[0].width, Some(1280));
        assert_eq!(variants[0].height, Some(720));
        assert_eq!(variants[0].tbr, Some(2500.0));
        assert!(!rendered.contains("signed.example"));
        assert!(!rendered.contains("token=secret"));
    }

    #[test]
    fn falls_back_to_nested_tiktok_stream_data_when_formats_are_missing() {
        let stream_data = serde_json::json!({
            "data": {
                "origin": {
                    "main": {
                        "hls": "https://signed.example/origin.m3u8",
                        "sdk_params": "{\"resolution\":\"1920x1080\",\"vbitrate\":4500000}"
                    }
                }
            }
        });
        let json = serde_json::json!({
            "stream_url": {
                "live_core_sdk_data": {
                    "pull_data": {
                        "stream_data": stream_data.to_string()
                    }
                }
            }
        });

        let variants = variants_from_ytdlp_json(&json);

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].format_id, "origin-hls");
        assert_eq!(variants[0].resolution.as_deref(), Some("1920x1080"));
    }

    #[test]
    fn merges_hevc_page_hydration_stream_data_with_ytdlp_formats() {
        let browser_stream_data = serde_json::json!({
            "data": {
                "hd": {
                    "main": {
                        "hls": "https://signed.example/hd.m3u8",
                        "sdk_params": "{\"resolution\":\"720x1280\",\"vbitrate\":1800000,\"VCodec\":\"h264\"}"
                    }
                }
            }
        });
        let hevc_stream_data = serde_json::json!({
            "data": {
                "uhd_60": {
                    "main": {
                        "flv": "https://signed.example/uhd60.flv",
                        "sdk_params": "{\"resolution\":\"1080x1920\",\"vbitrate\":4000000,\"VCodec\":\"h265\",\"stream_suffix\":\"uhd560\"}"
                    }
                }
            }
        });
        let sigi_state = serde_json::json!({
            "LiveRoom": {
                "liveRoomUserInfo": {
                    "liveRoom": {
                        "streamData": {
                            "pull_data": {
                                "stream_data": browser_stream_data.to_string()
                            }
                        },
                        "hevcStreamData": {
                            "pull_data": {
                                "stream_data": hevc_stream_data.to_string()
                            }
                        }
                    }
                }
            }
        });
        let json = serde_json::json!({
            "formats": [
                {
                    "format_id": "yt-dlp-hd",
                    "url": "https://signed.example/ytdlp-hd.m3u8",
                    "ext": "mp4",
                    "protocol": "m3u8_native",
                    "quality": "hd",
                    "resolution": "720x1280",
                    "width": 720,
                    "height": 1280,
                    "vcodec": "h264",
                    "acodec": "aac",
                    "tbr": 1800
                }
            ],
            "_youwee_page_hydration": [sigi_state]
        });

        let formats = formats_from_ytdlp_json(&json);
        let selected = select_format(
            &formats,
            &Some("origin".to_string()),
            &Some("auto".to_string()),
        )
        .expect("selected");

        assert!(
            formats
                .iter()
                .any(|format| format.variant.format_id == "uhd_60-flv")
        );
        assert_eq!(selected.variant.format_id, "uhd_60-flv");
        assert_eq!(selected.variant.width, Some(1080));
        assert_eq!(selected.variant.height, Some(1920));
        assert_eq!(selected.variant.fps, None);
        assert_eq!(selected.variant.vcodec.as_deref(), Some("h265"));
        assert_eq!(
            tiktok_live_variant_label(&selected.variant),
            "uhd_60-flv · 1080x1920 · h265 · FLV · 4000 kbps"
        );
    }

    #[test]
    fn origin_quality_prefers_uhd_60_when_resolution_matches() {
        let hevc_stream_data = serde_json::json!({
            "data": {
                "origin": {
                    "main": {
                        "hls": "https://signed.example/origin.m3u8",
                        "sdk_params": "{\"resolution\":\"1080x1920\",\"vbitrate\":4593000,\"VCodec\":\"h265\"}"
                    }
                },
                "uhd_60": {
                    "main": {
                        "hls": "https://signed.example/uhd60.m3u8",
                        "flv": "https://signed.example/uhd60.flv",
                        "sdk_params": "{\"resolution\":\"1080x1920\",\"vbitrate\":4000000,\"VCodec\":\"h265\",\"stream_suffix\":\"uhd560\"}"
                    }
                }
            }
        });
        let sigi_state = serde_json::json!({
            "LiveRoom": {
                "liveRoomUserInfo": {
                    "liveRoom": {
                        "hevcStreamData": {
                            "pull_data": {
                                "stream_data": hevc_stream_data.to_string()
                            }
                        }
                    }
                }
            }
        });
        let json = serde_json::json!({
            "_youwee_page_hydration": [sigi_state]
        });

        let variants = variants_from_ytdlp_json(&json);
        let selected_variant = select_variant(
            &variants,
            &Some("origin".to_string()),
            &Some("auto".to_string()),
        )
        .expect("selected variant");
        assert_eq!(selected_variant.format_id, "uhd_60-flv");
        assert_eq!(selected_variant.fps, None);

        let formats = formats_from_ytdlp_json(&json);
        let selected_format = select_format(
            &formats,
            &Some("origin".to_string()),
            &Some("auto".to_string()),
        )
        .expect("selected format");
        assert_eq!(selected_format.variant.format_id, "uhd_60-flv");
        assert_eq!(selected_format.variant.fps, None);
        assert_eq!(
            tiktok_live_variant_label(&selected_format.variant),
            "uhd_60-flv · 1080x1920 · h265 · FLV · 4000 kbps"
        );
    }

    #[test]
    fn extracts_tiktok_live_stream_data_from_sigi_html() {
        let stream_data = serde_json::json!({
            "data": {
                "uhd_60": {
                    "main": {
                        "flv": "https://signed.example/uhd60.flv",
                        "sdk_params": "{\"resolution\":\"1080x1920\",\"vbitrate\":4000000,\"VCodec\":\"h265\"}"
                    }
                }
            }
        });
        let sigi_state = serde_json::json!({
            "LiveRoom": {
                "liveRoomUserInfo": {
                    "liveRoom": {
                        "hevcStreamData": {
                            "pull_data": {
                                "stream_data": stream_data.to_string()
                            }
                        }
                    }
                }
            }
        });
        let html = format!(
            r#"<html><body><script id="SIGI_STATE" type="application/json">{sigi_state}</script></body></html>"#
        );

        let values = tiktok_page_hydration_values_from_html(&html);

        assert_eq!(values.len(), 1);
        assert_eq!(tiktok_stream_data_values_from_json(&values[0]).len(), 1);
    }

    #[test]
    fn falls_back_to_legacy_room_stream_urls() {
        let json = serde_json::json!({
            "data": {
                "stream_url": {
                    "flv_pull_url": {
                        "HD1": "https://signed.example/live.flv?token=secret"
                    }
                }
            }
        });

        let variants = variants_from_ytdlp_json(&json);
        let rendered = serde_json::to_string(&variants).expect("json");

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].format_id, "legacy-HD1");
        assert!(!rendered.contains("signed.example"));
    }

    #[test]
    fn extracts_room_owner_username_for_cookie_backed_live_url() {
        let json = serde_json::json!({
            "data": {
                "owner": {
                    "display_id": "some.user"
                }
            }
        });

        assert_eq!(room_owner_username(&json).as_deref(), Some("some.user"));
    }

    #[test]
    fn selects_highest_variant_by_default() {
        let low = TikTokLiveVariant {
            format_id: "low".to_string(),
            ext: Some("mp4".to_string()),
            protocol: Some("https".to_string()),
            quality: Some("sd".to_string()),
            resolution: Some("640x360".to_string()),
            width: Some(640),
            height: Some(360),
            fps: None,
            vcodec: None,
            acodec: None,
            tbr: Some(800.0),
            note: None,
        };
        let high = TikTokLiveVariant {
            format_id: "high".to_string(),
            width: Some(1920),
            height: Some(1080),
            tbr: Some(4500.0),
            protocol: Some("m3u8_native".to_string()),
            ..low.clone()
        };

        let selected = select_variant(&[low, high], &None, &None).expect("selected");
        assert_eq!(selected.format_id, "high");
    }

    #[test]
    fn auto_prefers_muxed_video_audio_over_audio_only_tiktok_live_format() {
        let audio_only = TikTokLiveFormat {
            variant: TikTokLiveVariant {
                format_id: "audio".to_string(),
                ext: Some("m4a".to_string()),
                protocol: Some("https".to_string()),
                quality: Some("ao".to_string()),
                resolution: Some("audio only".to_string()),
                width: None,
                height: None,
                fps: None,
                vcodec: Some("none".to_string()),
                acodec: Some("aac".to_string()),
                tbr: Some(12_000.0),
                note: None,
            },
            url: "https://signed.example/audio.m4a".to_string(),
            http_headers: serde_json::Map::new(),
        };
        let muxed = TikTokLiveFormat {
            variant: TikTokLiveVariant {
                format_id: "hd-hls".to_string(),
                ext: Some("m3u8".to_string()),
                protocol: Some("hls".to_string()),
                quality: Some("hd".to_string()),
                resolution: Some("1280x720".to_string()),
                width: Some(1280),
                height: Some(720),
                fps: Some(30.0),
                vcodec: Some("h264".to_string()),
                acodec: Some("aac".to_string()),
                tbr: Some(2500.0),
                note: None,
            },
            url: "https://signed.example/hd.m3u8".to_string(),
            http_headers: serde_json::Map::new(),
        };

        let selected = select_format(
            &[audio_only, muxed],
            &Some("auto".to_string()),
            &Some("auto".to_string()),
        )
        .expect("selected");

        assert_eq!(selected.variant.format_id, "hd-hls");
    }

    #[test]
    fn auto_prefers_muxed_video_audio_over_video_only_tiktok_live_format() {
        let video_only = TikTokLiveFormat {
            variant: TikTokLiveVariant {
                format_id: "uhd-video-only".to_string(),
                ext: Some("m3u8".to_string()),
                protocol: Some("hls".to_string()),
                quality: Some("uhd".to_string()),
                resolution: Some("1920x1080".to_string()),
                width: Some(1920),
                height: Some(1080),
                fps: Some(60.0),
                vcodec: Some("h264".to_string()),
                acodec: Some("none".to_string()),
                tbr: Some(8000.0),
                note: None,
            },
            url: "https://signed.example/uhd.m3u8".to_string(),
            http_headers: serde_json::Map::new(),
        };
        let muxed = TikTokLiveFormat {
            variant: TikTokLiveVariant {
                format_id: "hd-muxed".to_string(),
                ext: Some("m3u8".to_string()),
                protocol: Some("hls".to_string()),
                quality: Some("hd".to_string()),
                resolution: Some("1280x720".to_string()),
                width: Some(1280),
                height: Some(720),
                fps: Some(30.0),
                vcodec: Some("h264".to_string()),
                acodec: Some("aac".to_string()),
                tbr: Some(2500.0),
                note: None,
            },
            url: "https://signed.example/hd.m3u8".to_string(),
            http_headers: serde_json::Map::new(),
        };

        let selected = select_format(
            &[video_only, muxed],
            &Some("auto".to_string()),
            &Some("auto".to_string()),
        )
        .expect("selected");

        assert_eq!(selected.variant.format_id, "hd-muxed");
    }

    #[test]
    fn retries_transient_metadata_errors_but_not_offline_streams() {
        let timeout = BackendError::new(code::NETWORK_TIMEOUT, "temporary timeout")
            .with_retryable(true)
            .to_wire_string();
        let offline = BackendError::new(code::TIKTOK_LIVE_OFFLINE, "offline")
            .with_retryable(false)
            .to_wire_string();

        assert!(should_retry_metadata_error(&timeout));
        assert!(!should_retry_metadata_error(&offline));
        assert_eq!(metadata_retry_delay(1), Duration::from_millis(750));
        assert_eq!(metadata_retry_delay(2), Duration::from_millis(1500));
        assert!(tiktok_live_metadata_is_offline(
            &serde_json::json!({ "is_live": false })
        ));
        assert!(tiktok_live_metadata_is_offline(
            &serde_json::json!({ "live_status": "ended" })
        ));
        assert!(!tiktok_live_metadata_is_offline(
            &serde_json::json!({ "is_live": true, "live_status": "is_live" })
        ));
    }

    #[test]
    fn auto_reconnect_adds_bounded_ffmpeg_http_retries() {
        let mut enabled = Vec::new();
        append_reconnect_args(&mut enabled, true);

        assert!(enabled.windows(2).any(|args| args == ["-reconnect", "1"]));
        assert!(
            enabled
                .windows(2)
                .any(|args| args == ["-reconnect_max_retries", "20"])
        );
        assert!(
            enabled
                .windows(2)
                .any(|args| args == ["-reconnect_delay_total_max", "120"])
        );
        assert!(
            enabled
                .windows(2)
                .any(|args| args == ["-reconnect_on_http_error", "408,429,5xx"])
        );

        let mut disabled = Vec::new();
        append_reconnect_args(&mut disabled, false);
        assert!(disabled.is_empty());
    }

    #[test]
    fn prefers_flv_over_hls_when_resolution_matches() {
        let stream_data = serde_json::json!({
            "data": {
                "hd": {
                    "main": {
                        "flv": "https://signed.example/hd.flv",
                        "hls": "https://signed.example/hd.m3u8",
                        "lls": "https://signed.example/hd-lls.m3u8",
                        "sdk_params": "{\"resolution\":\"1280x720\",\"vbitrate\":2500000}"
                    }
                }
            }
        });

        let variants: Vec<TikTokLiveVariant> = formats_from_tiktok_stream_data(&stream_data)
            .into_iter()
            .map(|format| format.variant)
            .collect();
        let selected = select_variant(&variants, &None, &None).expect("selected");

        assert_eq!(selected.format_id, "hd-flv");
    }

    #[test]
    fn treats_hls_filter_as_m3u8_protocol() {
        let variant = TikTokLiveVariant {
            format_id: "hls".to_string(),
            ext: Some("mp4".to_string()),
            protocol: Some("m3u8_native".to_string()),
            quality: None,
            resolution: None,
            width: None,
            height: None,
            fps: None,
            vcodec: None,
            acodec: None,
            tbr: None,
            note: None,
        };

        assert!(matches_transport(&variant, &Some("hls".to_string())));
    }

    #[test]
    fn adds_default_tiktok_headers_for_ffmpeg_without_overwriting_cookies() {
        let mut headers = serde_json::Map::new();
        headers.insert(
            "Cookie".to_string(),
            serde_json::Value::String("sessionid=secret".to_string()),
        );

        let headers = tiktok_ffmpeg_headers(&headers);
        let block = ffmpeg_header_block(&headers).expect("headers");

        assert_eq!(
            header_value(&headers, "User-Agent").as_deref(),
            Some(TIKTOK_BROWSER_USER_AGENT)
        );
        assert!(block.contains("Origin: https://www.tiktok.com"));
        assert!(block.contains("Referer: https://www.tiktok.com/"));
        assert!(block.contains("Cookie: sessionid=secret"));
    }

    #[test]
    fn parses_tiktok_cookie_header_from_netscape_file() {
        let path = std::env::temp_dir().join(format!(
            "youwee-tiktok-cookie-test-{}.txt",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            ".tiktok.com\tTRUE\t/\tTRUE\t0\tsessionid\tabc\n.example.com\tTRUE\t/\tTRUE\t0\tother\tnope\n",
        )
        .expect("write cookie file");

        let header =
            tiktok_cookie_header_from_netscape_file(path.to_str().expect("path"), "www.tiktok.com")
                .expect("cookie header");
        std::fs::remove_file(path).ok();

        assert_eq!(header, "sessionid=abc");
    }

    #[test]
    fn sqlite_sidecar_path_appends_sqlite_wal_suffix() {
        assert_eq!(
            sqlite_sidecar_path(Path::new("cookies.sqlite"), "wal"),
            PathBuf::from("cookies.sqlite-wal")
        );
    }

    #[test]
    fn copies_and_removes_sqlite_sidecars() {
        let dir =
            std::env::temp_dir().join(format!("youwee-sqlite-copy-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let source = dir.join("cookies.sqlite");
        let dest = dir.join("copy.sqlite");
        std::fs::write(&source, "db").expect("write db");
        std::fs::write(sqlite_sidecar_path(&source, "wal"), "wal").expect("write wal");
        std::fs::write(sqlite_sidecar_path(&source, "shm"), "shm").expect("write shm");

        assert!(copy_sqlite_with_sidecars(&source, &dest));
        assert_eq!(std::fs::read_to_string(&dest).ok().as_deref(), Some("db"));
        assert_eq!(
            std::fs::read_to_string(sqlite_sidecar_path(&dest, "wal"))
                .ok()
                .as_deref(),
            Some("wal")
        );
        assert_eq!(
            std::fs::read_to_string(sqlite_sidecar_path(&dest, "shm"))
                .ok()
                .as_deref(),
            Some("shm")
        );

        remove_sqlite_copy(&dest);
        assert!(!dest.exists());
        assert!(!sqlite_sidecar_path(&dest, "wal").exists());
        assert!(!sqlite_sidecar_path(&dest, "shm").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn sanitizes_recording_filename_parts() {
        assert_eq!(
            sanitize_filename_part("  TikTok <LIVE>: test?  ", "fallback"),
            "TikTok _LIVE__ test_"
        );
        assert_eq!(sanitize_filename_part("...", "fallback"), "fallback");
    }

    #[test]
    fn creates_ordered_segment_paths_and_escaped_concat_manifest() {
        let output = PathBuf::from(r"C:\Videos\creator's live.mp4");
        let first = segment_path_for_recording(&output, 1);
        let second = segment_path_for_recording(&output, 2);

        assert_eq!(
            first.file_name().and_then(|value| value.to_str()),
            Some("creator's live.part-001.mkv")
        );
        assert_eq!(
            second.file_name().and_then(|value| value.to_str()),
            Some("creator's live.part-002.mkv")
        );

        let manifest = ffconcat_content(&[first, second]);
        assert!(manifest.contains("creator'\\''s live.part-001.mkv"));
        assert!(manifest.contains("creator'\\''s live.part-002.mkv"));
        assert_eq!(manifest.lines().count(), 2);
    }

    #[test]
    fn records_crash_safe_matroska_segments_before_mp4_finalization() {
        let selected = TikTokLiveFormat {
            variant: TikTokLiveVariant {
                format_id: "best".to_string(),
                ext: Some("m3u8".to_string()),
                protocol: Some("m3u8_native".to_string()),
                quality: None,
                resolution: None,
                width: None,
                height: None,
                fps: None,
                vcodec: Some("h264".to_string()),
                acodec: Some("aac".to_string()),
                tbr: None,
                note: None,
            },
            url: "https://signed.example/live.m3u8".to_string(),
            http_headers: serde_json::Map::new(),
        };
        let output = PathBuf::from(r"C:\Videos\live.part-001.mkv");
        let args = build_ffmpeg_record_args(&selected, None, None, true, &output);

        assert!(args.windows(2).any(|args| args == ["-c", "copy"]));
        assert!(args.windows(2).any(|args| args == ["-f", "matroska"]));
        assert!(
            args.windows(2)
                .any(|args| args == ["-cluster_time_limit", "2000"])
        );
        assert!(!args.iter().any(|arg| arg == "+faststart"));
        assert_eq!(args.last().map(String::as_str), output.to_str());
    }

    #[test]
    fn keeps_recoverable_container_extension_when_mp4_finalization_fails() {
        let output = PathBuf::from(r"C:\Videos\creator live.mp4");
        let recoverable = recoverable_output_path_for_recording(&output);

        assert_eq!(recoverable, PathBuf::from(r"C:\Videos\creator live.mkv"));
        assert_eq!(media_extension(&recoverable), "mkv");
        assert_eq!(media_extension(&output), "mp4");
    }

    #[tokio::test]
    async fn multi_segment_finalization_failure_keeps_every_segment_recoverable() {
        let dir = std::env::temp_dir().join(format!(
            "youwee-tiktok-finalize-fallback-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        let output = dir.join("creator live.mp4");
        let first = segment_path_for_recording(&output, 1);
        let second = segment_path_for_recording(&output, 2);
        std::fs::write(&first, b"first recoverable segment").expect("write first segment");
        std::fs::write(&second, b"second recoverable segment").expect("write second segment");
        let (_cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();

        let error = finalize_recording_segments(
            &dir.join("missing-ffmpeg.exe"),
            &[first.clone(), second.clone()],
            &output,
            &mut cancel_rx,
        )
        .await
        .expect_err("keep multi-segment job recoverable");

        assert!(
            BackendError::from_message(&error)
                .message()
                .contains("preserved for recovery")
        );
        assert_eq!(
            std::fs::read(&first).ok().as_deref(),
            Some(&b"first recoverable segment"[..])
        );
        assert!(second.exists());
        assert!(!output.exists());
        assert!(!recoverable_output_path_for_recording(&output).exists());
        assert!(!concat_list_path_for_recording(&output).exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn single_segment_finalization_failure_surfaces_the_whole_mkv() {
        let dir = std::env::temp_dir().join(format!(
            "youwee-tiktok-single-finalize-fallback-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        let output = dir.join("creator live.mp4");
        let segment = segment_path_for_recording(&output, 1);
        std::fs::write(&segment, b"complete recoverable segment").expect("write segment");
        let (_cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();

        let (final_path, partial) = finalize_recording_segments(
            &dir.join("missing-ffmpeg.exe"),
            &[segment],
            &output,
            &mut cancel_rx,
        )
        .await
        .expect("surface complete fallback");

        assert!(partial);
        assert_eq!(final_path, recoverable_output_path_for_recording(&output));
        assert_eq!(
            std::fs::read(&final_path).ok().as_deref(),
            Some(&b"complete recoverable segment"[..])
        );
        std::fs::remove_dir_all(dir).ok();
    }

    fn sample_recovery_job(output_dir: &Path, status: TikTokLiveJobStatus) -> TikTokLiveJob {
        let output_path = output_dir.join("creator_20260710_120000.mp4");
        TikTokLiveJob {
            id: uuid::Uuid::new_v4().to_string(),
            target_input: "@creator".to_string(),
            target_url: "https://www.tiktok.com/@creator/live".to_string(),
            username: Some("creator".to_string()),
            title: "TikTok LIVE @creator".to_string(),
            thumbnail: None,
            output_dir: output_dir.to_string_lossy().to_string(),
            output_path: Some(output_path.to_string_lossy().to_string()),
            final_path: None,
            preferred_quality: Some("auto".to_string()),
            preferred_transport: Some("auto".to_string()),
            duration_seconds: None,
            cookie_mode: Some("browser".to_string()),
            cookie_browser: Some("firefox".to_string()),
            cookie_browser_profile: Some("i879pxds.default-release".to_string()),
            cookie_file_path: None,
            auto_reconnect: true,
            status,
            segment_paths: vec![
                segment_path_for_recording(&output_path, 1)
                    .to_string_lossy()
                    .to_string(),
            ],
            refresh_count: 2,
            reconnect_count: 2,
            format_id: Some("best".to_string()),
            history_id: Some("tiktok-live:test".to_string()),
            error_message: None,
            started_at: 1,
            updated_at: 2,
            completed_at: None,
        }
    }

    #[test]
    fn recovery_keeps_fallback_mkv_before_remaining_numbered_segments() {
        let dir = std::env::temp_dir().join(format!(
            "youwee-tiktok-fallback-order-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        let mut job = sample_recovery_job(&dir, TikTokLiveJobStatus::Finalizing);
        let output = PathBuf::from(job.output_path.as_deref().expect("output path"));
        let fallback = recoverable_output_path_for_recording(&output);
        let second = segment_path_for_recording(&output, 2);
        job.segment_paths.push(second.to_string_lossy().to_string());
        job.final_path = Some(fallback.to_string_lossy().to_string());
        std::fs::write(&fallback, b"first segment").expect("write fallback");
        std::fs::write(&second, b"second segment").expect("write second");

        assert_eq!(recoverable_segment_paths(&job), vec![fallback, second]);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn startup_reconciliation_marks_stale_jobs_by_media_presence() {
        use crate::database::{DB_CONNECTION, db_test_guard, get_db};
        use std::sync::Mutex as StdMutex;

        let _guard = db_test_guard();
        if DB_CONNECTION.get().is_none() {
            let connection = Connection::open_in_memory().expect("open in-memory database");
            let _ = DB_CONNECTION.set(StdMutex::new(connection));
        }
        let connection = get_db().expect("get database");
        crate::database::init_tiktok_live_jobs_table(&connection).expect("create jobs table");
        connection
            .execute("DELETE FROM tiktok_live_jobs", [])
            .expect("clear jobs");
        drop(connection);

        let dir = std::env::temp_dir().join(format!(
            "youwee-tiktok-reconcile-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        let recoverable = sample_recovery_job(&dir, TikTokLiveJobStatus::Recording);
        std::fs::write(&recoverable.segment_paths[0], b"recoverable media")
            .expect("write recoverable segment");
        let interrupted =
            sample_recovery_job(&dir.join("missing"), TikTokLiveJobStatus::Reconnecting);
        save_tiktok_live_job_internal(&recoverable).expect("save recoverable job");
        save_tiktok_live_job_internal(&interrupted).expect("save interrupted job");

        assert_eq!(
            reconcile_tiktok_live_jobs_after_restart().expect("reconcile"),
            2
        );
        assert_eq!(
            get_tiktok_live_job_internal(&recoverable.id)
                .expect("load recoverable")
                .expect("recoverable exists")
                .status,
            TikTokLiveJobStatus::Recoverable
        );
        assert_eq!(
            get_tiktok_live_job_internal(&interrupted.id)
                .expect("load interrupted")
                .expect("interrupted exists")
                .status,
            TikTokLiveJobStatus::Interrupted
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn recovery_delete_paths_are_limited_to_generated_job_files() {
        let dir = std::env::temp_dir().join(format!(
            "youwee-tiktok-delete-safety-test-{}",
            uuid::Uuid::new_v4()
        ));
        let mut job = sample_recovery_job(&dir, TikTokLiveJobStatus::Recoverable);
        let paths = job_owned_paths(&job).expect("generated paths are accepted");
        assert!(
            paths
                .iter()
                .all(|path| path.parent() == Some(dir.as_path()))
        );

        job.segment_paths = vec![dir.join(r"..\unrelated.mkv").to_string_lossy().to_string()];
        assert!(job_owned_paths(&job).is_err());
    }

    fn sample_watch_entry(status: TikTokLiveWatchStatus) -> TikTokLiveWatchEntry {
        TikTokLiveWatchEntry {
            id: "watch-creator".to_string(),
            target_input: "@creator".to_string(),
            target_url: "https://www.tiktok.com/@creator/live".to_string(),
            username: Some("creator".to_string()),
            enabled: true,
            auto_record: true,
            output_dir: "C:/Downloads".to_string(),
            preferred_quality: Some("auto".to_string()),
            preferred_transport: Some("auto".to_string()),
            duration_seconds: None,
            cookie_mode: Some("browser".to_string()),
            cookie_browser: Some("firefox".to_string()),
            cookie_browser_profile: Some("i879pxds.default-release".to_string()),
            cookie_file_path: None,
            poll_interval_seconds: 60,
            record_mode: TikTokLiveRecordMode::OncePerLive,
            cooldown_seconds: WATCHLIST_DEFAULT_COOLDOWN_SECONDS,
            filename_template: None,
            schedule_enabled: false,
            schedule_days: None,
            schedule_start_minute: None,
            schedule_end_minute: None,
            backoff_attempt: 0,
            next_check_at: 100,
            status,
            active_job_id: None,
            last_error: None,
            last_checked_at: None,
            last_online_at: None,
            last_recording_at: None,
            last_session_id: None,
            last_outcome: None,
            last_completed_at: None,
            last_started_job_id: None,
            last_segment_count: 0,
            last_refresh_count: 0,
            last_reconnect_count: 0,
            last_file_size: None,
            created_at: 100,
            updated_at: 100,
        }
    }

    #[test]
    fn telegram_watchlist_reply_formats_backend_entries() {
        let mut entry = sample_watch_entry(TikTokLiveWatchStatus::Recording);
        entry.last_outcome = Some("recording".to_string());
        entry.last_segment_count = 2;
        entry.last_reconnect_count = 1;

        let reply = build_tiktok_live_telegram_watchlist_reply(&[entry]);

        assert!(reply.contains("📺 TikTok Live watchlist (1)"));
        assert!(reply.contains("🔴 @creator"));
        assert!(reply.contains("Status: recording"));
        assert!(reply.contains("Segments: 2"));
        assert!(reply.contains("Reconnects: 1"));
    }

    #[test]
    fn telegram_status_reply_matches_targets_like_frontend_hook() {
        let mut entry = sample_watch_entry(TikTokLiveWatchStatus::Online);
        entry.last_checked_at = Some(1_783_750_000);
        let config = TikTokLiveRecorderConfig {
            max_concurrent_recordings: 2,
            active_recordings: 1,
            hard_limit: TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT,
        };

        assert!(tiktok_live_telegram_target_matches(&entry, "@creator"));
        assert!(tiktok_live_telegram_target_matches(
            &entry,
            "https://www.tiktok.com/@creator/live"
        ));
        assert!(tiktok_live_telegram_target_matches(&entry, "watch-creator"));

        let target_reply =
            build_tiktok_live_telegram_status_reply(&[entry.clone()], &config, Some("@creator"));
        assert!(target_reply.contains("🟢 @creator"));
        assert!(target_reply.contains("Status: online"));
        assert!(target_reply.contains("Enabled: yes"));

        let summary_reply = build_tiktok_live_telegram_status_reply(&[entry], &config, None);
        assert!(summary_reply.contains("Active rooms: 1/2"));
        assert!(summary_reply.contains("Watchlist: 1 total · 1 enabled"));
        assert!(summary_reply.contains("Online: 1"));
    }

    #[test]
    fn watchlist_backoff_is_bounded_deterministic_and_respects_poll_floor() {
        assert_eq!(clamp_watchlist_poll_interval(Some(1)), 30);
        assert_eq!(clamp_watchlist_poll_interval(Some(120)), 120);
        assert_eq!(clamp_watchlist_poll_interval(Some(10_000)), 3600);
        let first = watchlist_backoff_seconds("watch-creator", 60, 1);
        assert_eq!(first, watchlist_backoff_seconds("watch-creator", 60, 1));
        assert!((60..72).contains(&first));
        assert_eq!(watchlist_backoff_seconds("watch-creator", 60, 20), 1800);
        assert_eq!(watchlist_backoff_seconds("watch-creator", 3600, 1), 3600);
    }

    #[test]
    fn watchlist_auto_record_only_starts_on_a_free_offline_to_live_transition() {
        let mut entry = sample_watch_entry(TikTokLiveWatchStatus::Offline);
        assert!(should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Offline,
            &entry,
            true,
            Some("session-1"),
            1_000
        ));
        assert!(!should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Online,
            &entry,
            true,
            Some("session-1"),
            1_000
        ));
        entry.last_session_id = Some("session-1".to_string());
        entry.enabled = false;
        assert!(!should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Offline,
            &entry,
            true,
            Some("session-1"),
            1_000
        ));
        entry.enabled = true;
        entry.active_job_id = Some("existing-job".to_string());
        assert!(!should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Offline,
            &entry,
            true,
            Some("session-2"),
            1_000
        ));
    }

    #[test]
    fn watchlist_record_modes_dedupe_by_session_and_cooldown() {
        let mut entry = sample_watch_entry(TikTokLiveWatchStatus::Offline);
        entry.last_session_id = Some("session-1".to_string());
        assert!(!should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Offline,
            &entry,
            true,
            Some("session-1"),
            2_000
        ));
        assert!(should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Offline,
            &entry,
            true,
            Some("session-2"),
            2_000
        ));

        entry.record_mode = TikTokLiveRecordMode::AlwaysAfterCooldown;
        entry.last_recording_at = Some(1_000);
        entry.cooldown_seconds = 600;
        assert!(!should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Online,
            &entry,
            true,
            Some("session-1"),
            1_500
        ));
        assert!(should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Online,
            &entry,
            true,
            Some("session-1"),
            1_600
        ));

        entry.record_mode = TikTokLiveRecordMode::ManualOnly;
        assert!(!should_auto_record_watch_entry(
            TikTokLiveWatchStatus::Offline,
            &entry,
            true,
            Some("session-3"),
            3_000
        ));
    }

    #[test]
    fn watchlist_busy_live_state_requires_a_new_offline_transition() {
        let mut entry = sample_watch_entry(TikTokLiveWatchStatus::Offline);
        mark_watch_entry_live_but_busy(&mut entry, 1_000);

        assert_eq!(entry.status, TikTokLiveWatchStatus::Online);
        assert_eq!(entry.last_error.as_deref(), Some("recordingBusy"));
        assert!(!should_auto_record_watch_entry(
            entry.status,
            &entry,
            true,
            Some("session-1"),
            1_000
        ));
    }

    #[test]
    fn watchlist_schedule_rules_normalize_and_match_windows() {
        assert_eq!(
            normalize_schedule_days(Some(" 2,1,2,9,x,0 ".to_string())).as_deref(),
            Some("0,1,2")
        );
        assert_eq!(normalize_schedule_minute(Some(1439)), Some(1439));
        assert_eq!(normalize_schedule_minute(Some(1440)), None);
        assert!(schedule_days_contains(Some("0,2,4"), 2));
        assert!(!schedule_days_contains(Some("0,2,4"), 3));
        assert!(schedule_window_contains(Some(60), Some(120), 90));
        assert!(!schedule_window_contains(Some(60), Some(120), 120));
        assert!(schedule_window_contains(Some(1320), Some(120), 30));
        assert!(schedule_window_contains(Some(1320), Some(120), 1380));
        assert!(!schedule_window_contains(Some(1320), Some(120), 600));
        assert_eq!(tiktok_live_resource_warning(0, 1), None);
        assert_eq!(tiktok_live_resource_warning(1, 2), Some("limitHigh"));
        assert_eq!(tiktok_live_resource_warning(2, 2), Some("multiRoomActive"));
    }

    #[tokio::test]
    async fn global_tiktok_live_reservation_is_atomic() {
        let _limit_guard = RECORDING_LIMIT_TEST_LOCK
            .lock()
            .expect("lock recording limit");
        TIKTOK_LIVE_MAX_RECORDINGS.store(1, Ordering::SeqCst);
        let first_id = format!("watch-reservation-first-{}", uuid::Uuid::new_v4());
        let second_id = format!("watch-reservation-second-{}", uuid::Uuid::new_v4());
        let first_cancel = reserve_tiktok_live_recording(&first_id)
            .await
            .expect("reserve first recording");

        let second_error = reserve_tiktok_live_recording(&second_id)
            .await
            .expect_err("reject concurrent recording reservation");
        assert!(
            BackendError::from_message(&second_error)
                .message()
                .contains("configured room limit")
        );

        cancel_tiktok_live_recording(first_id.clone())
            .await
            .expect("signal first cancellation");
        assert!(first_cancel.await.is_ok());
        let while_stopping_error = reserve_tiktok_live_recording(&second_id)
            .await
            .expect_err("keep slot reserved while cancellation finishes");
        assert!(
            BackendError::from_message(&while_stopping_error)
                .message()
                .contains("configured room limit")
        );

        release_tiktok_live_recording(&first_id).await;
        let second_cancel = reserve_tiktok_live_recording(&second_id)
            .await
            .expect("reserve after first releases");
        release_tiktok_live_recording(&second_id).await;
        drop(second_cancel);
        TIKTOK_LIVE_MAX_RECORDINGS.store(1, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn global_tiktok_live_reservation_allows_configured_multi_room_limit() {
        let _limit_guard = RECORDING_LIMIT_TEST_LOCK
            .lock()
            .expect("lock recording limit");
        TIKTOK_LIVE_MAX_RECORDINGS.store(2, Ordering::SeqCst);
        let first_id = format!("watch-multi-first-{}", uuid::Uuid::new_v4());
        let second_id = format!("watch-multi-second-{}", uuid::Uuid::new_v4());
        let third_id = format!("watch-multi-third-{}", uuid::Uuid::new_v4());

        let first_cancel = reserve_tiktok_live_recording(&first_id)
            .await
            .expect("reserve first recording");
        let second_cancel = reserve_tiktok_live_recording(&second_id)
            .await
            .expect("reserve second recording");
        let third_error = reserve_tiktok_live_recording(&third_id)
            .await
            .expect_err("reject over configured limit");
        assert!(
            BackendError::from_message(&third_error)
                .message()
                .contains("configured room limit")
        );

        release_tiktok_live_recording(&first_id).await;
        release_tiktok_live_recording(&second_id).await;
        drop(first_cancel);
        drop(second_cancel);
        TIKTOK_LIVE_MAX_RECORDINGS.store(1, Ordering::SeqCst);
    }

    #[test]
    fn startup_restores_persisted_recorder_limit_with_hard_cap() {
        use crate::database::{DB_CONNECTION, db_test_guard, get_db};
        use std::sync::Mutex as StdMutex;

        let _db_guard = db_test_guard();
        let _limit_guard = RECORDING_LIMIT_TEST_LOCK
            .lock()
            .expect("lock recording limit");
        if DB_CONNECTION.get().is_none() {
            let connection = Connection::open_in_memory().expect("open in-memory database");
            let _ = DB_CONNECTION.set(StdMutex::new(connection));
        }
        let connection = get_db().expect("get database");
        crate::database::init_tiktok_live_jobs_table(&connection).expect("create jobs table");
        connection
            .execute("DELETE FROM tiktok_live_recorder_config", [])
            .expect("clear recorder config");
        drop(connection);

        set_tiktok_live_recorder_limit_internal(99).expect("save oversized limit");
        TIKTOK_LIVE_MAX_RECORDINGS.store(1, Ordering::SeqCst);

        assert_eq!(
            load_tiktok_live_recorder_config_after_restart().expect("load recorder config"),
            TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT
        );
        assert_eq!(
            configured_tiktok_live_recording_limit(),
            TIKTOK_LIVE_MAX_RECORDINGS_HARD_LIMIT
        );
        TIKTOK_LIVE_MAX_RECORDINGS.store(1, Ordering::SeqCst);
    }

    #[test]
    fn watchlist_target_identity_ignores_username_case() {
        assert!(same_watch_target(
            "https://www.tiktok.com/@Creator/live",
            "https://www.tiktok.com/@creator/live"
        ));
        assert!(!same_watch_target(
            "https://www.tiktok.com/@creator/live",
            "https://www.tiktok.com/@another/live"
        ));
    }

    #[test]
    fn watchlist_restart_links_recoverable_job_without_starting_a_duplicate() {
        use crate::database::{DB_CONNECTION, db_test_guard, get_db};
        use std::sync::Mutex as StdMutex;

        let _guard = db_test_guard();
        if DB_CONNECTION.get().is_none() {
            let connection = Connection::open_in_memory().expect("open in-memory database");
            let _ = DB_CONNECTION.set(StdMutex::new(connection));
        }
        let connection = get_db().expect("get database");
        crate::database::init_tiktok_live_jobs_table(&connection).expect("create jobs table");
        crate::database::init_tiktok_live_watchlist_table(&connection)
            .expect("create watchlist table");
        connection
            .execute("DELETE FROM tiktok_live_jobs", [])
            .expect("clear jobs");
        connection
            .execute("DELETE FROM tiktok_live_watchlist", [])
            .expect("clear watchlist");
        drop(connection);

        let dir = std::env::temp_dir().join(format!(
            "youwee-tiktok-watch-reconcile-test-{}",
            uuid::Uuid::new_v4()
        ));
        let mut job = sample_recovery_job(&dir, TikTokLiveJobStatus::Recoverable);
        job.id = "watch-recovery-job".to_string();
        save_tiktok_live_job_internal(&job).expect("save recoverable job");
        let mut entry = sample_watch_entry(TikTokLiveWatchStatus::Recording);
        entry.active_job_id = Some(job.id.clone());
        save_tiktok_live_watch_entry_internal(&entry).expect("save watch entry");

        assert_eq!(
            reconcile_tiktok_live_watchlist_after_restart().expect("reconcile watchlist"),
            1
        );
        let loaded = get_tiktok_live_watch_entry_internal(&entry.id)
            .expect("load watch entry")
            .expect("watch entry exists");
        assert_eq!(loaded.status, TikTokLiveWatchStatus::Recoverable);
        assert_eq!(loaded.active_job_id.as_deref(), Some(job.id.as_str()));
        assert_eq!(loaded.next_check_at, WATCHLIST_PAUSED_CHECK_AT);
    }
}
