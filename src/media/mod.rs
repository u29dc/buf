pub mod image;
pub mod input;
pub mod pipeline;
pub mod profile;
pub mod video;

pub use pipeline::prepare_media_for_post;

use serde::Serialize;
use serde_json::Value;

use crate::storage::{PlannedObject, StoredObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
}

impl MediaKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }

    #[must_use]
    pub const fn asset_input_name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizationPlan {
    pub profile: String,
    pub output_width: u32,
    pub output_height: u32,
    pub output_extension: String,
    pub content_type: String,
    pub upscaled: bool,
    pub source_frame_rate: Option<f64>,
    pub output_frame_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedMedia {
    pub uploaded: bool,
    pub provider: String,
    pub bucket: String,
    pub key: String,
    pub public_url: String,
    pub content_type: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMedia {
    pub source: String,
    pub input_type: String,
    pub kind: String,
    pub source_width: Option<u32>,
    pub source_height: Option<u32>,
    pub normalization: Option<NormalizationPlan>,
    pub staged: Option<StagedMedia>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMediaBundle {
    pub items: Vec<PreparedMedia>,
    pub effective_post_type: Option<String>,
    pub asset_kind: Option<String>,
    #[serde(skip_serializing)]
    pub assets: Value,
}

impl StagedMedia {
    #[must_use]
    pub fn from_planned(planned: &PlannedObject) -> Self {
        Self {
            uploaded: false,
            provider: planned.provider.clone(),
            bucket: planned.bucket.clone(),
            key: planned.key.clone(),
            public_url: planned.public_url.clone(),
            content_type: planned.content_type.clone(),
            size_bytes: None,
        }
    }

    #[must_use]
    pub fn from_stored(stored: &StoredObject) -> Self {
        Self {
            uploaded: true,
            provider: stored.planned.provider.clone(),
            bucket: stored.planned.bucket.clone(),
            key: stored.planned.key.clone(),
            public_url: stored.planned.public_url.clone(),
            content_type: stored.planned.content_type.clone(),
            size_bytes: Some(stored.size_bytes),
        }
    }
}
