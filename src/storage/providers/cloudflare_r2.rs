use std::fs;
use std::path::Path;

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use tokio::runtime::{Builder, Runtime};

use crate::config::MediaCredentials;
use crate::error::CommandError;
use crate::storage::types::{PlannedObject, StoredObject};

pub struct CloudflareR2Provider {
    bucket: String,
    base_url: String,
    key_prefix: String,
    client: Client,
    runtime: Runtime,
}

impl CloudflareR2Provider {
    pub fn new(config: &MediaCredentials) -> Result<Self, CommandError> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                CommandError::blocked(
                    "STORAGE_RUNTIME_INIT_FAILED",
                    format!("failed to initialize async runtime for R2: {error}"),
                    "Check the local Rust runtime environment and retry",
                )
            })?;

        let client = runtime.block_on(async {
            let credentials = Credentials::new(
                &config.access_key_id,
                &config.secret_access_key,
                None,
                None,
                "buf",
            );

            let shared_config = aws_config::defaults(BehaviorVersion::latest())
                .region("auto")
                .endpoint_url(&config.endpoint)
                .credentials_provider(credentials)
                .load()
                .await;

            let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
                .force_path_style(true)
                .build();

            Client::from_conf(s3_config)
        });

        Ok(Self {
            bucket: config.bucket.clone(),
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            key_prefix: config.key_prefix.trim_matches('/').to_owned(),
            client,
            runtime,
        })
    }

    #[must_use]
    pub fn plan_object(
        &self,
        source_name: &str,
        output_extension: &str,
        content_type: &str,
    ) -> PlannedObject {
        let timestamp = Utc::now();
        let date_prefix = timestamp.format("%Y/%m/%d").to_string();
        let time_prefix = timestamp.format("%Y%m%dT%H%M%S%6f").to_string();
        let sanitized = sanitize_stem(source_name);
        let extension = output_extension
            .trim_start_matches('.')
            .to_ascii_lowercase();
        let key = format!(
            "{}/{}/{}-{}.{}",
            self.key_prefix, date_prefix, time_prefix, sanitized, extension
        );

        PlannedObject {
            provider: "cloudflare-r2".to_owned(),
            bucket: self.bucket.clone(),
            key: key.clone(),
            public_url: format!("{}/{}", self.base_url, key),
            content_type: content_type.to_owned(),
        }
    }

    pub fn put_object(
        &self,
        planned: &PlannedObject,
        source_path: &Path,
    ) -> Result<StoredObject, CommandError> {
        let file_size = fs::metadata(source_path)
            .map(|metadata| metadata.len())
            .map_err(|error| {
                CommandError::failure(
                    "STORAGE_UPLOAD_FAILED",
                    format!(
                        "failed to inspect staged file `{}` before upload: {error}",
                        source_path.display()
                    ),
                    "Check the local staged file path and retry",
                )
            })?;

        self.runtime.block_on(async {
            let body = ByteStream::from_path(source_path.to_path_buf())
                .await
                .map_err(|error| {
                    CommandError::failure(
                        "STORAGE_UPLOAD_FAILED",
                        format!(
                            "failed to read staged file `{}` for upload: {error}",
                            source_path.display()
                        ),
                        "Check the local staged file path and retry",
                    )
                })?;

            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&planned.key)
                .body(body)
                .content_type(&planned.content_type)
                .send()
                .await
                .map_err(|error| {
                    CommandError::failure(
                        "STORAGE_UPLOAD_FAILED",
                        format!("R2 upload failed for `{}`: {error}", planned.key),
                        "Check the R2 credentials, endpoint, bucket access, and public bucket configuration",
                    )
                })?;

            Ok(StoredObject {
                planned: planned.clone(),
                size_bytes: file_size,
            })
        })
    }
}

fn sanitize_stem(source_name: &str) -> String {
    let stem = Path::new(source_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let mut sanitized = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "asset".to_owned()
    } else {
        trimmed.to_owned()
    }
}
