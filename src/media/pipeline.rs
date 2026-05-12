use std::fs;

use serde_json::{Value, json};
use tempfile::Builder;

use crate::cli::InstagramPostType;
use crate::config::RuntimeContext;
use crate::error::CommandError;
use crate::media::image::{build_image_plan, normalize_image};
use crate::media::input::{
    LocalMediaReference, MediaReference, RemoteMediaReference, parse_media_references,
};
use crate::media::profile::resolve_profile;
use crate::media::video::{build_video_plan, normalize_video};
use crate::media::{MediaKind, PreparedMedia, PreparedMediaBundle, StagedMedia};
use crate::storage::StorageService;
use crate::storage::types::StageFileRequest;

pub fn prepare_media_for_post(
    runtime: &RuntimeContext,
    service: &str,
    requested_post_type: Option<InstagramPostType>,
    values: &[String],
    dry_run: bool,
) -> Result<PreparedMediaBundle, CommandError> {
    if values.is_empty() {
        return Ok(PreparedMediaBundle {
            items: Vec::new(),
            effective_post_type: requested_post_type.map(|post_type| post_type.as_str().to_owned()),
            asset_kind: None,
            assets: Value::Null,
        });
    }

    let references = parse_media_references(values)?;
    let kind = media_kind(&references)?;
    let decision = resolve_profile(service, requested_post_type, kind, references.len())?;

    let requires_storage = references
        .iter()
        .any(|reference| matches!(reference, MediaReference::Local(_)));
    let storage = if requires_storage {
        Some(StorageService::from_runtime(runtime)?)
    } else {
        None
    };
    let temp_dir = if requires_storage && !dry_run {
        fs::create_dir_all(&runtime.paths.temp_dir).map_err(|error| {
            CommandError::blocked(
                "MEDIA_TEMP_DIR_UNAVAILABLE",
                format!(
                    "failed to create temp directory `{}`: {error}",
                    runtime.paths.temp_dir.display()
                ),
                "Ensure BUF_HOME is writable and retry",
            )
        })?;
        Some(
            Builder::new()
                .prefix("media-")
                .tempdir_in(&runtime.paths.temp_dir)
                .map_err(|error| {
                    CommandError::blocked(
                        "MEDIA_TEMP_DIR_UNAVAILABLE",
                        format!(
                            "failed to create temp work directory in `{}`: {error}",
                            runtime.paths.temp_dir.display()
                        ),
                        "Ensure BUF_HOME is writable and retry",
                    )
                })?,
        )
    } else {
        None
    };

    let mut items = Vec::with_capacity(references.len());
    let mut asset_urls = Vec::with_capacity(references.len());

    for (index, reference) in references.iter().enumerate() {
        match reference {
            MediaReference::Local(local) => {
                let storage = storage.as_ref().expect("storage required for local media");
                let prepared = prepare_local_media(
                    local,
                    &decision.profile,
                    storage,
                    temp_dir.as_ref().map(|dir| dir.path()),
                    dry_run,
                    index,
                )?;
                let asset_url = prepared
                    .staged
                    .as_ref()
                    .map(|staged| staged.public_url.clone())
                    .ok_or_else(|| {
                        CommandError::failure(
                            "STORAGE_UPLOAD_FAILED",
                            "local media staging did not produce a public URL",
                            "Retry the command and inspect the staged media payload",
                        )
                    })?;
                asset_urls.push(asset_url);
                items.push(prepared);
            }
            MediaReference::Remote(remote) => {
                let prepared = prepare_remote_media(remote);
                asset_urls.push(remote.raw.clone());
                items.push(prepared);
            }
        }
    }

    let assets = asset_urls
        .iter()
        .map(|url| json!({ kind.asset_input_name(): { "url": url } }))
        .collect::<Value>();

    Ok(PreparedMediaBundle {
        items,
        effective_post_type: decision.effective_post_type,
        asset_kind: Some(kind.as_str().to_owned()),
        assets,
    })
}

fn media_kind(references: &[MediaReference]) -> Result<MediaKind, CommandError> {
    let Some(first) = references.first() else {
        return Err(CommandError::failure(
            "MEDIA_INPUT_INVALID",
            "media list cannot be empty",
            "Provide at least one --media value or remove the flag entirely",
        ));
    };

    let first_kind = match first {
        MediaReference::Local(local) => local.kind,
        MediaReference::Remote(remote) => remote.kind,
    };

    for reference in references.iter().skip(1) {
        let next_kind = match reference {
            MediaReference::Local(local) => local.kind,
            MediaReference::Remote(remote) => remote.kind,
        };
        if next_kind != first_kind {
            return Err(CommandError::failure(
                "MEDIA_COMBINATION_UNSUPPORTED",
                "mixed image and video media is not supported in this prototype",
                "Use either images or a single video in one post",
            ));
        }
    }

    Ok(first_kind)
}

fn prepare_local_media(
    local: &LocalMediaReference,
    profile: &crate::media::profile::MediaProfile,
    storage: &StorageService,
    temp_dir: Option<&std::path::Path>,
    dry_run: bool,
    index: usize,
) -> Result<PreparedMedia, CommandError> {
    let mut warnings = Vec::new();
    let plan = match local.kind {
        MediaKind::Image => build_image_plan(&local.probe, profile)?,
        MediaKind::Video => build_video_plan(&local.probe, profile)?,
    };
    if plan.upscaled {
        warnings.push(format!(
            "source media `{}` was smaller than the target profile and will be upscaled",
            local.file_name
        ));
    }

    let staged = if dry_run {
        let planned =
            storage.plan_file(&local.file_name, &plan.output_extension, &plan.content_type);
        Some(StagedMedia::from_planned(&planned))
    } else {
        let temp_dir = temp_dir.expect("temp dir required when executing local media pipeline");
        let output_path = temp_dir.join(format!(
            "normalized-{}-{}.{}",
            index,
            local.kind.as_str(),
            plan.output_extension
        ));
        match local.kind {
            MediaKind::Image => normalize_image(&local.path, &output_path, &plan)?,
            MediaKind::Video => normalize_video(&local.path, &output_path, &plan)?,
        }
        let stored = storage.stage_file(&StageFileRequest {
            source_path: output_path,
            source_name: local.file_name.clone(),
            output_extension: plan.output_extension.clone(),
            content_type: plan.content_type.clone(),
        })?;
        Some(StagedMedia::from_stored(&stored))
    };

    Ok(PreparedMedia {
        source: local.raw.clone(),
        input_type: "local".to_owned(),
        kind: local.kind.as_str().to_owned(),
        source_width: Some(local.probe.width),
        source_height: Some(local.probe.height),
        normalization: Some(plan),
        staged,
        warnings,
    })
}

fn prepare_remote_media(remote: &RemoteMediaReference) -> PreparedMedia {
    PreparedMedia {
        source: remote.raw.clone(),
        input_type: "url".to_owned(),
        kind: remote.kind.as_str().to_owned(),
        source_width: None,
        source_height: None,
        normalization: None,
        staged: Some(StagedMedia {
            uploaded: false,
            provider: "remote".to_owned(),
            bucket: String::new(),
            key: remote.file_name.clone(),
            public_url: remote.raw.clone(),
            content_type: remote.content_type.clone(),
            size_bytes: None,
        }),
        warnings: Vec::new(),
    }
}
