use super::connection::get_db;
use rusqlite::{params, types::Type, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TikTokLiveWatchStatus {
    Offline,
    Checking,
    Online,
    Recording,
    Backoff,
    Recoverable,
    Error,
}

impl TikTokLiveWatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Checking => "checking",
            Self::Online => "online",
            Self::Recording => "recording",
            Self::Backoff => "backoff",
            Self::Recoverable => "recoverable",
            Self::Error => "error",
        }
    }
}

impl TryFrom<&str> for TikTokLiveWatchStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, String> {
        match value {
            "offline" => Ok(Self::Offline),
            "checking" => Ok(Self::Checking),
            "online" => Ok(Self::Online),
            "recording" => Ok(Self::Recording),
            "backoff" => Ok(Self::Backoff),
            "recoverable" => Ok(Self::Recoverable),
            "error" => Ok(Self::Error),
            _ => Err(format!("Unknown TikTok Live watch status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TikTokLiveRecordMode {
    OncePerLive,
    AlwaysAfterCooldown,
    ManualOnly,
}

impl TikTokLiveRecordMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OncePerLive => "once_per_live",
            Self::AlwaysAfterCooldown => "always_after_cooldown",
            Self::ManualOnly => "manual_only",
        }
    }
}

impl Default for TikTokLiveRecordMode {
    fn default() -> Self {
        Self::OncePerLive
    }
}

impl TryFrom<&str> for TikTokLiveRecordMode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, String> {
        match value {
            "once_per_live" => Ok(Self::OncePerLive),
            "always_after_cooldown" => Ok(Self::AlwaysAfterCooldown),
            "manual_only" => Ok(Self::ManualOnly),
            _ => Err(format!("Unknown TikTok Live record mode: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveWatchEntry {
    pub id: String,
    pub target_input: String,
    pub target_url: String,
    pub username: Option<String>,
    pub enabled: bool,
    pub auto_record: bool,
    pub output_dir: String,
    pub preferred_quality: Option<String>,
    pub preferred_transport: Option<String>,
    pub duration_seconds: Option<u32>,
    pub cookie_mode: Option<String>,
    pub cookie_browser: Option<String>,
    pub cookie_browser_profile: Option<String>,
    pub cookie_file_path: Option<String>,
    pub poll_interval_seconds: u32,
    pub record_mode: TikTokLiveRecordMode,
    pub cooldown_seconds: u32,
    pub filename_template: Option<String>,
    pub schedule_enabled: bool,
    pub schedule_days: Option<String>,
    pub schedule_start_minute: Option<u32>,
    pub schedule_end_minute: Option<u32>,
    pub backoff_attempt: u32,
    pub next_check_at: i64,
    pub status: TikTokLiveWatchStatus,
    pub active_job_id: Option<String>,
    pub last_error: Option<String>,
    pub last_checked_at: Option<i64>,
    pub last_online_at: Option<i64>,
    pub last_recording_at: Option<i64>,
    pub last_session_id: Option<String>,
    pub last_outcome: Option<String>,
    pub last_completed_at: Option<i64>,
    pub last_started_job_id: Option<String>,
    pub last_segment_count: u32,
    pub last_refresh_count: u32,
    pub last_reconnect_count: u32,
    pub last_file_size: Option<u64>,
    pub last_title: Option<String>,
    pub last_uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub avatar: Option<String>,
    pub last_viewer_count: Option<u64>,
    pub created_at: i64,
    pub updated_at: i64,
}

const WATCH_COLUMNS: &str = "id, target_input, target_url, username, enabled, auto_record, output_dir, preferred_quality, preferred_transport, duration_seconds, cookie_mode, cookie_browser, cookie_browser_profile, cookie_file_path, poll_interval_seconds, backoff_attempt, next_check_at, status, active_job_id, last_error, last_checked_at, last_online_at, last_recording_at, created_at, updated_at, record_mode, cooldown_seconds, filename_template, last_session_id, last_outcome, last_completed_at, last_started_job_id, last_segment_count, last_refresh_count, last_reconnect_count, last_file_size, schedule_enabled, schedule_days, schedule_start_minute, schedule_end_minute, last_title, last_uploader, thumbnail, avatar, last_viewer_count";

pub(crate) fn init_tiktok_live_watchlist_table(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tiktok_live_watchlist (
            id TEXT PRIMARY KEY,
            target_input TEXT NOT NULL,
            target_url TEXT NOT NULL,
            username TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            auto_record INTEGER NOT NULL DEFAULT 1,
            output_dir TEXT NOT NULL,
            preferred_quality TEXT,
            preferred_transport TEXT,
            duration_seconds INTEGER,
            cookie_mode TEXT,
            cookie_browser TEXT,
            cookie_browser_profile TEXT,
            cookie_file_path TEXT,
            poll_interval_seconds INTEGER NOT NULL DEFAULT 60,
            record_mode TEXT NOT NULL DEFAULT 'once_per_live',
            cooldown_seconds INTEGER NOT NULL DEFAULT 3600,
            filename_template TEXT,
            backoff_attempt INTEGER NOT NULL DEFAULT 0,
            next_check_at INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'offline',
            active_job_id TEXT,
            last_error TEXT,
            last_checked_at INTEGER,
            last_online_at INTEGER,
            last_recording_at INTEGER,
            last_session_id TEXT,
            last_outcome TEXT,
            last_completed_at INTEGER,
            last_started_job_id TEXT,
            last_segment_count INTEGER NOT NULL DEFAULT 0,
            last_refresh_count INTEGER NOT NULL DEFAULT 0,
            last_reconnect_count INTEGER NOT NULL DEFAULT 0,
            last_file_size INTEGER,
            schedule_enabled INTEGER NOT NULL DEFAULT 0,
            schedule_days TEXT,
            schedule_start_minute INTEGER,
            schedule_end_minute INTEGER,
            last_title TEXT,
            last_uploader TEXT,
            thumbnail TEXT,
            avatar TEXT,
            last_viewer_count INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|error| format!("Failed to create TikTok Live watchlist table: {error}"))?;
    for migration in [
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN record_mode TEXT NOT NULL DEFAULT 'once_per_live'",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN cooldown_seconds INTEGER NOT NULL DEFAULT 3600",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN filename_template TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_session_id TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_outcome TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_completed_at INTEGER",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_started_job_id TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_segment_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_refresh_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_reconnect_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_file_size INTEGER",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN schedule_enabled INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN schedule_days TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN schedule_start_minute INTEGER",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN schedule_end_minute INTEGER",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_title TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_uploader TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN thumbnail TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN avatar TEXT",
        "ALTER TABLE tiktok_live_watchlist ADD COLUMN last_viewer_count INTEGER",
    ] {
        let _ = conn.execute(migration, []);
    }
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_tiktok_live_watchlist_target
         ON tiktok_live_watchlist(target_url COLLATE NOCASE)",
        [],
    )
    .map_err(|error| format!("Failed to create TikTok Live watchlist target index: {error}"))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tiktok_live_watchlist_due
         ON tiktok_live_watchlist(enabled, next_check_at)",
        [],
    )
    .map_err(|error| format!("Failed to create TikTok Live watchlist due index: {error}"))?;
    Ok(())
}

fn watch_entry_from_row(row: &Row<'_>) -> rusqlite::Result<TikTokLiveWatchEntry> {
    let raw_status: String = row.get(17)?;
    let status = TikTokLiveWatchStatus::try_from(raw_status.as_str()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            17,
            Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    })?;
    let raw_record_mode: String = row.get(25)?;
    let record_mode =
        TikTokLiveRecordMode::try_from(raw_record_mode.as_str()).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                25,
                Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?;

    Ok(TikTokLiveWatchEntry {
        id: row.get(0)?,
        target_input: row.get(1)?,
        target_url: row.get(2)?,
        username: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        auto_record: row.get::<_, i64>(5)? != 0,
        output_dir: row.get(6)?,
        preferred_quality: row.get(7)?,
        preferred_transport: row.get(8)?,
        duration_seconds: row.get::<_, Option<i64>>(9)?.map(|value| value as u32),
        cookie_mode: row.get(10)?,
        cookie_browser: row.get(11)?,
        cookie_browser_profile: row.get(12)?,
        cookie_file_path: row.get(13)?,
        poll_interval_seconds: row.get::<_, i64>(14)? as u32,
        record_mode,
        cooldown_seconds: row.get::<_, i64>(26)? as u32,
        filename_template: row.get(27)?,
        backoff_attempt: row.get::<_, i64>(15)? as u32,
        next_check_at: row.get(16)?,
        status,
        active_job_id: row.get(18)?,
        last_error: row.get(19)?,
        last_checked_at: row.get(20)?,
        last_online_at: row.get(21)?,
        last_recording_at: row.get(22)?,
        last_session_id: row.get(28)?,
        last_outcome: row.get(29)?,
        last_completed_at: row.get(30)?,
        last_started_job_id: row.get(31)?,
        last_segment_count: row.get::<_, i64>(32)? as u32,
        last_refresh_count: row.get::<_, i64>(33)? as u32,
        last_reconnect_count: row.get::<_, i64>(34)? as u32,
        last_file_size: row
            .get::<_, Option<i64>>(35)?
            .map(|value| value.max(0) as u64),
        schedule_enabled: row.get::<_, i64>(36)? != 0,
        schedule_days: row.get(37)?,
        schedule_start_minute: row.get::<_, Option<i64>>(38)?.map(|value| value as u32),
        schedule_end_minute: row.get::<_, Option<i64>>(39)?.map(|value| value as u32),
        last_title: row.get(40)?,
        last_uploader: row.get(41)?,
        thumbnail: row.get(42)?,
        avatar: row.get(43)?,
        last_viewer_count: row
            .get::<_, Option<i64>>(44)?
            .map(|value| value.max(0) as u64),
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
    })
}

fn persisted_target_input(entry: &TikTokLiveWatchEntry) -> &str {
    if entry.target_input.starts_with("http://") || entry.target_input.starts_with("https://") {
        &entry.target_url
    } else {
        &entry.target_input
    }
}

pub fn save_tiktok_live_watch_entry_internal(entry: &TikTokLiveWatchEntry) -> Result<(), String> {
    let conn = get_db()?;
    let persisted_target_input = persisted_target_input(entry);
    conn.execute(
        "INSERT INTO tiktok_live_watchlist (
            id, target_input, target_url, username, enabled, auto_record, output_dir,
            preferred_quality, preferred_transport, duration_seconds, cookie_mode,
            cookie_browser, cookie_browser_profile, cookie_file_path, poll_interval_seconds,
            backoff_attempt, next_check_at, status, active_job_id, last_error,
            last_checked_at, last_online_at, last_recording_at, created_at, updated_at,
            record_mode, cooldown_seconds, filename_template, last_session_id, last_outcome,
            last_completed_at, last_started_job_id, last_segment_count, last_refresh_count,
            last_reconnect_count, last_file_size, schedule_enabled, schedule_days,
            schedule_start_minute, schedule_end_minute, last_title, last_uploader, thumbnail,
            avatar, last_viewer_count
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
            ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40,
            ?41, ?42, ?43, ?44, ?45
         )
         ON CONFLICT(id) DO UPDATE SET
            target_input = excluded.target_input,
            target_url = excluded.target_url,
            username = excluded.username,
            enabled = excluded.enabled,
            auto_record = excluded.auto_record,
            output_dir = excluded.output_dir,
            preferred_quality = excluded.preferred_quality,
            preferred_transport = excluded.preferred_transport,
            duration_seconds = excluded.duration_seconds,
            cookie_mode = excluded.cookie_mode,
            cookie_browser = excluded.cookie_browser,
            cookie_browser_profile = excluded.cookie_browser_profile,
            cookie_file_path = excluded.cookie_file_path,
            poll_interval_seconds = excluded.poll_interval_seconds,
            backoff_attempt = excluded.backoff_attempt,
            next_check_at = excluded.next_check_at,
            status = excluded.status,
            active_job_id = excluded.active_job_id,
            last_error = excluded.last_error,
            last_checked_at = excluded.last_checked_at,
            last_online_at = excluded.last_online_at,
            last_recording_at = excluded.last_recording_at,
            record_mode = excluded.record_mode,
            cooldown_seconds = excluded.cooldown_seconds,
            filename_template = excluded.filename_template,
            last_session_id = excluded.last_session_id,
            last_outcome = excluded.last_outcome,
            last_completed_at = excluded.last_completed_at,
            last_started_job_id = excluded.last_started_job_id,
            last_segment_count = excluded.last_segment_count,
            last_refresh_count = excluded.last_refresh_count,
            last_reconnect_count = excluded.last_reconnect_count,
            last_file_size = excluded.last_file_size,
            schedule_enabled = excluded.schedule_enabled,
            schedule_days = excluded.schedule_days,
            schedule_start_minute = excluded.schedule_start_minute,
            schedule_end_minute = excluded.schedule_end_minute,
            last_title = excluded.last_title,
            last_uploader = excluded.last_uploader,
            thumbnail = excluded.thumbnail,
            avatar = excluded.avatar,
            last_viewer_count = excluded.last_viewer_count,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            entry.id,
            persisted_target_input,
            entry.target_url,
            entry.username,
            i64::from(entry.enabled),
            i64::from(entry.auto_record),
            entry.output_dir,
            entry.preferred_quality,
            entry.preferred_transport,
            entry.duration_seconds.map(i64::from),
            entry.cookie_mode,
            entry.cookie_browser,
            entry.cookie_browser_profile,
            entry.cookie_file_path,
            i64::from(entry.poll_interval_seconds),
            i64::from(entry.backoff_attempt),
            entry.next_check_at,
            entry.status.as_str(),
            entry.active_job_id,
            entry.last_error,
            entry.last_checked_at,
            entry.last_online_at,
            entry.last_recording_at,
            entry.created_at,
            entry.updated_at,
            entry.record_mode.as_str(),
            i64::from(entry.cooldown_seconds),
            entry.filename_template,
            entry.last_session_id,
            entry.last_outcome,
            entry.last_completed_at,
            entry.last_started_job_id,
            i64::from(entry.last_segment_count),
            i64::from(entry.last_refresh_count),
            i64::from(entry.last_reconnect_count),
            entry
                .last_file_size
                .and_then(|value| i64::try_from(value).ok()),
            i64::from(entry.schedule_enabled),
            entry.schedule_days,
            entry.schedule_start_minute.map(i64::from),
            entry.schedule_end_minute.map(i64::from),
            entry.last_title,
            entry.last_uploader,
            entry.thumbnail,
            entry.avatar,
            entry
                .last_viewer_count
                .and_then(|value| i64::try_from(value).ok()),
        ],
    )
    .map_err(|error| format!("Failed to save TikTok Live watchlist entry: {error}"))?;
    Ok(())
}

pub fn update_tiktok_live_watch_entry_internal(
    entry: &TikTokLiveWatchEntry,
) -> Result<bool, String> {
    let conn = get_db()?;
    let persisted_target_input = persisted_target_input(entry);
    let changed = conn
        .execute(
            "UPDATE tiktok_live_watchlist SET
                target_input = ?2,
                target_url = ?3,
                username = ?4,
                enabled = ?5,
                auto_record = ?6,
                output_dir = ?7,
                preferred_quality = ?8,
                preferred_transport = ?9,
                duration_seconds = ?10,
                cookie_mode = ?11,
                cookie_browser = ?12,
                cookie_browser_profile = ?13,
                cookie_file_path = ?14,
                poll_interval_seconds = ?15,
                backoff_attempt = ?16,
                next_check_at = ?17,
                status = ?18,
                active_job_id = ?19,
                last_error = ?20,
                last_checked_at = ?21,
                last_online_at = ?22,
                last_recording_at = ?23,
                created_at = ?24,
                updated_at = ?25,
                record_mode = ?26,
                cooldown_seconds = ?27,
                filename_template = ?28,
                last_session_id = ?29,
                last_outcome = ?30,
                last_completed_at = ?31,
                last_started_job_id = ?32,
                last_segment_count = ?33,
                last_refresh_count = ?34,
                last_reconnect_count = ?35,
                last_file_size = ?36,
                schedule_enabled = ?37,
                schedule_days = ?38,
                schedule_start_minute = ?39,
                schedule_end_minute = ?40,
                last_title = ?41,
                last_uploader = ?42,
                thumbnail = ?43,
                avatar = ?44,
                last_viewer_count = ?45
             WHERE id = ?1",
            params![
                entry.id,
                persisted_target_input,
                entry.target_url,
                entry.username,
                i64::from(entry.enabled),
                i64::from(entry.auto_record),
                entry.output_dir,
                entry.preferred_quality,
                entry.preferred_transport,
                entry.duration_seconds.map(i64::from),
                entry.cookie_mode,
                entry.cookie_browser,
                entry.cookie_browser_profile,
                entry.cookie_file_path,
                i64::from(entry.poll_interval_seconds),
                i64::from(entry.backoff_attempt),
                entry.next_check_at,
                entry.status.as_str(),
                entry.active_job_id,
                entry.last_error,
                entry.last_checked_at,
                entry.last_online_at,
                entry.last_recording_at,
                entry.created_at,
                entry.updated_at,
                entry.record_mode.as_str(),
                i64::from(entry.cooldown_seconds),
                entry.filename_template,
                entry.last_session_id,
                entry.last_outcome,
                entry.last_completed_at,
                entry.last_started_job_id,
                i64::from(entry.last_segment_count),
                i64::from(entry.last_refresh_count),
                i64::from(entry.last_reconnect_count),
                entry
                    .last_file_size
                    .and_then(|value| i64::try_from(value).ok()),
                i64::from(entry.schedule_enabled),
                entry.schedule_days,
                entry.schedule_start_minute.map(i64::from),
                entry.schedule_end_minute.map(i64::from),
                entry.last_title,
                entry.last_uploader,
                entry.thumbnail,
                entry.avatar,
                entry
                    .last_viewer_count
                    .and_then(|value| i64::try_from(value).ok()),
            ],
        )
        .map_err(|error| format!("Failed to update TikTok Live watchlist entry: {error}"))?;
    Ok(changed > 0)
}

pub fn get_tiktok_live_watch_entry_internal(
    id: &str,
) -> Result<Option<TikTokLiveWatchEntry>, String> {
    query_single_watch_entry("id = ?1", id)
}

pub fn get_tiktok_live_watch_entry_by_target_internal(
    target_url: &str,
) -> Result<Option<TikTokLiveWatchEntry>, String> {
    query_single_watch_entry("target_url = ?1 COLLATE NOCASE", target_url)
}

pub fn get_tiktok_live_watch_entry_by_active_job_internal(
    job_id: &str,
) -> Result<Option<TikTokLiveWatchEntry>, String> {
    query_single_watch_entry("active_job_id = ?1", job_id)
}

fn query_single_watch_entry(
    predicate: &str,
    value: &str,
) -> Result<Option<TikTokLiveWatchEntry>, String> {
    let conn = get_db()?;
    let mut statement = conn
        .prepare(&format!(
            "SELECT {WATCH_COLUMNS} FROM tiktok_live_watchlist WHERE {predicate}"
        ))
        .map_err(|error| format!("Failed to prepare TikTok Live watchlist query: {error}"))?;
    let mut rows = statement
        .query(params![value])
        .map_err(|error| format!("Failed to query TikTok Live watchlist: {error}"))?;
    rows.next()
        .map_err(|error| format!("Failed to read TikTok Live watchlist: {error}"))?
        .map(watch_entry_from_row)
        .transpose()
        .map_err(|error| format!("Failed to decode TikTok Live watchlist: {error}"))
}

pub fn get_tiktok_live_watchlist_internal() -> Result<Vec<TikTokLiveWatchEntry>, String> {
    query_watch_entries(
        "SELECT {columns} FROM tiktok_live_watchlist ORDER BY created_at ASC",
        None,
    )
}

pub fn get_due_tiktok_live_watchlist_internal(
    now: i64,
) -> Result<Vec<TikTokLiveWatchEntry>, String> {
    query_watch_entries(
        "SELECT {columns} FROM tiktok_live_watchlist
         WHERE enabled = 1 AND next_check_at <= ?1
         ORDER BY next_check_at ASC",
        Some(now),
    )
}

fn query_watch_entries(
    sql: &str,
    timestamp: Option<i64>,
) -> Result<Vec<TikTokLiveWatchEntry>, String> {
    let conn = get_db()?;
    let sql = sql.replace("{columns}", WATCH_COLUMNS);
    let mut statement = conn
        .prepare(&sql)
        .map_err(|error| format!("Failed to prepare TikTok Live watchlist query: {error}"))?;
    let rows = match timestamp {
        Some(value) => statement.query_map(params![value], watch_entry_from_row),
        None => statement.query_map([], watch_entry_from_row),
    }
    .map_err(|error| format!("Failed to query TikTok Live watchlist: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to decode TikTok Live watchlist: {error}"))
}

pub fn delete_tiktok_live_watch_entry_internal(id: &str) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "DELETE FROM tiktok_live_watchlist WHERE id = ?1",
        params![id],
    )
    .map_err(|error| format!("Failed to delete TikTok Live watchlist entry: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{db_test_guard, DB_CONNECTION};
    use std::sync::Mutex;

    fn ensure_test_table() {
        if DB_CONNECTION.get().is_none() {
            let connection = Connection::open_in_memory().expect("open in-memory database");
            let _ = DB_CONNECTION.set(Mutex::new(connection));
        }
        let connection = get_db().expect("get database");
        init_tiktok_live_watchlist_table(&connection).expect("create watchlist table");
        connection
            .execute("DELETE FROM tiktok_live_watchlist", [])
            .expect("clear watchlist");
    }

    fn sample_entry(id: &str, target_url: &str) -> TikTokLiveWatchEntry {
        TikTokLiveWatchEntry {
            id: id.to_string(),
            target_input: "@creator".to_string(),
            target_url: target_url.to_string(),
            username: Some("creator".to_string()),
            enabled: true,
            auto_record: true,
            output_dir: "C:/Downloads".to_string(),
            preferred_quality: Some("auto".to_string()),
            preferred_transport: Some("auto".to_string()),
            duration_seconds: Some(3600),
            cookie_mode: Some("browser".to_string()),
            cookie_browser: Some("firefox".to_string()),
            cookie_browser_profile: Some("i879pxds.default-release".to_string()),
            cookie_file_path: None,
            poll_interval_seconds: 60,
            record_mode: TikTokLiveRecordMode::OncePerLive,
            cooldown_seconds: 3600,
            filename_template: None,
            schedule_enabled: false,
            schedule_days: None,
            schedule_start_minute: None,
            schedule_end_minute: None,
            backoff_attempt: 0,
            next_check_at: 100,
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
            last_title: None,
            last_uploader: None,
            thumbnail: None,
            avatar: None,
            last_viewer_count: None,
            created_at: 100,
            updated_at: 100,
        }
    }

    #[test]
    fn watchlist_round_trip_and_due_query_keep_rules_without_secrets() {
        let _guard = db_test_guard();
        ensure_test_table();
        let mut entry = sample_entry("watch-1", "https://www.tiktok.com/@creator/live");
        entry.target_input =
            "https://www.tiktok.com/@creator/live?token=secret#fragment".to_string();
        entry.last_title = Some("Creator is live".to_string());
        entry.last_uploader = Some("Creator".to_string());
        entry.thumbnail = Some("https://p16.example/live-cover.jpeg".to_string());
        entry.avatar = Some("https://p16.example/creator-avatar.jpeg".to_string());
        entry.last_viewer_count = Some(12_345);
        save_tiktok_live_watch_entry_internal(&entry).expect("save watch entry");

        let loaded = get_tiktok_live_watch_entry_internal(&entry.id)
            .expect("load watch entry")
            .expect("watch entry exists");
        assert_eq!(loaded.cookie_browser_profile, entry.cookie_browser_profile);
        assert_eq!(loaded.poll_interval_seconds, 60);
        assert_eq!(loaded.record_mode, TikTokLiveRecordMode::OncePerLive);
        assert_eq!(loaded.cooldown_seconds, 3600);
        assert_eq!(loaded.target_input, entry.target_url);
        assert_eq!(loaded.last_title, entry.last_title);
        assert_eq!(loaded.last_uploader, entry.last_uploader);
        assert_eq!(loaded.thumbnail, entry.thumbnail);
        assert_eq!(loaded.avatar, entry.avatar);
        assert_eq!(loaded.last_viewer_count, entry.last_viewer_count);
        assert!(!loaded.schedule_enabled);
        assert!(
            get_tiktok_live_watch_entry_by_active_job_internal("missing-job")
                .expect("query missing job link")
                .is_none()
        );
        assert_eq!(
            get_due_tiktok_live_watchlist_internal(99)
                .expect("query early")
                .len(),
            0
        );
        assert_eq!(
            get_due_tiktok_live_watchlist_internal(100)
                .expect("query due")
                .len(),
            1
        );

        let serialized = serde_json::to_string(&loaded).expect("serialize watch entry");
        assert!(!serialized.contains("signedUrl"));
        assert!(!serialized.contains("cookieValue"));
        assert!(!serialized.contains("proxyUrl"));
        assert!(!serialized.contains("token=secret"));
        let connection = get_db().expect("get database");
        let mut statement = connection
            .prepare("PRAGMA table_info(tiktok_live_watchlist)")
            .expect("prepare schema query");
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query schema")
            .collect::<Result<Vec<_>, _>>()
            .expect("read schema");
        assert!(!columns.iter().any(|column| {
            let column = column.to_ascii_lowercase();
            column.contains("signed")
                || column.contains("proxy")
                || column.contains("cookie_value")
                || column.contains("secret_header")
        }));
        drop(statement);
        drop(connection);

        let mut linked = loaded;
        linked.active_job_id = Some("linked-job".to_string());
        save_tiktok_live_watch_entry_internal(&linked).expect("save active job link");
        assert_eq!(
            get_tiktok_live_watch_entry_by_active_job_internal("linked-job")
                .expect("query active job link")
                .map(|entry| entry.id),
            Some(linked.id)
        );
    }

    #[test]
    fn watchlist_update_does_not_resurrect_deleted_entry() {
        let _guard = db_test_guard();
        ensure_test_table();
        let mut entry = sample_entry("watch-deleted", "https://www.tiktok.com/@deleted/live");
        save_tiktok_live_watch_entry_internal(&entry).expect("save watch entry");
        delete_tiktok_live_watch_entry_internal(&entry.id).expect("delete watch entry");

        entry.status = TikTokLiveWatchStatus::Checking;
        entry.updated_at += 1;
        assert!(!update_tiktok_live_watch_entry_internal(&entry)
            .expect("update deleted watch entry returns false"));
        assert!(get_tiktok_live_watch_entry_internal(&entry.id)
            .expect("query deleted watch entry")
            .is_none());
    }

    #[test]
    fn watchlist_unique_target_prevents_duplicate_streamers() {
        let _guard = db_test_guard();
        ensure_test_table();
        let first = sample_entry("watch-1", "https://www.tiktok.com/@creator/live");
        let duplicate = sample_entry("watch-2", "https://www.tiktok.com/@CREATOR/live");
        save_tiktok_live_watch_entry_internal(&first).expect("save first entry");

        assert!(save_tiktok_live_watch_entry_internal(&duplicate).is_err());
        assert_eq!(
            get_tiktok_live_watchlist_internal()
                .expect("list watch entries")
                .len(),
            1
        );
    }
}
