use super::get_db;
use crate::types::{HistoryAdvancedFilters, HistoryEntry, HistoryMediaType, HistorySort};
use chrono::Utc;
use rusqlite::{params, params_from_iter, types::Value};

fn parse_history_row(row: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    let filepath: String = row.get(4)?;
    let file_exists = std::path::Path::new(&filepath).exists();
    let downloaded_at: i64 = row.get(10)?;
    let dt = chrono::DateTime::from_timestamp(downloaded_at, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();

    Ok(HistoryEntry {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        thumbnail: row.get(3)?,
        filepath,
        filesize: row.get(5)?,
        duration: row.get(6)?,
        quality: row.get(7)?,
        format: row.get(8)?,
        source: row.get(9)?,
        downloaded_at: dt,
        file_exists,
        summary: row.get(11)?,
        time_range: row.get(12)?,
    })
}

/// Add a history entry (internal use)
pub fn add_history_internal(
    url: String,
    title: String,
    thumbnail: Option<String>,
    filepath: String,
    filesize: Option<u64>,
    duration: Option<u64>,
    quality: Option<String>,
    format: Option<String>,
    source: Option<String>,
    time_range: Option<String>,
) -> Result<String, String> {
    let conn = get_db()?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    // Get max entries from default (500)
    let max_entries: i64 = 500;

    conn.execute(
        "INSERT OR REPLACE INTO history (id, url, title, thumbnail, filepath, filesize, duration, quality, format, source, downloaded_at, time_range)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![id, url, title, thumbnail, filepath, filesize, duration, quality, format, source, now, time_range],
    ).map_err(|e| format!("Failed to add history: {}", e))?;

    // Prune old entries
    conn.execute(
        "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY downloaded_at DESC LIMIT ?1)",
        params![max_entries],
    ).ok();

    Ok(id)
}

/// Update summary for a history entry
pub fn update_history_summary(id: String, summary: String) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "UPDATE history SET summary = ?1 WHERE id = ?2",
        params![summary, id],
    )
    .map_err(|e| format!("Failed to update summary: {}", e))?;
    Ok(())
}

/// Update a history entry with download info (for re-downloads)
pub fn update_history_download(
    id: String,
    filepath: String,
    filesize: Option<u64>,
    quality: Option<String>,
    format: Option<String>,
    time_range: Option<String>,
) -> Result<(), String> {
    let conn = get_db()?;
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE history SET filepath = ?1, filesize = ?2, quality = ?3, format = ?4, downloaded_at = ?5, time_range = ?6 WHERE id = ?7",
        params![filepath, filesize, quality, format, now, time_range, id],
    )
    .map_err(|e| format!("Failed to update history: {}", e))?;
    Ok(())
}

/// Update history filepath + title by old filepath (used after file rename)
pub fn update_history_filepath_and_title(
    old_filepath: String,
    new_filepath: String,
    new_title: String,
) -> Result<(), String> {
    let conn = get_db()?;
    let rows = conn
        .execute(
        "UPDATE history SET filepath = ?1, title = ?2 WHERE filepath = ?3",
        params![new_filepath, new_title, old_filepath],
    )
        .map_err(|e| format!("Failed to update history filepath/title: {}", e))?;
    if rows == 0 {
        return Err("No history entry matched this filepath".to_string());
    }
    Ok(())
}

/// Update history filepath + title by history entry ID (preferred when available)
pub fn update_history_filepath_and_title_by_id(
    id: String,
    new_filepath: String,
    new_title: String,
) -> Result<(), String> {
    let conn = get_db()?;
    let rows = conn
        .execute(
            "UPDATE history SET filepath = ?1, title = ?2 WHERE id = ?3",
            params![new_filepath, new_title, id],
        )
        .map_err(|e| format!("Failed to update history filepath/title by id: {}", e))?;
    if rows == 0 {
        return Err("History entry not found".to_string());
    }
    Ok(())
}

/// Add a history entry with summary (for videos summarized without downloading)
pub fn add_history_with_summary(
    url: String,
    title: String,
    thumbnail: Option<String>,
    duration: Option<u64>,
    source: Option<String>,
    summary: String,
) -> Result<String, String> {
    let conn = get_db()?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    // Use empty filepath to indicate it's summary-only (not downloaded)
    let filepath = "";

    conn.execute(
        "INSERT OR REPLACE INTO history (id, url, title, thumbnail, filepath, filesize, duration, quality, format, source, downloaded_at, summary)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![id, url, title, thumbnail, filepath, Option::<u64>::None, duration, Option::<String>::None, Option::<String>::None, source, now, summary],
    ).map_err(|e| format!("Failed to add history: {}", e))?;

    Ok(id)
}

fn audio_media_sql_condition() -> &'static str {
    "(LOWER(COALESCE(format, '')) IN ('mp3', 'm4a', 'opus', 'flac', 'wav', 'aac', 'ogg', 'oga') OR LOWER(COALESCE(quality, '')) LIKE '%audio%')"
}

fn normalize_list(values: Option<&Vec<String>>) -> Vec<String> {
    let mut normalized: Vec<String> = Vec::new();
    if let Some(items) = values {
        for item in items {
            let value = item.trim().to_lowercase();
            if value.is_empty() || normalized.iter().any(|v| v == &value) {
                continue;
            }
            normalized.push(value);
        }
    }
    normalized
}

fn normalize_quality(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    if lower.contains("audio") {
        "audio".to_string()
    } else if lower.contains("best") {
        "best".to_string()
    } else if lower.contains("8k") {
        "8k".to_string()
    } else if lower.contains("4k") {
        "4k".to_string()
    } else if lower.contains("2k") {
        "2k".to_string()
    } else if lower.contains("1080") {
        "1080".to_string()
    } else if lower.contains("720") {
        "720".to_string()
    } else if lower.contains("480") {
        "480".to_string()
    } else if lower.contains("360") {
        "360".to_string()
    } else {
        lower
    }
}

fn apply_history_filters(
    query: &mut String,
    params: &mut Vec<Value>,
    source: Option<&str>,
    search: Option<&str>,
    filters: Option<&HistoryAdvancedFilters>,
) {
    if let Some(src) = source {
        let src = src.trim();
        if !src.is_empty() && src != "all" {
            query.push_str(" AND source = ?");
            params.push(Value::from(src.to_string()));
        }
    }

    if let Some(search_text) = search.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let search_pattern = format!("%{}%", search_text);
        query.push_str(" AND (title LIKE ? OR filepath LIKE ?)");
        params.push(Value::from(search_pattern.clone()));
        params.push(Value::from(search_pattern));
    }

    if let Some(filter) = filters {
        match filter.media_type {
            Some(HistoryMediaType::Audio) => {
                query.push_str(" AND ");
                query.push_str(audio_media_sql_condition());
            }
            Some(HistoryMediaType::Video) => {
                query.push_str(" AND NOT ");
                query.push_str(audio_media_sql_condition());
            }
            _ => {}
        }

        if let Some(from) = filter.downloaded_at_from {
            query.push_str(" AND downloaded_at >= ?");
            params.push(Value::from(from));
        }
        if let Some(to) = filter.downloaded_at_to {
            query.push_str(" AND downloaded_at <= ?");
            params.push(Value::from(to));
        }

        let formats = normalize_list(filter.formats.as_ref());
        if !formats.is_empty() {
            query.push_str(" AND LOWER(COALESCE(format, '')) IN (");
            for (idx, value) in formats.iter().enumerate() {
                if idx > 0 {
                    query.push_str(", ");
                }
                query.push('?');
                params.push(Value::from(value.clone()));
            }
            query.push(')');
        }

        let mut qualities = normalize_list(filter.qualities.as_ref());
        qualities = qualities
            .into_iter()
            .map(|q| normalize_quality(&q))
            .fold(Vec::new(), |mut acc, q| {
                if !q.is_empty() && !acc.iter().any(|existing| existing == &q) {
                    acc.push(q);
                }
                acc
            });

        if !qualities.is_empty() {
            query.push_str(" AND (");
            for (idx, quality) in qualities.iter().enumerate() {
                if idx > 0 {
                    query.push_str(" OR ");
                }
                if quality == "audio" {
                    query.push_str("(LOWER(COALESCE(quality, '')) LIKE ? OR LOWER(COALESCE(format, '')) IN ('mp3', 'm4a', 'opus', 'flac', 'wav', 'aac', 'ogg', 'oga'))");
                } else {
                    query.push_str("LOWER(COALESCE(quality, '')) LIKE ?");
                }
                params.push(Value::from(format!("%{}%", quality)));
            }
            query.push(')');
        }
    }
}

/// Get history entries
pub fn get_history_from_db(
    limit: Option<i64>,
    offset: Option<i64>,
    source: Option<String>,
    search: Option<String>,
    filters: Option<HistoryAdvancedFilters>,
    sort: Option<HistorySort>,
) -> Result<Vec<HistoryEntry>, String> {
    let conn = get_db()?;

    let limit = limit.unwrap_or(50).min(500);
    let offset = offset.unwrap_or(0);

    let mut query = String::from(
        "SELECT id, url, title, thumbnail, filepath, filesize, duration, quality, format, source, downloaded_at, summary, time_range 
         FROM history WHERE 1=1"
    );
    let mut query_params: Vec<Value> = Vec::new();
    apply_history_filters(
        &mut query,
        &mut query_params,
        source.as_deref(),
        search.as_deref(),
        filters.as_ref(),
    );

    match sort.unwrap_or_default() {
        HistorySort::Recent => query.push_str(" ORDER BY downloaded_at DESC"),
        HistorySort::Oldest => query.push_str(" ORDER BY downloaded_at ASC"),
        HistorySort::Title => query.push_str(" ORDER BY LOWER(title) ASC"),
        HistorySort::Size => query.push_str(" ORDER BY filesize IS NULL ASC, filesize DESC"),
    }
    query.push_str(" LIMIT ? OFFSET ?");
    query_params.push(Value::from(limit));
    query_params.push(Value::from(offset));

    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let entries: Vec<HistoryEntry> = stmt
        .query_map(params_from_iter(query_params.iter()), parse_history_row)
        .map_err(|e| format!("Query failed: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(entries)
}

pub fn get_history_entries_by_ids_from_db(ids: Vec<String>) -> Result<Vec<HistoryEntry>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let conn = get_db()?;
    let placeholders = vec!["?"; ids.len()].join(", ");
    let query = format!(
        "SELECT id, url, title, thumbnail, filepath, filesize, duration, quality, format, source, downloaded_at, summary, time_range
         FROM history
         WHERE id IN ({})",
        placeholders
    );
    let query_params: Vec<Value> = ids.into_iter().map(Value::from).collect();

    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let entries: Vec<HistoryEntry> = stmt
        .query_map(params_from_iter(query_params.iter()), parse_history_row)
        .map_err(|e| format!("Query failed: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(entries)
}

/// Delete a history entry
pub fn delete_history_from_db(id: String) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM history WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete history: {}", e))?;
    Ok(())
}

/// Clear all history
pub fn clear_history_from_db() -> Result<(), String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM history", [])
        .map_err(|e| format!("Failed to clear history: {}", e))?;
    Ok(())
}

/// Get history count
pub fn get_history_count_from_db(
    source: Option<String>,
    search: Option<String>,
    filters: Option<HistoryAdvancedFilters>,
) -> Result<i64, String> {
    let conn = get_db()?;

    let mut query = String::from("SELECT COUNT(*) FROM history WHERE 1=1");
    let mut query_params: Vec<Value> = Vec::new();
    apply_history_filters(
        &mut query,
        &mut query_params,
        source.as_deref(),
        search.as_deref(),
        filters.as_ref(),
    );

    let count: i64 = conn
        .query_row(&query, params_from_iter(query_params.iter()), |row| row.get(0))
        .map_err(|e| format!("Failed to count history: {}", e))?;

    Ok(count)
}
