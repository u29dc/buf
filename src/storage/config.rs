use crate::config::{MediaCredentials, RuntimeContext};
use crate::error::CommandError;

pub fn require_media_credentials(
    runtime: &RuntimeContext,
) -> Result<MediaCredentials, CommandError> {
    runtime.media_credentials.clone().ok_or_else(|| {
        CommandError::blocked(
            "STORAGE_CONFIG_MISSING",
            "R2 media storage is not fully configured",
            format!(
                "Set BUF_MEDIA_ENDPOINT, BUF_MEDIA_BUCKET, BUF_MEDIA_ACCESS_KEY_ID, BUF_MEDIA_SECRET_ACCESS_KEY, and BUF_MEDIA_BASE_URL in {}",
                runtime.paths.env_file.display()
            ),
        )
    })
}
