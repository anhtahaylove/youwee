use crate::types::{BackendError, YoutubeSearchResponse, YoutubeSearchVideo};
use reqwest::StatusCode;
use serde_json::{json, Value};

const YOUTUBE_SEARCH_API_URL: &str = "https://www.youtube.com/youtubei/v1/search?prettyPrint=false";
const YOUTUBE_WEB_CLIENT_NAME: &str = "WEB";
const YOUTUBE_WEB_CLIENT_VERSION: &str = "2.20240101.00.00";
const YOUTUBE_VIDEO_FILTER_PARAMS: &str = "EgIQAQ==";
const DEFAULT_SEARCH_LIMIT: u32 = 20;
const MAX_SEARCH_LIMIT: u32 = 100;

fn clamp_search_limit(limit: Option<u32>) -> usize {
    limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT) as usize
}

fn run_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.get("simpleText").and_then(|v| v.as_str()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let text = value
        .get("runs")
        .and_then(|runs| runs.as_array())
        .map(|runs| {
            runs.iter()
                .filter_map(|run| run.get("text").and_then(|v| v.as_str()))
                .collect::<String>()
        })
        .unwrap_or_default();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn best_thumbnail(renderer: &Value) -> Option<String> {
    renderer
        .get("thumbnail")
        .and_then(|thumbnail| thumbnail.get("thumbnails"))
        .and_then(|thumbnails| thumbnails.as_array())
        .and_then(|thumbnails| thumbnails.last())
        .and_then(|thumbnail| thumbnail.get("url"))
        .and_then(|url| url.as_str())
        .map(|url| url.replace("http://", "https://"))
}

fn parse_video_renderer(renderer: &Value) -> Option<YoutubeSearchVideo> {
    let id = renderer.get("videoId")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }

    let title = run_text(renderer.get("title"))?;
    if title.is_empty() {
        return None;
    }

    Some(YoutubeSearchVideo {
        id: id.to_string(),
        url: format!("https://www.youtube.com/watch?v={id}"),
        title,
        thumbnail: best_thumbnail(renderer),
        duration: run_text(renderer.get("lengthText")),
        channel: run_text(renderer.get("ownerText"))
            .or_else(|| run_text(renderer.get("longBylineText")))
            .or_else(|| run_text(renderer.get("shortBylineText"))),
        view_count_text: run_text(renderer.get("viewCountText"))
            .or_else(|| run_text(renderer.get("shortViewCountText"))),
        published_time_text: run_text(renderer.get("publishedTimeText")),
    })
}

fn collect_search_parts(
    value: &Value,
    videos: &mut Vec<YoutubeSearchVideo>,
    continuation: &mut Option<String>,
) {
    match value {
        Value::Object(map) => {
            if let Some(renderer) = map.get("videoRenderer") {
                if let Some(video) = parse_video_renderer(renderer) {
                    videos.push(video);
                }
            }

            if continuation.is_none() {
                if let Some(token) = map
                    .get("continuationItemRenderer")
                    .and_then(|renderer| renderer.get("continuationEndpoint"))
                    .and_then(|endpoint| endpoint.get("continuationCommand"))
                    .and_then(|command| command.get("token"))
                    .and_then(|token| token.as_str())
                    .filter(|token| !token.trim().is_empty())
                {
                    *continuation = Some(token.to_string());
                }
            }

            for child in map.values() {
                collect_search_parts(child, videos, continuation);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_search_parts(item, videos, continuation);
            }
        }
        _ => {}
    }
}

fn parse_youtube_search_response(json: &Value) -> YoutubeSearchResponse {
    let mut videos = Vec::new();
    let mut continuation = None;
    collect_search_parts(json, &mut videos, &mut continuation);
    videos.dedup_by(|a, b| a.id == b.id);
    YoutubeSearchResponse {
        videos,
        continuation,
    }
}

async fn fetch_search_page(
    client: &reqwest::Client,
    query: &str,
    continuation: Option<&str>,
) -> Result<YoutubeSearchResponse, String> {
    let mut body = json!({
        "context": {
            "client": {
                "clientName": YOUTUBE_WEB_CLIENT_NAME,
                "clientVersion": YOUTUBE_WEB_CLIENT_VERSION,
                "hl": "vi",
                "gl": "VN"
            }
        }
    });

    if let Some(token) = continuation {
        body["continuation"] = Value::String(token.to_string());
    } else {
        body["query"] = Value::String(query.to_string());
        body["params"] = Value::String(YOUTUBE_VIDEO_FILTER_PARAMS.to_string());
    }

    let response = client
        .post(YOUTUBE_SEARCH_API_URL)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let message = if e.is_timeout() {
                "YouTube search request timed out.".to_string()
            } else if e.is_connect() {
                "Unable to connect to YouTube search.".to_string()
            } else {
                format!("Failed to search YouTube: {e}")
            };
            BackendError::from_message(message).to_wire_string()
        })?;

    let status = response.status();
    if status != StatusCode::OK {
        return Err(
            BackendError::from_message(format!("YouTube search returned HTTP {status}"))
                .to_wire_string(),
        );
    }

    let json = response.json::<Value>().await.map_err(|e| {
        BackendError::from_message(format!("Failed to parse YouTube search response: {e}"))
            .to_wire_string()
    })?;

    Ok(parse_youtube_search_response(&json))
}

#[tauri::command]
pub async fn search_youtube_videos(
    query: String,
    limit: Option<u32>,
    continuation: Option<String>,
) -> Result<YoutubeSearchResponse, String> {
    let query = query.trim().to_string();
    let initial_continuation = continuation
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string);

    if query.is_empty() && initial_continuation.is_none() {
        return Err(BackendError::from_message("Search query is required.").to_wire_string());
    }

    let limit = clamp_search_limit(limit);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| {
            BackendError::from_message(format!("Failed to create YouTube search client: {e}"))
                .to_wire_string()
        })?;

    let mut videos = Vec::new();
    let mut next_continuation = initial_continuation;

    loop {
        let page = fetch_search_page(&client, &query, next_continuation.as_deref()).await?;
        for video in page.videos {
            if !videos
                .iter()
                .any(|existing: &YoutubeSearchVideo| existing.id == video.id)
            {
                videos.push(video);
            }
            if videos.len() >= limit {
                break;
            }
        }

        next_continuation = page.continuation;
        if videos.len() >= limit || next_continuation.is_none() {
            break;
        }
    }

    videos.truncate(limit);
    Ok(YoutubeSearchResponse {
        videos,
        continuation: next_continuation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_video_renderer_and_continuation() {
        let json = json!({
            "contents": {
                "itemSectionRenderer": {
                    "contents": [
                        {
                            "videoRenderer": {
                                "videoId": "abc123",
                                "title": {"runs": [{"text": "Test "}, {"text": "Video"}]},
                                "thumbnail": {
                                    "thumbnails": [
                                        {"url": "http://example.com/s.jpg", "width": 120},
                                        {"url": "https://example.com/l.jpg", "width": 720}
                                    ]
                                },
                                "lengthText": {"simpleText": "3:21"},
                                "ownerText": {"runs": [{"text": "Channel"}]},
                                "viewCountText": {"simpleText": "1,234 views"},
                                "publishedTimeText": {"simpleText": "1 day ago"}
                            }
                        },
                        {
                            "continuationItemRenderer": {
                                "continuationEndpoint": {
                                    "continuationCommand": {"token": "next-token"}
                                }
                            }
                        }
                    ]
                }
            }
        });

        let response = parse_youtube_search_response(&json);

        assert_eq!(response.continuation.as_deref(), Some("next-token"));
        assert_eq!(response.videos.len(), 1);
        assert_eq!(response.videos[0].id, "abc123");
        assert_eq!(response.videos[0].title, "Test Video");
        assert_eq!(
            response.videos[0].url,
            "https://www.youtube.com/watch?v=abc123"
        );
        assert_eq!(
            response.videos[0].thumbnail.as_deref(),
            Some("https://example.com/l.jpg")
        );
        assert_eq!(response.videos[0].duration.as_deref(), Some("3:21"));
        assert_eq!(response.videos[0].channel.as_deref(), Some("Channel"));
    }

    #[test]
    fn skips_renderers_without_video_id_or_title() {
        let json = json!({
            "items": [
                {"videoRenderer": {"title": {"simpleText": "Missing id"}}},
                {"videoRenderer": {"videoId": "missing-title"}}
            ]
        });

        let response = parse_youtube_search_response(&json);

        assert!(response.videos.is_empty());
        assert!(response.continuation.is_none());
    }

    #[test]
    fn handles_response_without_videos() {
        let json = json!({"contents": []});

        let response = parse_youtube_search_response(&json);

        assert!(response.videos.is_empty());
        assert!(response.continuation.is_none());
    }
}
