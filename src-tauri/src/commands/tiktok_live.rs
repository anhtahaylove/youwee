use crate::database::add_history_internal;
use crate::database::add_log_internal;
use crate::services::{
    get_ffmpeg_path, parse_ytdlp_error, run_ytdlp_json_with_cookies, should_skip_cookies_for_url,
};
use crate::types::{code, BackendError};
use crate::utils::{firefox_profiles_ini_path, resolve_firefox_profile_for_cookies};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ORIGIN, REFERER, USER_AGENT};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout, Instant};

static ACTIVE_RECORDINGS: LazyLock<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const METADATA_FETCH_ATTEMPTS: u32 = 3;
const METADATA_RETRY_BASE_DELAY_MS: u64 = 750;
const RECONNECT_MAX_RETRIES: u32 = 20;
const RECONNECT_DELAY_MAX_SECONDS: u32 = 5;
const RECONNECT_DELAY_TOTAL_MAX_SECONDS: u32 = 120;
const STREAM_URL_REFRESH_ATTEMPTS: u32 = 3;

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

const TIKTOK_BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

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
            input: trimmed.to_string(),
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
    let title = sanitize_filename_part(title, "TikTok LIVE");
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    output_dir.join(format!("{title}_{timestamp}.mp4"))
}

fn segment_path_for_recording(output_path: &Path, index: usize) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("TikTok LIVE");
    output_path.with_file_name(format!("{stem}.part-{index:03}.mp4"))
}

fn concat_list_path_for_recording(output_path: &Path) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("TikTok LIVE");
    output_path.with_file_name(format!("{stem}.ffconcat"))
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
        "uhd_60" => 95,
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
    if protocol.contains("hls") || protocol.contains("m3u8") || ext == "m3u8" {
        30
    } else if protocol.contains("flv") || ext == "flv" {
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

fn matches_filter(value: &Option<String>, filter: &Option<String>) -> bool {
    let Some(filter) = filter.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if filter.eq_ignore_ascii_case("auto") {
        return true;
    }

    value
        .as_deref()
        .map(|value| {
            value
                .to_ascii_lowercase()
                .contains(&filter.to_ascii_lowercase())
        })
        .unwrap_or(false)
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
            .filter(|variant| {
                matches_filter(&variant.quality, preferred_quality)
                    || matches_filter(&variant.note, preferred_quality)
            })
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
            .filter(|format| {
                matches_filter(&format.variant.quality, preferred_quality)
                    || matches_filter(&format.variant.note, preferred_quality)
            })
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
                    fps: None,
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

fn variants_from_tiktok_stream_data(stream_data: &serde_json::Value) -> Vec<TikTokLiveVariant> {
    formats_from_tiktok_stream_data(stream_data)
        .into_iter()
        .map(|format| format.variant)
        .collect()
}

fn stream_data_from_json(json: &serde_json::Value) -> Option<serde_json::Value> {
    [
        "/stream_url/live_core_sdk_data/pull_data/stream_data",
        "/live_core_sdk_data/pull_data/stream_data",
        "/data/stream_url/live_core_sdk_data/pull_data/stream_data",
    ]
    .iter()
    .find_map(|path| match json.pointer(path)? {
        serde_json::Value::String(raw) => serde_json::from_str(raw).ok(),
        value @ serde_json::Value::Object(_) => Some(value.clone()),
        _ => None,
    })
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
    let variants: Vec<TikTokLiveVariant> = json
        .get("formats")
        .and_then(|v| v.as_array())
        .map(|formats| {
            formats
                .iter()
                .filter_map(variant_from_ytdlp_format)
                .collect()
        })
        .unwrap_or_default();

    if variants.is_empty() {
        stream_data_from_json(json)
            .map(|stream_data| variants_from_tiktok_stream_data(&stream_data))
            .filter(|variants| !variants.is_empty())
            .unwrap_or_else(|| {
                formats_from_legacy_room_stream_urls(json)
                    .into_iter()
                    .map(|format| format.variant)
                    .collect()
            })
    } else {
        variants
    }
}

fn formats_from_ytdlp_json(json: &serde_json::Value) -> Vec<TikTokLiveFormat> {
    let formats: Vec<TikTokLiveFormat> = json
        .get("formats")
        .and_then(|v| v.as_array())
        .map(|formats| {
            formats
                .iter()
                .filter_map(format_from_ytdlp_format)
                .collect()
        })
        .unwrap_or_default();

    if formats.is_empty() {
        stream_data_from_json(json)
            .map(|stream_data| formats_from_tiktok_stream_data(&stream_data))
            .filter(|formats| !formats.is_empty())
            .unwrap_or_else(|| formats_from_legacy_room_stream_urls(json))
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
        "-movflags".to_string(),
        "+faststart".to_string(),
        output_path.to_string_lossy().to_string(),
    ]);
    args
}

async fn remove_recording_paths(paths: &[PathBuf]) {
    for path in paths {
        tokio::fs::remove_file(path).await.ok();
    }
}

async fn preserve_first_segment(segment_paths: &[PathBuf], output_path: &Path) -> PathBuf {
    let first = segment_paths[0].clone();
    tokio::fs::remove_file(output_path).await.ok();
    if tokio::fs::rename(&first, output_path).await.is_ok() {
        output_path.to_path_buf()
    } else {
        first
    }
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

    if segment_paths.len() == 1 {
        let first = &segment_paths[0];
        tokio::fs::remove_file(output_path).await.ok();
        return match tokio::fs::rename(first, output_path).await {
            Ok(()) => Ok((output_path.to_path_buf(), false)),
            Err(_) => Ok((first.clone(), false)),
        };
    }

    let concat_path = concat_list_path_for_recording(output_path);
    if tokio::fs::write(&concat_path, ffconcat_content(segment_paths))
        .await
        .is_err()
    {
        return Ok((
            preserve_first_segment(segment_paths, output_path).await,
            true,
        ));
    }

    let mut cmd = Command::new(ffmpeg_path);
    cmd.args([
        "-hide_banner",
        "-nostdin",
        "-y",
        "-fflags",
        "+genpts",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
    ])
    .arg(&concat_path)
    .args([
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
            tokio::fs::remove_file(&concat_path).await.ok();
            return Ok((
                preserve_first_segment(segment_paths, output_path).await,
                true,
            ));
        }
    };

    let status = tokio::select! {
        status = child.wait() => status.ok(),
        _ = &mut *cancel_rx => {
            child.kill().await.ok();
            tokio::fs::remove_file(output_path).await.ok();
            tokio::fs::remove_file(&concat_path).await.ok();
            remove_recording_paths(segment_paths).await;
            return Err(BackendError::from_message("TikTok Live recording cancelled.").to_wire_string());
        }
    };

    let merged = status.is_some_and(|status| status.success())
        && tokio::fs::metadata(output_path)
            .await
            .is_ok_and(|metadata| metadata.len() > 0);
    tokio::fs::remove_file(&concat_path).await.ok();

    if merged {
        remove_recording_paths(segment_paths).await;
        Ok((output_path.to_path_buf(), false))
    } else {
        tokio::fs::remove_file(output_path).await.ok();
        Ok((
            preserve_first_segment(segment_paths, output_path).await,
            true,
        ))
    }
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
            if let Ok(json) = fetch_tiktok_live_json(
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
                return Ok((json, live_url));
            }
        }
        return Ok((room_json, url));
    }

    let target_url = target
        .url
        .clone()
        .ok_or_else(|| BackendError::from_message("Missing TikTok Live URL").to_wire_string())?;
    let json = fetch_tiktok_live_json(
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

    Ok(TikTokLiveInspectResult {
        input: target.input,
        target_url,
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
    let target = parse_tiktok_live_target(&input)
        .map_err(|e| BackendError::from_message(e).to_wire_string())?;
    let auto_reconnect = auto_reconnect.unwrap_or(true);
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut recordings = ACTIVE_RECORDINGS.lock().await;
        recordings.insert(job_id.clone(), cancel_tx);
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

        let output_dir = if output_dir.trim().is_empty() {
            app.path().download_dir().map_err(|e| {
                BackendError::from_message(format!("Failed to resolve Downloads folder: {e}"))
                    .to_wire_string()
            })?
        } else {
            PathBuf::from(output_dir.trim())
        };
        tokio::fs::create_dir_all(&output_dir)
            .await
            .map_err(|e| {
                BackendError::from_message(format!("Failed to create output folder: {e}"))
                    .to_wire_string()
            })?;

        let title = tiktok_live_title(&json, target.username.as_deref());
        let output_path = output_path_for_recording(&output_dir, &title);
        let history_target_url = target_url.clone();
        let started_at = Instant::now();
        let mut segment_paths = Vec::new();
        let mut segment_number = 1usize;
        let mut refresh_attempts = 0u32;
        let mut partial = false;

        loop {
            let remaining_seconds = remaining_recording_seconds(started_at, duration_seconds);
            if duration_seconds.is_some() && remaining_seconds == Some(0) {
                break;
            }

            let segment_path = segment_path_for_recording(&output_path, segment_number);
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
                    title, selected.variant.format_id, auto_reconnect
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
            }

            if status.is_some_and(|status| status.success()) {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "FFmpeg completed without recording TikTok Live media.",
                    )
                    .to_wire_string());
                }
                break;
            }

            if duration_seconds.is_some() && remaining_recording_seconds(started_at, duration_seconds) == Some(0) {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "TikTok Live recording ended without media.",
                    )
                    .to_wire_string());
                }
                break;
            }

            if !auto_reconnect {
                remove_recording_paths(&segment_paths).await;
                return Err(BackendError::from_message(format!(
                    "FFmpeg exited with code: {:?}",
                    status.and_then(|status| status.code())
                ))
                .to_wire_string());
            }

            if refresh_attempts >= STREAM_URL_REFRESH_ATTEMPTS {
                if segment_paths.is_empty() {
                    return Err(BackendError::from_message(
                        "TikTok Live stream URL refresh attempts were exhausted without media.",
                    )
                    .to_wire_string());
                }
                partial = true;
                break;
            }

            refresh_attempts += 1;
            emit_tiktok_live_status(
                &app,
                Some(&job_id),
                "refreshing-stream",
                Some(refresh_attempts),
                Some(STREAM_URL_REFRESH_ATTEMPTS),
                Some(auto_reconnect),
            );
            add_log_internal(
                "info",
                &format!(
                    "Refreshing TikTok Live signed stream URL ({refresh_attempts}/{STREAM_URL_REFRESH_ATTEMPTS})"
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
                    return Err(BackendError::from_message("TikTok Live recording cancelled.").to_wire_string());
                }
            };

            let (refreshed_json, refreshed_target_url) = match refreshed {
                Ok(result) => result,
                Err(error) if !segment_paths.is_empty() => {
                    let backend_error = BackendError::from_message(&error);
                    let stream_ended = backend_error.code() == code::TIKTOK_LIVE_OFFLINE;
                    let message = backend_error.message().to_string();
                    add_log_internal(
                        "info",
                        &format!("TikTok Live URL refresh stopped: {message}"),
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

        let segment_count = segment_paths.len();
        let (final_path, merge_failed) = finalize_recording_segments(
            &ffmpeg_path,
            &segment_paths,
            &output_path,
            &mut cancel_rx,
        )
        .await?;
        partial |= merge_failed;

        let output_path_str = final_path.to_string_lossy().to_string();
        let filesize = tokio::fs::metadata(&final_path)
            .await
            .ok()
            .map(|metadata| metadata.len());
        let thumbnail = string_at(
            &json,
            &[
                "/thumbnail",
                "/data/cover/url_list/0",
                "/data/owner/avatar_thumb/url_list/0",
            ],
        );
        let history_id = add_history_internal(
            history_target_url.clone(),
            title.clone(),
            thumbnail,
            output_path_str.clone(),
            filesize,
            if partial {
                None
            } else {
                duration_seconds.map(u64::from)
            },
            Some(selected.variant.format_id),
            Some("mp4".to_string()),
            Some("tiktok-live".to_string()),
            None,
        )?;

        add_log_internal(
            "success",
            &format!(
                "Recorded TikTok Live: {}{}",
                title,
                if merge_failed {
                    format!(
                        " ({segment_count} segments preserved because automatic merge failed)"
                    )
                } else if partial {
                    format!(
                        " (partial recording merged after {refresh_attempts} signed URL refresh attempts)"
                    )
                } else if segment_count > 1 {
                    format!(
                        " ({segment_count} segments merged after {refresh_attempts} signed URL refreshes)"
                    )
                } else {
                    String::new()
                }
            ),
            Some(&output_path_str),
            Some(&history_target_url),
        )
        .ok();

        Ok(TikTokLiveRecordResult {
            job_id: job_id.clone(),
            history_id,
            filepath: output_path_str,
            title,
            filesize,
            partial,
        })
    }
    .await;

    if let Err(error) = &result {
        let backend_error = BackendError::from_message(error.as_str());
        let target_url = tiktok_target_url(&target).unwrap_or_else(|| target.input.clone());
        add_log_internal(
            if backend_error.code() == code::DOWNLOAD_CANCELLED {
                "info"
            } else {
                "error"
            },
            backend_error.message(),
            None,
            Some(&target_url),
        )
        .ok();
    }

    ACTIVE_RECORDINGS.lock().await.remove(&job_id);
    result
}

#[tauri::command]
pub async fn cancel_tiktok_live_recording(job_id: String) -> Result<(), String> {
    let mut recordings = ACTIVE_RECORDINGS.lock().await;
    if let Some(cancel_tx) = recordings.remove(&job_id) {
        cancel_tx.send(()).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let variants = variants_from_tiktok_stream_data(&stream_data);
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
        assert!(enabled
            .windows(2)
            .any(|args| args == ["-reconnect_max_retries", "20"]));
        assert!(enabled
            .windows(2)
            .any(|args| args == ["-reconnect_delay_total_max", "120"]));
        assert!(enabled
            .windows(2)
            .any(|args| args == ["-reconnect_on_http_error", "408,429,5xx"]));

        let mut disabled = Vec::new();
        append_reconnect_args(&mut disabled, false);
        assert!(disabled.is_empty());
    }

    #[test]
    fn prefers_hls_over_flv_when_resolution_matches() {
        let stream_data = serde_json::json!({
            "data": {
                "hd": {
                    "main": {
                        "flv": "https://signed.example/hd.flv",
                        "hls": "https://signed.example/hd.m3u8",
                        "sdk_params": "{\"resolution\":\"1280x720\",\"vbitrate\":2500000}"
                    }
                }
            }
        });

        let variants = variants_from_tiktok_stream_data(&stream_data);
        let selected = select_variant(&variants, &None, &None).expect("selected");

        assert_eq!(selected.format_id, "hd-hls");
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
            Some("creator's live.part-001.mp4")
        );
        assert_eq!(
            second.file_name().and_then(|value| value.to_str()),
            Some("creator's live.part-002.mp4")
        );

        let manifest = ffconcat_content(&[first, second]);
        assert!(manifest.contains("creator'\\''s live.part-001.mp4"));
        assert!(manifest.contains("creator'\\''s live.part-002.mp4"));
        assert_eq!(manifest.lines().count(), 2);
    }
}
