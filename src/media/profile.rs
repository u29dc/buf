use crate::cli::InstagramPostType;
use crate::error::CommandError;
use crate::media::MediaKind;

#[derive(Debug, Clone)]
pub struct MediaProfile {
    pub name: &'static str,
    pub max_width: u32,
    pub max_height: u32,
    pub min_ratio: f64,
    pub max_ratio: f64,
    pub output_extension: &'static str,
    pub content_type: &'static str,
}

#[derive(Debug, Clone)]
pub struct ProfileDecision {
    pub profile: MediaProfile,
    pub effective_post_type: Option<String>,
}

pub fn resolve_profile(
    service: &str,
    requested_post_type: Option<InstagramPostType>,
    kind: MediaKind,
    media_count: usize,
) -> Result<ProfileDecision, CommandError> {
    match service {
        "instagram" => resolve_instagram_profile(requested_post_type, kind, media_count),
        "linkedin" => resolve_linkedin_profile(requested_post_type, kind, media_count),
        _ => Err(CommandError::failure(
            "MEDIA_SERVICE_UNSUPPORTED",
            format!("media preparation is not implemented for `{service}`"),
            "Use an Instagram or LinkedIn channel for posts with --media",
        )),
    }
}

pub fn validate_aspect_ratio(
    profile: &MediaProfile,
    width: u32,
    height: u32,
) -> Result<(), CommandError> {
    let ratio = f64::from(width) / f64::from(height);
    if ratio < profile.min_ratio || ratio > profile.max_ratio {
        return Err(CommandError::failure(
            "MEDIA_ASPECT_RATIO_UNSUPPORTED",
            format!(
                "source media ratio {:.3} is outside the supported range for profile `{}`",
                ratio, profile.name
            ),
            format!(
                "Use media closer to the supported range {:.3} to {:.3}",
                profile.min_ratio, profile.max_ratio
            ),
        ));
    }
    Ok(())
}

#[must_use]
pub fn fit_dimensions(
    source_width: u32,
    source_height: u32,
    max_width: u32,
    max_height: u32,
    even: bool,
) -> (u32, u32, bool) {
    let width_scale = f64::from(max_width) / f64::from(source_width);
    let height_scale = f64::from(max_height) / f64::from(source_height);
    let scale = width_scale.min(height_scale);
    let mut output_width = (f64::from(source_width) * scale).round() as u32;
    let mut output_height = (f64::from(source_height) * scale).round() as u32;

    output_width = output_width.max(1);
    output_height = output_height.max(1);

    if even {
        output_width = make_even(output_width);
        output_height = make_even(output_height);
    }

    (
        output_width.max(1),
        output_height.max(1),
        scale > 1.000_001_f64,
    )
}

fn make_even(value: u32) -> u32 {
    if value.is_multiple_of(2) {
        value
    } else {
        value.saturating_sub(1).max(2)
    }
}

fn resolve_instagram_profile(
    requested_post_type: Option<InstagramPostType>,
    kind: MediaKind,
    media_count: usize,
) -> Result<ProfileDecision, CommandError> {
    if kind == MediaKind::Video && media_count > 1 {
        return Err(CommandError::failure(
            "MEDIA_COMBINATION_UNSUPPORTED",
            "multiple videos are not supported in this prototype",
            "Use one video per post or use multiple images for a carousel",
        ));
    }

    let effective_post_type = match (requested_post_type, kind, media_count) {
        (Some(InstagramPostType::Story), _, count) | (Some(InstagramPostType::Reel), _, count)
            if count > 1 =>
        {
            return Err(CommandError::failure(
                "MEDIA_COMBINATION_UNSUPPORTED",
                "Instagram story and reel posts only support one asset in this prototype",
                "Use one asset or switch the post type to `carousel`",
            ));
        }
        (Some(InstagramPostType::Post), MediaKind::Image, count) if count > 1 => "carousel",
        (Some(kind), _, _) => kind.as_str(),
        (None, MediaKind::Image, count) if count > 1 => "carousel",
        (None, _, _) => "post",
    };

    let profile = match effective_post_type {
        "story" | "reel" => MediaProfile {
            name: "instagram-vertical",
            max_width: 2160,
            max_height: 3840,
            min_ratio: 0.5625,
            max_ratio: 0.8,
            output_extension: match kind {
                MediaKind::Image => "jpg",
                MediaKind::Video => "mp4",
            },
            content_type: match kind {
                MediaKind::Image => "image/jpeg",
                MediaKind::Video => "video/mp4",
            },
        },
        _ => MediaProfile {
            name: "instagram-feed",
            max_width: 2160,
            max_height: 2700,
            min_ratio: 0.8,
            max_ratio: 1.91,
            output_extension: match kind {
                MediaKind::Image => "jpg",
                MediaKind::Video => "mp4",
            },
            content_type: match kind {
                MediaKind::Image => "image/jpeg",
                MediaKind::Video => "video/mp4",
            },
        },
    };

    Ok(ProfileDecision {
        profile,
        effective_post_type: Some(effective_post_type.to_owned()),
    })
}

fn resolve_linkedin_profile(
    requested_post_type: Option<InstagramPostType>,
    kind: MediaKind,
    media_count: usize,
) -> Result<ProfileDecision, CommandError> {
    if media_count > 1 {
        return Err(CommandError::failure(
            "MEDIA_COMBINATION_UNSUPPORTED",
            "LinkedIn only supports one media asset in this prototype",
            "Use one image or one video for LinkedIn posts",
        ));
    }
    if requested_post_type.is_some_and(|post_type| post_type != InstagramPostType::Post) {
        return Err(CommandError::failure(
            "VALIDATION_ERROR",
            "LinkedIn only supports the default `post` type in this prototype",
            "Remove --type or leave it as `post`",
        ));
    }

    let profile = match kind {
        MediaKind::Image => MediaProfile {
            name: "linkedin-image",
            max_width: 2160,
            max_height: 2700,
            min_ratio: 0.333,
            max_ratio: 3.0,
            output_extension: "jpg",
            content_type: "image/jpeg",
        },
        MediaKind::Video => MediaProfile {
            name: "linkedin-video",
            max_width: 2304,
            max_height: 2304,
            min_ratio: 1.0 / 2.4,
            max_ratio: 2.4,
            output_extension: "mp4",
            content_type: "video/mp4",
        },
    };

    Ok(ProfileDecision {
        profile,
        effective_post_type: Some("post".to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use crate::cli::InstagramPostType;
    use crate::media::MediaKind;

    use super::{fit_dimensions, resolve_profile};

    #[test]
    fn multiple_images_infer_instagram_carousel() {
        let decision =
            resolve_profile("instagram", None, MediaKind::Image, 2).expect("profile decision");
        assert_eq!(decision.effective_post_type.as_deref(), Some("carousel"));
    }

    #[test]
    fn fit_dimensions_upscales_within_target_envelope() {
        let (width, height, upscaled) = fit_dimensions(1000, 1250, 2160, 2700, true);
        assert_eq!((width, height), (2160, 2700));
        assert!(upscaled);
    }

    #[test]
    fn linkedin_rejects_non_post_type() {
        let error = resolve_profile(
            "linkedin",
            Some(InstagramPostType::Story),
            MediaKind::Image,
            1,
        )
        .expect_err("should reject");
        assert_eq!(error.code(), "VALIDATION_ERROR");
    }
}
