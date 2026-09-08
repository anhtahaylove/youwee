use std::sync::LazyLock;

/// Parsed yt-dlp progress line:
/// (percent, speed, eta, playlist_index, playlist_count, downloaded_size, elapsed_time)
pub type ProgressUpdate = (
    f64,
    String,
    String,
    Option<u32>,
    Option<u32>,
    Option<String>,
    Option<String>,
);

// yt-dlp emits progress lines continuously, so these patterns are compiled once
// instead of on every parsed line.
static PLAYLIST_ITEM_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"Downloading item (\d+) of (\d+)").expect("valid regex"));
static PERCENT_SPEED_ETA_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(\d+\.?\d*)%.*at\s+(\S+)(?:.*ETA\s+(\S+))?").expect("valid regex")
});
static PERCENT_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(\d+\.?\d*)%").expect("valid regex"));
static LIVE_FRAGMENT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"\[download\]\s+([\d.]+\s*\w+)\s+at\s+([\d.]+\s*\w+/s)\s*(?:\((\d{2}:\d{2}:\d{2})\))?",
    )
    .expect("valid regex")
});

/// Parse yt-dlp progress output
pub fn parse_progress(line: &str) -> Option<ProgressUpdate> {
    let mut playlist_index: Option<u32> = None;
    let mut playlist_count: Option<u32> = None;

    // Check for playlist progress
    if line.contains("Downloading item") {
        if let Some(caps) = PLAYLIST_ITEM_RE.captures(line) {
            playlist_index = caps.get(1).and_then(|m| m.as_str().parse().ok());
            playlist_count = caps.get(2).and_then(|m| m.as_str().parse().ok());
        }
    }

    // Standard progress with percentage (normal videos)
    if line.contains("[download]") && line.contains("%") {
        // Try to match with speed and ETA
        if line.contains(" at ") {
            if let Some(caps) = PERCENT_SPEED_ETA_RE.captures(line) {
                let percent: f64 = caps.get(1)?.as_str().parse().ok()?;
                let speed = caps
                    .get(2)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let eta = caps
                    .get(3)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                return Some((
                    percent,
                    speed,
                    eta,
                    playlist_index,
                    playlist_count,
                    None,
                    None,
                ));
            }
        }
        // Fallback: just extract percent (no speed/eta available)
        if let Some(caps) = PERCENT_RE.captures(line) {
            let percent: f64 = caps.get(1)?.as_str().parse().ok()?;
            return Some((
                percent,
                String::new(),
                String::new(),
                playlist_index,
                playlist_count,
                None,
                None,
            ));
        }
    }

    // Live stream progress: [download]    2.87MiB at  506.63KiB/s (00:00:07) (frag 91/2097)
    if line.contains("[download]") && !line.contains("%") && line.contains(" at ") {
        if let Some(caps) = LIVE_FRAGMENT_RE.captures(line) {
            let downloaded_size = caps.get(1).map(|m| m.as_str().trim().to_string());
            let speed = caps
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let elapsed_time = caps.get(3).map(|m| m.as_str().to_string());
            return Some((
                0.0,
                speed,
                String::new(),
                playlist_index,
                playlist_count,
                downloaded_size,
                elapsed_time,
            ));
        }
    }

    None
}
