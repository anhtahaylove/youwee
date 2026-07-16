use super::connection::get_db;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TikTokLiveRoomCacheEntry {
    pub username: String,
    pub room_id: String,
    pub session_id: Option<String>,
    pub observed_at: i64,
}

fn normalize_cache_username(username: &str) -> Result<String, String> {
    let normalized = username.trim().trim_start_matches('@').to_ascii_lowercase();
    if normalized.is_empty() {
        return Err("TikTok Live cache username cannot be empty".to_string());
    }
    Ok(normalized)
}

fn validate_room_id(room_id: &str) -> Result<String, String> {
    let normalized = room_id.trim();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err("TikTok Live cache room ID must be numeric".to_string());
    }
    Ok(normalized.to_string())
}

pub(crate) fn init_tiktok_live_room_cache_table(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tiktok_live_room_cache (
            username TEXT PRIMARY KEY COLLATE NOCASE,
            room_id TEXT NOT NULL,
            session_id TEXT,
            observed_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|error| format!("Failed to create TikTok Live room cache table: {error}"))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tiktok_live_room_cache_observed
         ON tiktok_live_room_cache(observed_at)",
        [],
    )
    .map_err(|error| format!("Failed to create TikTok Live room cache index: {error}"))?;
    Ok(())
}

pub fn get_tiktok_live_room_cache(
    username: &str,
) -> Result<Option<TikTokLiveRoomCacheEntry>, String> {
    let username = normalize_cache_username(username)?;
    let conn = get_db()?;
    let mut statement = conn
        .prepare(
            "SELECT username, room_id, session_id, observed_at
             FROM tiktok_live_room_cache
             WHERE username = ?1 COLLATE NOCASE",
        )
        .map_err(|error| format!("Failed to prepare TikTok Live room cache query: {error}"))?;
    let mut rows = statement
        .query(params![username])
        .map_err(|error| format!("Failed to query TikTok Live room cache: {error}"))?;
    let Some(row) = rows
        .next()
        .map_err(|error| format!("Failed to read TikTok Live room cache: {error}"))?
    else {
        return Ok(None);
    };

    Ok(Some(TikTokLiveRoomCacheEntry {
        username: row.get(0).map_err(|error| error.to_string())?,
        room_id: row.get(1).map_err(|error| error.to_string())?,
        session_id: row.get(2).map_err(|error| error.to_string())?,
        observed_at: row.get(3).map_err(|error| error.to_string())?,
    }))
}

pub fn upsert_tiktok_live_room_cache(
    username: &str,
    room_id: &str,
    session_id: Option<&str>,
    observed_at: i64,
) -> Result<(), String> {
    let username = normalize_cache_username(username)?;
    let room_id = validate_room_id(room_id)?;
    let session_id = session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO tiktok_live_room_cache (username, room_id, session_id, observed_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(username) DO UPDATE SET
            room_id = excluded.room_id,
            session_id = excluded.session_id,
            observed_at = excluded.observed_at",
        params![username, room_id, session_id, observed_at],
    )
    .map_err(|error| format!("Failed to save TikTok Live room cache: {error}"))?;
    Ok(())
}

pub fn delete_tiktok_live_room_cache(username: &str) -> Result<(), String> {
    let username = normalize_cache_username(username)?;
    let conn = get_db()?;
    conn.execute(
        "DELETE FROM tiktok_live_room_cache WHERE username = ?1 COLLATE NOCASE",
        params![username],
    )
    .map_err(|error| format!("Failed to delete TikTok Live room cache: {error}"))?;
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
        init_tiktok_live_room_cache_table(&connection).expect("create room cache table");
        connection
            .execute("DELETE FROM tiktok_live_room_cache", [])
            .expect("clear room cache");
    }

    #[test]
    fn room_cache_round_trip_normalizes_username_and_keeps_session() {
        let _guard = db_test_guard();
        ensure_test_table();

        upsert_tiktok_live_room_cache(
            "@Creator.User",
            "7662757197105466133",
            Some("session-1"),
            1234,
        )
        .expect("save room cache");
        let cached = get_tiktok_live_room_cache("creator.user")
            .expect("load room cache")
            .expect("cache exists");

        assert_eq!(cached.username, "creator.user");
        assert_eq!(cached.room_id, "7662757197105466133");
        assert_eq!(cached.session_id.as_deref(), Some("session-1"));
        assert_eq!(cached.observed_at, 1234);
    }

    #[test]
    fn room_cache_rejects_non_numeric_room_ids_and_deletes_case_insensitively() {
        let _guard = db_test_guard();
        ensure_test_table();

        assert!(upsert_tiktok_live_room_cache("creator", "not-a-room", None, 1).is_err());
        upsert_tiktok_live_room_cache("creator", "123456", None, 2).expect("save room cache");
        delete_tiktok_live_room_cache("@CREATOR").expect("delete room cache");

        assert!(get_tiktok_live_room_cache("creator")
            .expect("load room cache")
            .is_none());
    }
}
