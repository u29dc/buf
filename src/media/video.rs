use std::path::Path;
use std::process::Command;

use crate::error::CommandError;
use crate::media::NormalizationPlan;
use crate::media::input::MediaProbe;
use crate::media::profile::{MediaProfile, fit_dimensions, validate_aspect_ratio};

pub fn build_video_plan(
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

    let output_frame_rate = probe.frame_rate.map(|fps| fps.min(30.0));

    Ok(NormalizationPlan {
        profile: profile.name.to_owned(),
        output_width,
        output_height,
        output_extension: profile.output_extension.to_owned(),
        content_type: profile.content_type.to_owned(),
        upscaled,
        source_frame_rate: probe.frame_rate,
        output_frame_rate,
    })
}

pub fn normalize_video(
    source_path: &Path,
    output_path: &Path,
    plan: &NormalizationPlan,
) -> Result<(), CommandError> {
    let scale = format!("scale={}:{}", plan.output_width, plan.output_height);
    let mut command = Command::new("ffmpeg");
    command.args([
        "-y",
        "-i",
        source_path.to_str().unwrap_or_default(),
        "-vf",
        &scale,
        "-map",
        "0:v:0",
        "-map",
        "0:a:0?",
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "20",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-map_metadata",
        "-1",
    ]);
    if let Some(frame_rate) = plan.output_frame_rate {
        command.args(["-r", &format!("{frame_rate:.3}")]);
    }
    command.arg(output_path.to_str().unwrap_or_default());

    let status = command.status().map_err(|error| {
        CommandError::blocked(
            "MEDIA_NORMALIZATION_FAILED",
            format!("failed to execute ffmpeg for video normalization: {error}"),
            "Install ffmpeg and ensure it is available on PATH",
        )
    })?;

    if !status.success() {
        return Err(CommandError::failure(
            "MEDIA_NORMALIZATION_FAILED",
            format!(
                "ffmpeg failed to normalize video `{}`",
                source_path.display()
            ),
            "Check that the source video is valid and supported by ffmpeg",
        ));
    }

    Ok(())
}
