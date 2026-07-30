use crate::services::build_browser_cookie_header;
use crate::utils::normalize_url;
use reqwest::{
    header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, USER_AGENT},
    redirect::Policy,
    Client, Proxy, Url,
};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const FACEBOOK_STORY_MAX_HTML_BYTES: usize = 10 * 1024 * 1024;
const FACEBOOK_STORY_CARD_SCAN_BYTES: usize = 256 * 1024;
const FACEBOOK_STORY_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const FACEBOOK_STORY_CACHE_LIMIT: usize = 64;
const FACEBOOK_BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";

static FACEBOOK_STORY_CACHE: LazyLock<Mutex<HashMap<String, (Instant, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn facebook_story_token(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    if host != "facebook.com" && !host.ends_with(".facebook.com") {
        return None;
    }

    let segments: Vec<_> = parsed.path_segments()?.collect();
    (segments.first() == Some(&"stories"))
        .then(|| segments.get(2).filter(|token| !token.is_empty()))
        .flatten()
        .map(|token| (*token).to_string())
}

fn json_string_after(input: &str, marker: &str) -> Option<String> {
    let value = input
        .get(input.find(marker)? + marker.len()..)?
        .trim_start();
    serde_json::Deserializer::from_str(value)
        .into_iter::<String>()
        .next()?
        .ok()
}

fn canonical_facebook_media_url(value: &str) -> Option<String> {
    let mut parsed = Url::parse(value).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    if host != "facebook.com" && !host.ends_with(".facebook.com") {
        return None;
    }

    let segments: Vec<_> = parsed
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let is_reel =
        matches!(segments.as_slice(), ["reel", id] if id.chars().all(|c| c.is_ascii_digit()));
    let is_video =
        matches!(segments.as_slice(), [_, "videos", id] if id.chars().all(|c| c.is_ascii_digit()));
    if !is_reel && !is_video {
        return None;
    }

    if is_reel {
        return Some(normalize_url(value));
    }

    parsed.set_scheme("https").ok()?;
    parsed.set_host(Some("www.facebook.com")).ok()?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string().trim_end_matches('/').to_string())
}

fn facebook_story_media_url(story_url: &str, html: &str) -> Option<String> {
    let token = facebook_story_token(story_url)?;
    let story_marker = format!("\"id\":\"{token}\"");
    let story_start = html.find(&story_marker)?;
    let story = &html[story_start..html.len().min(story_start + FACEBOOK_STORY_CARD_SCAN_BYTES)];

    for marker in [
        "\"fb_shorts_story\":{\"url\":",
        "\"story_video_thumbnail\":{\"url\":",
    ] {
        if let Some(url) =
            json_string_after(story, marker).and_then(|value| canonical_facebook_media_url(&value))
        {
            return Some(url);
        }
    }

    None
}

async fn cached_story_url(story_url: &str) -> Option<String> {
    let mut cache = FACEBOOK_STORY_CACHE.lock().await;
    cache.retain(|_, (created_at, _)| created_at.elapsed() < FACEBOOK_STORY_CACHE_TTL);
    cache.get(story_url).map(|(_, url)| url.clone())
}

async fn cache_story_url(story_url: String, media_url: String) {
    let mut cache = FACEBOOK_STORY_CACHE.lock().await;
    cache.retain(|_, (created_at, _)| created_at.elapsed() < FACEBOOK_STORY_CACHE_TTL);
    if cache.len() >= FACEBOOK_STORY_CACHE_LIMIT {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, (created_at, _))| *created_at)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(story_url, (Instant::now(), media_url));
}

async fn fetch_facebook_story_html(
    story_url: &str,
    cookie_header: Option<String>,
    proxy_url: Option<&str>,
) -> Result<String, String> {
    let mut builder = Client::builder()
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(20));
    if let Some(proxy_url) = proxy_url.filter(|value| !value.trim().is_empty()) {
        builder = builder.proxy(
            Proxy::all(proxy_url)
                .map_err(|error| format!("Invalid proxy for Facebook Story: {error}"))?,
        );
    }
    let client = builder
        .build()
        .map_err(|error| format!("Failed to prepare Facebook Story request: {error}"))?;

    let mut request = client
        .get(story_url)
        .header(USER_AGENT, FACEBOOK_BROWSER_USER_AGENT)
        .header(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        )
        .header(ACCEPT_LANGUAGE, "vi,en-US;q=0.7,en;q=0.3")
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "none")
        .header("Sec-Fetch-User", "?1")
        .header("Upgrade-Insecure-Requests", "1");
    if let Some(cookie_header) = cookie_header {
        request = request.header(COOKIE, cookie_header);
    }

    let mut response = request
        .send()
        .await
        .map_err(|error| format!("Failed to open Facebook Story: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Facebook Story returned HTTP {}.",
            response.status()
        ));
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Failed to read Facebook Story: {error}"))?
    {
        if body.len() + chunk.len() > FACEBOOK_STORY_MAX_HTML_BYTES {
            return Err("Facebook Story page is too large to inspect safely.".to_string());
        }
        body.extend_from_slice(&chunk);
    }

    String::from_utf8(body)
        .map_err(|_| "Facebook Story returned invalid page encoding.".to_string())
}

pub async fn resolve_facebook_media_url(
    url: &str,
    cookie_mode: Option<&str>,
    cookie_browser: Option<&str>,
    cookie_browser_profile: Option<&str>,
    cookie_file_path: Option<&str>,
    cookie_skip_patterns: Option<&[String]>,
    proxy_url: Option<&str>,
) -> Result<String, String> {
    let normalized = normalize_url(url);
    if facebook_story_token(&normalized).is_none() {
        return Ok(normalized);
    }
    if let Some(cached) = cached_story_url(&normalized).await {
        return Ok(cached);
    }

    let cookie_header = build_browser_cookie_header(
        &normalized,
        cookie_mode,
        cookie_browser,
        cookie_browser_profile,
        cookie_file_path,
        cookie_skip_patterns,
    );
    let html = fetch_facebook_story_html(&normalized, cookie_header, proxy_url).await?;
    let media_url = facebook_story_media_url(&normalized, &html).ok_or_else(|| {
        "Could not resolve this Facebook Story to a downloadable Reel. Keep Firefox signed in and select its profile under Network & Authentication.".to_string()
    })?;
    cache_story_url(normalized, media_url.clone()).await;
    Ok(media_url)
}

#[cfg(test)]
mod tests {
    use super::{facebook_story_media_url, facebook_story_token};

    const STORY_URL: &str = "https://www.facebook.com/stories/103239061171689/UzpfSVNDOjI3MzU2ODU2NjgwNjUxODk2?view_single=false";

    #[test]
    fn resolves_reshared_reel_from_target_facebook_story() {
        let html = r#"
            {"id":"other","story_card_info":{"story_overlays":[{"fb_shorts_story":{"url":"https:\/\/www.facebook.com\/reel\/111\/"}}]}}
            {"id":"UzpfSVNDOjI3MzU2ODU2NjgwNjUxODk2","story_card_info":{"story_overlays":[{"fb_shorts_story":{"url":"https:\/\/www.facebook.com\/reel\/920653751079018\/"}}]}}
        "#;

        assert_eq!(
            facebook_story_media_url(STORY_URL, html).as_deref(),
            Some("https://www.facebook.com/reel/920653751079018")
        );
    }

    #[test]
    fn resolves_direct_story_video_when_no_reel_overlay_exists() {
        let html = r#"
            {"id":"UzpfSVNDOjI3MzU2ODU2NjgwNjUxODk2","story_card_info":{"story_video_thumbnail":{"url":"https:\/\/www.facebook.com\/kimthoatour\/videos\/1418406220139762\/"}}}
        "#;

        assert_eq!(
            facebook_story_media_url(STORY_URL, html).as_deref(),
            Some("https://www.facebook.com/kimthoatour/videos/1418406220139762")
        );
    }

    #[test]
    fn recognizes_only_facebook_story_urls() {
        assert_eq!(
            facebook_story_token(STORY_URL).as_deref(),
            Some("UzpfSVNDOjI3MzU2ODU2NjgwNjUxODk2")
        );
        assert_eq!(
            facebook_story_token("https://www.facebook.com/reel/920653751079018"),
            None
        );
        assert_eq!(
            facebook_story_token(
                "https://notfacebook.com/stories/103239061171689/UzpfSVNDOjI3MzU2ODU2NjgwNjUxODk2"
            ),
            None
        );
    }
}
