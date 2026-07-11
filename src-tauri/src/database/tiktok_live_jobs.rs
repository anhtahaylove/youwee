use super::connection::get_db;
use chrono::Utc;
use rusqlite::{params, types::Type, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TikTokLiveJobStatus {
    Preparing,
    Recording,
    Reconnecting,
    Interrupted,
    Recoverable,
    Finalizing,
    Completed,
    Partial,
    Cancelled,
    Failed,
}

impl TikTokLiveJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Recording => "recording",
            Self::Reconnecting => "reconnecting",
            Self::Interrupted => "interrupted",
            Self::Recoverable => "recoverable",
            Self::Finalizing => "finalizing",
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn can_resume(self) -> bool {
        matches!(self, Self::Interrupted | Self::Recoverable | Self::Failed)
    }

    pub fn is_restart_candidate(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Recording
                | Self::Reconnecting
                | Self::Interrupted
                | Self::Recoverable
                | Self::Finalizing
        )
    }
}

impl TryFrom<&str> for TikTokLiveJobStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "preparing" => Ok(Self::Preparing),
            "recording" => Ok(Self::Recording),
            "reconnecting" => Ok(Self::Reconnecting),
            "interrupted" => Ok(Self::Interrupted),
            "recoverable" => Ok(Self::Recoverable),
            "finalizing" => Ok(Self::Finalizing),
            "completed" => Ok(Self::Completed),
            "partial" => Ok(Self::Partial),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("Unknown TikTok Live job status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TikTokLiveJob {
    pub id: String,
    pub target_input: String,
    pub target_url: String,
    pub username: Option<String>,
    pub title: String,
    pub thumbnail: Option<String>,
    pub output_dir: String,
    pub output_path: Option<String>,
    pub final_path: Option<String>,
    pub preferred_quality: Option<String>,
    pub preferred_transport: Option<String>,
    pub duration_seconds: Option<u32>,
    pub cookie_mode: Option<String>,
    pub cookie_browser: Option<String>,
    pub cookie_browser_profile: Option<String>,
    pub cookie_file_path: Option<String>,
    pub auto_reconnect: bool,
    pub status: TikTokLiveJobStatus,
    pub segment_paths: Vec<String>,
    pub refresh_count: u32,
    pub reconnect_count: u32,
    pub format_id: Option<String>,
    pub history_id: Option<String>,
    pub error_message: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

impl TikTokLiveJob {
    pub fn touch(&mut self) {
        self.updated_at = Utc::now().timestamp();
    }
}

const JOB_COLUMNS: &str = "id, target_input, target_url, username, title, thumbnail, output_dir, output_path, final_path, preferred_quality, preferred_transport, duration_seconds, cookie_mode, cookie_browser, cookie_browser_profile, cookie_file_path, auto_reconnect, status, segment_paths_json, refresh_count, reconnect_count, format_id, history_id, error_message, started_at, updated_at, completed_at";

pub(crate) fn init_tiktok_live_jobs_table(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tiktok_live_jobs (
            id TEXT PRIMARY KEY,
            target_input TEXT NOT NULL,
            target_url TEXT NOT NULL,
            username TEXT,
            title TEXT NOT NULL,
            thumbnail TEXT,
            output_dir TEXT NOT NULL,
            output_path TEXT,
            final_path TEXT,
            preferred_quality TEXT,
            preferred_transport TEXT,
            duration_seconds INTEGER,
            cookie_mode TEXT,
            cookie_browser TEXT,
            cookie_browser_profile TEXT,
            cookie_file_path TEXT,
            auto_reconnect INTEGER NOT NULL DEFAULT 1,
            status TEXT NOT NULL,
            segment_paths_json TEXT NOT NULL DEFAULT '[]',
            refresh_count INTEGER NOT NULL DEFAULT 0,
            reconnect_count INTEGER NOT NULL DEFAULT 0,
            format_id TEXT,
            history_id TEXT,
            error_message TEXT,
            started_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            completed_at INTEGER
        )",
        [],
    )
    .map_err(|e| format!("Failed to create TikTok Live jobs table: {e}"))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tiktok_live_jobs_status_updated
         ON tiktok_live_jobs(status, updated_at DESC)",
        [],
    )
    .map_err(|e| format!("Failed to create TikTok Live jobs index: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tiktok_live_recorder_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            max_concurrent_recordings INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create TikTok Live recorder config table: {e}"))?;
    Ok(())
}

fn job_from_row(row: &Row<'_>) -> rusqlite::Result<TikTokLiveJob> {
    let status: String = row.get(17)?;
    let status = TikTokLiveJobStatus::try_from(status.as_str()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            17,
            Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        )
    })?;
    let segment_paths_json: String = row.get(18)?;
    let segment_paths = serde_json::from_str(&segment_paths_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(18, Type::Text, Box::new(error))
    })?;

    Ok(TikTokLiveJob {
        id: row.get(0)?,
        target_input: row.get(1)?,
        target_url: row.get(2)?,
        username: row.get(3)?,
        title: row.get(4)?,
        thumbnail: row.get(5)?,
        output_dir: row.get(6)?,
        output_path: row.get(7)?,
        final_path: row.get(8)?,
        preferred_quality: row.get(9)?,
        preferred_transport: row.get(10)?,
        duration_seconds: row.get::<_, Option<i64>>(11)?.map(|value| value as u32),
        cookie_mode: row.get(12)?,
        cookie_browser: row.get(13)?,
        cookie_browser_profile: row.get(14)?,
        cookie_file_path: row.get(15)?,
        auto_reconnect: row.get::<_, i64>(16)? != 0,
        status,
        segment_paths,
        refresh_count: row.get::<_, i64>(19)? as u32,
        reconnect_count: row.get::<_, i64>(20)? as u32,
        format_id: row.get(21)?,
        history_id: row.get(22)?,
        error_message: row.get(23)?,
        started_at: row.get(24)?,
        updated_at: row.get(25)?,
        completed_at: row.get(26)?,
    })
}

pub fn save_tiktok_live_job_internal(job: &TikTokLiveJob) -> Result<(), String> {
    let segment_paths_json = serde_json::to_string(&job.segment_paths)
        .map_err(|e| format!("Failed to serialize TikTok Live segments: {e}"))?;
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO tiktok_live_jobs (
            id, target_input, target_url, username, title, thumbnail, output_dir,
            output_path, final_path, preferred_quality, preferred_transport,
            duration_seconds, cookie_mode, cookie_browser, cookie_browser_profile,
            cookie_file_path, auto_reconnect, status, segment_paths_json,
            refresh_count, reconnect_count, format_id, history_id, error_message,
            started_at, updated_at, completed_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27
         )
         ON CONFLICT(id) DO UPDATE SET
            target_input = excluded.target_input,
            target_url = excluded.target_url,
            username = excluded.username,
            title = excluded.title,
            thumbnail = excluded.thumbnail,
            output_dir = excluded.output_dir,
            output_path = excluded.output_path,
            final_path = excluded.final_path,
            preferred_quality = excluded.preferred_quality,
            preferred_transport = excluded.preferred_transport,
            duration_seconds = excluded.duration_seconds,
            cookie_mode = excluded.cookie_mode,
            cookie_browser = excluded.cookie_browser,
            cookie_browser_profile = excluded.cookie_browser_profile,
            cookie_file_path = excluded.cookie_file_path,
            auto_reconnect = excluded.auto_reconnect,
            status = excluded.status,
            segment_paths_json = excluded.segment_paths_json,
            refresh_count = excluded.refresh_count,
            reconnect_count = excluded.reconnect_count,
            format_id = excluded.format_id,
            history_id = excluded.history_id,
            error_message = excluded.error_message,
            started_at = excluded.started_at,
            updated_at = excluded.updated_at,
            completed_at = excluded.completed_at",
        params![
            job.id,
            job.target_input,
            job.target_url,
            job.username,
            job.title,
            job.thumbnail,
            job.output_dir,
            job.output_path,
            job.final_path,
            job.preferred_quality,
            job.preferred_transport,
            job.duration_seconds.map(i64::from),
            job.cookie_mode,
            job.cookie_browser,
            job.cookie_browser_profile,
            job.cookie_file_path,
            i64::from(job.auto_reconnect),
            job.status.as_str(),
            segment_paths_json,
            i64::from(job.refresh_count),
            i64::from(job.reconnect_count),
            job.format_id,
            job.history_id,
            job.error_message,
            job.started_at,
            job.updated_at,
            job.completed_at,
        ],
    )
    .map_err(|e| format!("Failed to save TikTok Live job: {e}"))?;
    Ok(())
}

pub fn get_tiktok_live_job_internal(id: &str) -> Result<Option<TikTokLiveJob>, String> {
    let conn = get_db()?;
    let mut statement = conn
        .prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM tiktok_live_jobs WHERE id = ?1"
        ))
        .map_err(|e| format!("Failed to prepare TikTok Live job query: {e}"))?;
    let mut rows = statement
        .query(params![id])
        .map_err(|e| format!("Failed to query TikTok Live job: {e}"))?;
    rows.next()
        .map_err(|e| format!("Failed to read TikTok Live job: {e}"))?
        .map(job_from_row)
        .transpose()
        .map_err(|e| format!("Failed to decode TikTok Live job: {e}"))
}

pub fn get_tiktok_live_jobs_internal() -> Result<Vec<TikTokLiveJob>, String> {
    let conn = get_db()?;
    let mut statement = conn
        .prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM tiktok_live_jobs ORDER BY updated_at DESC"
        ))
        .map_err(|e| format!("Failed to prepare TikTok Live jobs query: {e}"))?;
    let rows = statement
        .query_map([], job_from_row)
        .map_err(|e| format!("Failed to query TikTok Live jobs: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to decode TikTok Live jobs: {e}"))
}

pub fn delete_tiktok_live_job_internal(id: &str) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM tiktok_live_jobs WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete TikTok Live job: {e}"))?;
    Ok(())
}

pub fn get_tiktok_live_recorder_limit_internal() -> Result<Option<usize>, String> {
    let conn = get_db()?;
    match conn.query_row(
        "SELECT max_concurrent_recordings FROM tiktok_live_recorder_config WHERE id = 1",
        [],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(value) => Ok(Some(value.max(1) as usize)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to load TikTok Live recorder config: {e}")),
    }
}

pub fn set_tiktok_live_recorder_limit_internal(value: usize) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO tiktok_live_recorder_config (id, max_concurrent_recordings, updated_at)
         VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET
            max_concurrent_recordings = excluded.max_concurrent_recordings,
            updated_at = excluded.updated_at",
        params![value as i64, Utc::now().timestamp()],
    )
    .map_err(|e| format!("Failed to save TikTok Live recorder config: {e}"))?;
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
        init_tiktok_live_jobs_table(&connection).expect("create TikTok Live jobs table");
        connection
            .execute("DELETE FROM tiktok_live_jobs", [])
            .expect("clear TikTok Live jobs");
        connection
            .execute("DELETE FROM tiktok_live_recorder_config", [])
            .expect("clear TikTok Live recorder config");
    }

    fn sample_job() -> TikTokLiveJob {
        TikTokLiveJob {
            id: "job-1".to_string(),
            target_input: "@creator".to_string(),
            target_url: "https://www.tiktok.com/@creator/live".to_string(),
            username: Some("creator".to_string()),
            title: "TikTok LIVE @creator".to_string(),
            thumbnail: None,
            output_dir: "C:/Downloads".to_string(),
            output_path: Some("C:/Downloads/creator.mp4".to_string()),
            final_path: None,
            preferred_quality: Some("auto".to_string()),
            preferred_transport: Some("auto".to_string()),
            duration_seconds: Some(60),
            cookie_mode: Some("browser".to_string()),
            cookie_browser: Some("firefox".to_string()),
            cookie_browser_profile: Some("i879pxds.default-release".to_string()),
            cookie_file_path: None,
            auto_reconnect: true,
            status: TikTokLiveJobStatus::Recording,
            segment_paths: vec!["C:/Downloads/creator.part-001.mkv".to_string()],
            refresh_count: 1,
            reconnect_count: 1,
            format_id: Some("hls-hd".to_string()),
            history_id: None,
            error_message: None,
            started_at: 1,
            updated_at: 2,
            completed_at: None,
        }
    }

    #[test]
    fn persisted_job_round_trip_keeps_recovery_fields_without_secrets() {
        let _guard = db_test_guard();
        ensure_test_table();
        let job = sample_job();

        save_tiktok_live_job_internal(&job).expect("save job");
        let loaded = get_tiktok_live_job_internal(&job.id)
            .expect("load job")
            .expect("job exists");
        let serialized = serde_json::to_string(&loaded).expect("serialize job");

        assert_eq!(loaded.status, TikTokLiveJobStatus::Recording);
        assert_eq!(loaded.segment_paths, job.segment_paths);
        assert_eq!(loaded.cookie_browser_profile, job.cookie_browser_profile);
        assert!(!serialized.contains("signedUrl"));
        assert!(!serialized.contains("cookieValue"));
        assert!(!serialized.contains("proxyUrl"));

        let connection = get_db().expect("get database");
        let mut statement = connection
            .prepare("PRAGMA table_info(tiktok_live_jobs)")
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
    }

    #[test]
    fn job_statuses_cover_resume_and_restart_transitions() {
        assert!(TikTokLiveJobStatus::Recording.is_restart_candidate());
        assert!(TikTokLiveJobStatus::Finalizing.is_restart_candidate());
        assert!(TikTokLiveJobStatus::Recoverable.can_resume());
        assert!(TikTokLiveJobStatus::Interrupted.can_resume());
        assert!(!TikTokLiveJobStatus::Completed.can_resume());
    }

    #[test]
    fn recorder_limit_config_round_trip() {
        let _guard = db_test_guard();
        ensure_test_table();

        assert_eq!(
            get_tiktok_live_recorder_limit_internal().expect("missing config"),
            None
        );
        set_tiktok_live_recorder_limit_internal(3).expect("save config");

        assert_eq!(
            get_tiktok_live_recorder_limit_internal().expect("load config"),
            Some(3)
        );
    }
}
