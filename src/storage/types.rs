use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct StageFileRequest {
    pub source_path: PathBuf,
    pub source_name: String,
    pub output_extension: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedObject {
    pub provider: String,
    pub bucket: String,
    pub key: String,
    pub public_url: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredObject {
    #[serde(flatten)]
    pub planned: PlannedObject,
    pub size_bytes: u64,
}
