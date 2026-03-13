use std::path::Path;
use std::process::Command;

use crate::error::CommandError;
use crate::media::NormalizationPlan;
use crate::media::input::MediaProbe;
use crate::media::profile::{MediaProfile, fit_dimensions, validate_aspect_ratio};

pub fn build_image_plan(
    probe: &MediaProbe,
    profile: &MediaProfile,
) -> Result<NormalizationPlan, CommandError> {
    validate_aspect_ratio(profile, probe.width, probe.height)?;
    let (output_width, output_height, upscaled) = fit_dimensions(
        probe.width,
        probe.height,
        profile.max_width,
        profile.max_height,
        true,
    );

    Ok(NormalizationPlan {
        profile: profile.name.to_owned(),
        output_width,
        output_height,
        output_extension: profile.output_extension.to_owned(),
        content_type: profile.content_type.to_owned(),
        upscaled,
        source_frame_rate: None,
        output_frame_rate: None,
    })
}

pub fn normalize_image(
    source_path: &Path,
    output_path: &Path,
    plan: &NormalizationPlan,
) -> Result<(), CommandError> {
    let scale = format!("scale={}:{}", plan.output_width, plan.output_height);
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            source_path.to_str().unwrap_or_default(),
            "-vf",
            &scale,
            "-frames:v",
            "1",
            "-update",
            "1",
            "-an",
            "-map_metadata",
            "-1",
            "-q:v",
            "2",
            "-pix_fmt",
            "yuvj420p",
            output_path.to_str().unwrap_or_default(),
        ])
        .status()
        .map_err(|error| {
            CommandError::blocked(
                "MEDIA_NORMALIZATION_FAILED",
                format!("failed to execute ffmpeg for image normalization: {error}"),
                "Install ffmpeg and ensure it is available on PATH",
            )
        })?;

    if !status.success() {
        return Err(CommandError::failure(
            "MEDIA_NORMALIZATION_FAILED",
            format!(
                "ffmpeg failed to normalize image `{}`",
                source_path.display()
            ),
            "Check that the source image is valid and supported by ffmpeg",
        ));
    }

    Ok(())
}
