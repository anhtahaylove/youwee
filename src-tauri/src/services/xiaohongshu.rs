use std::collections::HashMap;

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XiaohongshuGalleryImage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XiaohongshuGalleryMetadata {
    pub id: String,
    pub title: String,
    pub images: Vec<XiaohongshuGalleryImage>,
}

pub fn is_xiaohongshu_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };

    host == "xhslink.com"
        || host.ends_with(".xhslink.com")
        || host == "xiaohongshu.com"
        || host.ends_with(".xiaohongshu.com")
}

pub fn is_xiaohongshu_short_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };

    host == "xhslink.com" || host.ends_with(".xhslink.com")
}

fn is_xiaohongshu_extractor(json: &Value) -> bool {
    json.get("extractor")
        .and_then(Value::as_str)
        .or_else(|| json.get("extractor_key").and_then(Value::as_str))
        .is_some_and(|value| {
            let normalized = value.to_ascii_lowercase();
            normalized.contains("xiaohongshu")
        })
}

fn is_xiaohongshu_cdn_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };

    matches!(parsed.scheme(), "http" | "https")
        && (host == "xhscdn.com" || host.ends_with(".xhscdn.com"))
}

fn gallery_asset_key(url: &str) -> String {
    let leaf = reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .unwrap_or_else(|| url.to_string());

    leaf.split("!nd_").next().unwrap_or(&leaf).to_string()
}

fn is_original_image(url: &str) -> bool {
    url.contains("!nd_dft_")
}

pub fn parse_xiaohongshu_gallery_metadata(json: &Value) -> Option<XiaohongshuGalleryMetadata> {
    if !is_xiaohongshu_extractor(json)
        || json
            .get("formats")
            .and_then(Value::as_array)
            .is_some_and(|formats| !formats.is_empty())
    {
        return None;
    }

    let thumbnails = json.get("thumbnails")?.as_array()?;
    let mut images = Vec::<XiaohongshuGalleryImage>::new();
    let mut asset_indexes = HashMap::<String, usize>::new();

    for thumbnail in thumbnails {
        let Some(url) = thumbnail
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| is_xiaohongshu_cdn_url(url))
        else {
            continue;
        };
        let candidate = XiaohongshuGalleryImage {
            url: url.to_string(),
            width: thumbnail
                .get("width")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
            height: thumbnail
                .get("height")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
        };
        let asset_key = gallery_asset_key(url);

        if let Some(index) = asset_indexes.get(&asset_key).copied() {
            if is_original_image(url) && !is_original_image(&images[index].url) {
                images[index] = candidate;
            }
        } else {
            asset_indexes.insert(asset_key, images.len());
            images.push(candidate);
        }
    }

    if images.is_empty() {
        return None;
    }

    let id = json
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("post")
        .to_string();
    let title = json
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Xiaohongshu post {id}"));

    Some(XiaohongshuGalleryMetadata { id, title, images })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_xiaohongshu_hosts() {
        assert!(is_xiaohongshu_url("http://xhslink.com/o/example"));
        assert!(is_xiaohongshu_url(
            "https://www.xiaohongshu.com/explore/example"
        ));
        assert!(!is_xiaohongshu_url(
            "https://example.com/xhslink.com/o/example"
        ));
        assert!(is_xiaohongshu_short_url("http://xhslink.com/o/example"));
        assert!(!is_xiaohongshu_short_url(
            "https://www.xiaohongshu.com/explore/example"
        ));
    }

    #[test]
    fn image_posts_prefer_original_assets_and_dedupe_previews() {
        let json = serde_json::json!({
            "id": "post-1",
            "title": "Seven images",
            "extractor": "XiaoHongShu",
            "formats": [],
            "thumbnails": [
                {"url": "https://sns-webpic-qc.xhscdn.com/asset-a!nd_prv_wlteh_jpg_3", "width": 720, "height": 960},
                {"url": "https://sns-webpic-qc.xhscdn.com/asset-b!nd_dft_wlteh_jpg_3", "width": 1440, "height": 1920},
                {"url": "https://sns-webpic-qc.xhscdn.com/asset-a!nd_dft_wlteh_jpg_3", "width": 1440, "height": 1920},
                {"url": "https://sns-webpic-qc.xhscdn.com/asset-b!nd_prv_wlteh_jpg_3", "width": 720, "height": 960}
            ]
        });

        let gallery = parse_xiaohongshu_gallery_metadata(&json).expect("image gallery");
        assert_eq!(gallery.images.len(), 2);
        assert!(gallery
            .images
            .iter()
            .all(|image| image.url.contains("!nd_dft_")));
        assert_eq!(gallery.images[0].width, Some(1440));
    }

    #[test]
    fn video_posts_stay_on_the_normal_download_path() {
        let json = serde_json::json!({
            "id": "video-1",
            "extractor": "XiaoHongShu",
            "formats": [{"format_id": "h264"}],
            "thumbnails": [{"url": "https://sns-webpic-qc.xhscdn.com/cover!nd_dft_wlteh_jpg_3"}]
        });

        assert!(parse_xiaohongshu_gallery_metadata(&json).is_none());
    }

    #[test]
    fn rejects_non_xiaohongshu_thumbnail_hosts() {
        let json = serde_json::json!({
            "id": "post-1",
            "extractor": "XiaoHongShu",
            "formats": [],
            "thumbnails": [{"url": "https://example.com/asset!nd_dft_wlteh_jpg_3"}]
        });

        assert!(parse_xiaohongshu_gallery_metadata(&json).is_none());
    }
}
