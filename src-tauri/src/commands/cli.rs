use std::sync::Mutex;
use tauri::Url;

static PENDING_CLI_DOWNLOAD_REQUESTS: Mutex<Vec<CliDownloadRequest>> = Mutex::new(Vec::new());
const MAX_PENDING_CLI_DOWNLOAD_REQUESTS: usize = 100;
const MAX_CLI_URL_LENGTH: usize = 2048;

// Allowlist of accepted quality values (mirrors frontend parseEnqueueOptions).
const ALLOWED_VIDEO_QUALITIES: [&str; 8] = ["best", "8k", "4k", "2k", "1080", "720", "480", "360"];
const ALLOWED_AUDIO_QUALITIES: [&str; 2] = ["128", "auto"];

#[derive(Clone, serde::Serialize)]
pub struct CliDownloadRequest {
    pub url: String,
    pub target: String,
    pub action: String,
    pub media: String,
    pub quality: String,
    pub trusted_local: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct ExternalCliDownloadEventPayload {
    pub requests: Vec<CliDownloadRequest>,
}

#[derive(Default)]
pub struct CliDownloadArgs {
    pub url: Option<String>,
    pub quality: Option<String>,
    pub audio: bool,
    pub queue_only: bool,
    pub target: Option<String>,
}

fn is_accepted_cli_url(url: &str) -> bool {
    if url.is_empty() || url.len() > MAX_CLI_URL_LENGTH {
        return false;
    }
    if url.contains(char::is_whitespace) {
        return false;
    }

    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }

    parsed
        .host_str()
        .map(|host| !is_private_or_local_host(host))
        .unwrap_or(false)
}

fn is_private_or_local_host(hostname: &str) -> bool {
    let host = hostname
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if host.is_empty() {
        return true;
    }

    if host == "localhost"
        || host == "0.0.0.0"
        || host == "::"
        || host == "::1"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return true;
    }

    if host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("169.254.")
    {
        return true;
    }

    if let Some(second_octet) = host
        .strip_prefix("172.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|octet| octet.parse::<u8>().ok())
    {
        if (16..=31).contains(&second_octet) {
            return true;
        }
    }

    if host.contains(':') {
        return host.starts_with("fe80:") || host.starts_with("fc") || host.starts_with("fd");
    }

    false
}

/// Build a structured CLI download request from parsed CLI arguments.
/// Returns `None` when no usable URL is present.
pub fn build_cli_download_request(args: &CliDownloadArgs) -> Option<CliDownloadRequest> {
    let url = args.url.as_ref()?.trim();
    let url = url.trim_matches('"').trim_matches('\'');
    if !is_accepted_cli_url(url) {
        return None;
    }

    let media = if args.audio { "audio" } else { "video" }.to_string();
    let quality = normalize_cli_quality(args.quality.as_deref(), args.audio);
    let action = if args.queue_only {
        "queue_only"
    } else {
        "download_now"
    }
    .to_string();
    let target = normalize_cli_target(args.target.as_deref());

    Some(CliDownloadRequest {
        url: url.to_string(),
        target,
        action,
        media,
        quality,
        trusted_local: true,
    })
}

fn normalize_cli_quality(value: Option<&str>, audio: bool) -> String {
    let fallback = if audio { "auto" } else { "best" };
    let Some(quality) = value.map(|q| q.trim().to_ascii_lowercase()) else {
        return fallback.to_string();
    };
    let allowed = if audio {
        ALLOWED_AUDIO_QUALITIES.contains(&quality.as_str())
    } else {
        ALLOWED_VIDEO_QUALITIES.contains(&quality.as_str())
    };
    if allowed {
        quality
    } else {
        fallback.to_string()
    }
}

fn normalize_cli_target(value: Option<&str>) -> String {
    let Some(target) = value.map(|t| t.trim().to_ascii_lowercase()) else {
        return "auto".to_string();
    };
    if target == "youtube" || target == "universal" {
        target
    } else {
        "auto".to_string()
    }
}

/// Best-effort parser for raw argv (used by the single-instance callback where
/// only the raw process arguments are available). Supports the same flags as
/// the declared CLI schema. Unknown flags are ignored.
pub fn parse_cli_args_from_argv(argv: &[String]) -> CliDownloadArgs {
    let mut args = CliDownloadArgs::default();
    let mut iter = argv.iter().skip(1).peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--url" | "-u" => {
                if let Some(value) = iter.next() {
                    args.url = Some(value.clone());
                }
            }
            "--quality" | "-q" => {
                if let Some(value) = iter.next() {
                    args.quality = Some(value.clone());
                }
            }
            "--target" | "-t" => {
                if let Some(value) = iter.next() {
                    args.target = Some(value.clone());
                }
            }
            "--audio" | "-a" => args.audio = true,
            "--queue-only" => args.queue_only = true,
            other => {
                // Handle --flag=value form.
                if let Some(rest) = other.strip_prefix("--url=") {
                    args.url = Some(rest.to_string());
                } else if let Some(rest) = other.strip_prefix("--quality=") {
                    args.quality = Some(rest.to_string());
                } else if let Some(rest) = other.strip_prefix("--target=") {
                    args.target = Some(rest.to_string());
                } else if !other.starts_with('-') && args.url.is_none() {
                    // First positional argument is treated as the URL.
                    args.url = Some(other.to_string());
                }
            }
        }
    }

    args
}

/// Build a structured CLI download request from raw argv.
pub fn build_cli_download_request_from_argv(argv: &[String]) -> Option<CliDownloadRequest> {
    let args = parse_cli_args_from_argv(argv);
    build_cli_download_request(&args)
}

pub fn enqueue_cli_download_requests(requests: Vec<CliDownloadRequest>) {
    if requests.is_empty() {
        return;
    }
    if let Ok(mut pending) = PENDING_CLI_DOWNLOAD_REQUESTS.lock() {
        for request in requests {
            if !is_accepted_cli_url(&request.url) {
                continue;
            }
            if let Some(existing) = pending.iter_mut().find(|existing| {
                existing.url == request.url
                    && existing.target == request.target
                    && existing.action == request.action
                    && existing.media == request.media
                    && existing.quality == request.quality
            }) {
                existing.trusted_local = existing.trusted_local || request.trusted_local;
            } else {
                pending.push(request);
                if pending.len() > MAX_PENDING_CLI_DOWNLOAD_REQUESTS {
                    let overflow = pending.len() - MAX_PENDING_CLI_DOWNLOAD_REQUESTS;
                    pending.drain(0..overflow);
                }
            }
        }
    }
}

pub fn take_pending_cli_download_requests() -> Vec<CliDownloadRequest> {
    if let Ok(mut pending) = PENDING_CLI_DOWNLOAD_REQUESTS.lock() {
        return std::mem::take(&mut *pending);
    }
    Vec::new()
}

#[tauri::command]
pub fn consume_pending_cli_download_requests() -> Vec<CliDownloadRequest> {
    take_pending_cli_download_requests()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_request_preserves_url_without_deep_link_encoding() {
        let args = CliDownloadArgs {
            url: Some("https://www.youtube.com/watch?v=abc123&list=PL1#t=30".to_string()),
            quality: Some("720".to_string()),
            ..Default::default()
        };

        let request = build_cli_download_request(&args).expect("expected CLI request");

        assert_eq!(
            request.url,
            "https://www.youtube.com/watch?v=abc123&list=PL1#t=30"
        );
        assert_eq!(request.quality, "720");
        assert_eq!(request.action, "download_now");
        assert_eq!(request.media, "video");
        assert_eq!(request.target, "auto");
        assert!(request.trusted_local);
    }

    #[test]
    fn cli_request_rejects_non_http_urls() {
        let args = CliDownloadArgs {
            url: Some("file:///tmp/video.mp4".to_string()),
            ..Default::default()
        };

        assert!(build_cli_download_request(&args).is_none());
    }

    #[test]
    fn cli_request_rejects_private_urls() {
        let args = CliDownloadArgs {
            url: Some("http://localhost:8080/video".to_string()),
            ..Default::default()
        };

        assert!(build_cli_download_request(&args).is_none());
    }

    #[test]
    fn raw_argv_supports_positional_url_and_flags() {
        let argv = vec![
            "youwee".to_string(),
            "https://example.com/video".to_string(),
            "--quality=480".to_string(),
            "--queue-only".to_string(),
            "--target".to_string(),
            "universal".to_string(),
        ];

        let request = build_cli_download_request_from_argv(&argv).expect("expected CLI request");

        assert_eq!(request.url, "https://example.com/video");
        assert_eq!(request.quality, "480");
        assert_eq!(request.action, "queue_only");
        assert_eq!(request.target, "universal");
    }

    #[test]
    fn cli_request_falls_back_for_unsupported_values() {
        let args = CliDownloadArgs {
            url: Some("https://example.com/video".to_string()),
            quality: Some("999".to_string()),
            target: Some("desktop".to_string()),
            ..Default::default()
        };

        let request = build_cli_download_request(&args).expect("expected CLI request");

        assert_eq!(request.quality, "best");
        assert_eq!(request.target, "auto");
    }
}
