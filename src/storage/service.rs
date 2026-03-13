use crate::config::RuntimeContext;
use crate::error::CommandError;
use crate::storage::config::require_media_credentials;
use crate::storage::providers::cloudflare_r2::CloudflareR2Provider;
use crate::storage::types::{PlannedObject, StageFileRequest, StoredObject};

pub struct StorageService {
    provider: CloudflareR2Provider,
}

impl StorageService {
    pub fn from_runtime(runtime: &RuntimeContext) -> Result<Self, CommandError> {
        let credentials = require_media_credentials(runtime)?;
        let provider = CloudflareR2Provider::new(&credentials)?;
        Ok(Self { provider })
    }

    #[must_use]
    pub fn plan_file(
        &self,
        source_name: &str,
        output_extension: &str,
        content_type: &str,
    ) -> PlannedObject {
        self.provider
            .plan_object(source_name, output_extension, content_type)
    }

    pub fn stage_file(&self, request: &StageFileRequest) -> Result<StoredObject, CommandError> {
        let planned = self.plan_file(
            &request.source_name,
            &request.output_extension,
            &request.content_type,
        );
        self.provider.put_object(&planned, &request.source_path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::config::{PathOverrides, load_runtime};
    use crate::storage::StorageService;
    use crate::storage::types::StageFileRequest;

    #[test]
    #[ignore = "live R2 smoke test; run manually when BUF_HOME/.env is configured"]
    fn live_r2_upload_and_public_url_smoke() {
        let runtime =
            load_runtime(&PathOverrides::default(), None).expect("load runtime with real env");
        let service = StorageService::from_runtime(&runtime).expect("storage service");

        let temp_dir = TempDir::new().expect("temp dir");
        let source_path = temp_dir.path().join("smoke.txt");
        fs::write(&source_path, b"buf storage smoke").expect("write source file");

        let stored = service
            .stage_file(&StageFileRequest {
                source_path,
                source_name: "smoke.txt".to_owned(),
                output_extension: "txt".to_owned(),
                content_type: "text/plain".to_owned(),
            })
            .expect("stage file");

        assert!(stored.planned.public_url.starts_with("https://"));
        assert!(stored.planned.key.starts_with("tmp/buf/"));

        let response = reqwest::blocking::get(&stored.planned.public_url).expect("public GET");
        assert!(response.status().is_success());
    }
}
