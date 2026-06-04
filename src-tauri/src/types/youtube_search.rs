use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct YoutubeSearchVideo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration: Option<String>,
    pub channel: Option<String>,
    pub view_count_text: Option<String>,
    pub published_time_text: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct YoutubeSearchResponse {
    pub videos: Vec<YoutubeSearchVideo>,
    pub continuation: Option<String>,
}
