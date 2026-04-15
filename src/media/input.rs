use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, RANGE};
use serde::Deserialize;

use crate::error::CommandError;
use crate::media::MediaKind;

#[derive(Debug, Clone)]
pub enum MediaReference {
    Local(LocalMediaReference),
    Remote(RemoteMediaReference),
}

#[derive(Debug, Clone)]
pub struct LocalMediaReference {
    pub raw: String,
    pub path: PathBuf,
    pub kind: MediaKind,
    pub file_name: String,
    pub probe: MediaProbe,
}

#[derive(Debug, Clone)]
pub struct RemoteMediaReference {
    pub raw: String,
    pub kind: MediaKind,
    pub file_name: String,
    pub content_type: String,
}

#[derive(Debug, Clone)]
pub struct MediaProbe {
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<f64>,
}

pub fn parse_media_references(values: &[String]) -> Result<Vec<MediaReference>, CommandError> {
    values
        .iter()
        .map(|value| parse_media_reference(value))
        .collect()
}

fn parse_media_reference(value: &str) -> Result<MediaReference, CommandError> {
    if let Ok(url) = Url::parse(value) {
        let scheme = url.scheme();
        if scheme == "http" || scheme == "https" {
            return parse_remote_reference(value, &url);
        }
    }

    parse_local_reference(value)
}

fn parse_local_reference(value: &str) -> Result<MediaReference, CommandError> {
    let path = PathBuf::from(value);
    if !path.exists() || !path.is_file() {
        return Err(CommandError::failure(
            "MEDIA_INPUT_INVALID",
            format!("local media path `{value}` was not found"),
            "Pass an existing local file path or a public http(s) URL",
        ));
    }

    let kind = detect_kind_from_path(&path)?;
    let probe = probe_media(&path, kind)?;
    let file_name = path
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("asset")
        .to_owned();

    Ok(MediaReference::Local(LocalMediaReference {
        raw: value.to_owned(),
        path,
        kind,
        file_name,
        probe,
    }))
}

fn parse_remote_reference(value: &str, url: &Url) -> Result<MediaReference, CommandError> {
    let file_name = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .unwrap_or("asset");
    let (kind, content_type) = detect_remote_kind(file_name, value)?;

    Ok(MediaReference::Remote(RemoteMediaReference {
        raw: value.to_owned(),
        kind,
        file_name: file_name.to_owned(),
        content_type,
    }))
}

pub fn detect_kind_from_path(path: &Path) -> Result<MediaKind, CommandError> {
    let file_name = path
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("asset");
    detect_kind_and_content_type_from_name(file_name).map(|(kind, _)| kind)
}

pub fn detect_kind_and_content_type_from_name(
    file_name: &str,
) -> Result<(MediaKind, &'static str), CommandError> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" => Ok((MediaKind::Image, "image/jpeg")),
        "png" => Ok((MediaKind::Image, "image/png")),
        "webp" => Ok((MediaKind::Image, "image/webp")),
        "gif" => Ok((MediaKind::Image, "image/gif")),
        "heic" | "heif" => Ok((MediaKind::Image, "image/heic")),
        "avif" => Ok((MediaKind::Image, "image/avif")),
        "bmp" => Ok((MediaKind::Image, "image/bmp")),
        "tif" | "tiff" => Ok((MediaKind::Image, "image/tiff")),
        "mp4" => Ok((MediaKind::Video, "video/mp4")),
        "mov" => Ok((MediaKind::Video, "video/quicktime")),
        "m4v" => Ok((MediaKind::Video, "video/x-m4v")),
        "webm" => Ok((MediaKind::Video, "video/webm")),
        "mkv" => Ok((MediaKind::Video, "video/x-matroska")),
        "avi" => Ok((MediaKind::Video, "video/x-msvideo")),
        "mpeg" | "mpg" => Ok((MediaKind::Video, "video/mpeg")),
        _ => Err(CommandError::failure(
            "MEDIA_TYPE_UNSUPPORTED",
            format!("unsupported media extension for `{file_name}`"),
            "Use a supported image or video file with an explicit file extension",
        )),
    }
}

fn detect_remote_kind(file_name: &str, url: &str) -> Result<(MediaKind, String), CommandError> {
    if let Ok((kind, content_type)) = detect_kind_and_content_type_from_name(file_name) {
        return Ok((kind, content_type.to_owned()));
    }

    detect_remote_kind_from_http(url)
}

fn detect_remote_kind_from_http(url: &str) -> Result<(MediaKind, String), CommandError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| {
            CommandError::failure(
                "MEDIA_TYPE_UNSUPPORTED",
                format!("failed to initialize HTTP client for remote media detection: {error}"),
                "Retry with a URL that includes an explicit file extension",
            )
        })?;

    if let Some(result) = probe_remote_content_type(&client, url)? {
        return Ok(result);
    }

    Err(CommandError::failure(
        "MEDIA_TYPE_UNSUPPORTED",
        format!(
            "unsupported remote media URL `{url}`: no usable file extension and no detectable image/video Content-Type"
        ),
        "Use a URL with an explicit file extension or a server that returns a standard image/* or video/* Content-Type",
    ))
}

fn probe_remote_content_type(
    client: &Client,
    url: &str,
) -> Result<Option<(MediaKind, String)>, CommandError> {
    let head_response = client.head(url).send().map_err(|error| {
        CommandError::failure(
            "MEDIA_TYPE_UNSUPPORTED",
            format!("failed to inspect remote media URL `{url}`: {error}"),
            "Use a reachable public URL or add an explicit file extension",
        )
    })?;

    if head_response.status().is_success()
        && let Some(content_type) = media_content_type_from_headers(head_response.headers())
    {
        return Ok(Some(content_type));
    }

    let get_response = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send()
        .map_err(|error| {
            CommandError::failure(
                "MEDIA_TYPE_UNSUPPORTED",
                format!("failed to inspect remote media URL `{url}`: {error}"),
                "Use a reachable public URL or add an explicit file extension",
            )
        })?;

    if (get_response.status().is_success() || get_response.status().as_u16() == 206)
        && let Some(content_type) = media_content_type_from_headers(get_response.headers())
    {
        return Ok(Some(content_type));
    }

    Ok(None)
}

fn media_content_type_from_headers(
    headers: &reqwest::header::HeaderMap,
) -> Option<(MediaKind, String)> {
    let content_type = headers.get(CONTENT_TYPE)?.to_str().ok()?;
    let normalized = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase();

    if normalized.starts_with("image/") {
        return Some((MediaKind::Image, normalized));
    }
    if normalized.starts_with("video/") {
        return Some((MediaKind::Video, normalized));
    }

    None
}

fn probe_media(path: &Path, kind: MediaKind) -> Result<MediaProbe, CommandError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|error| {
            CommandError::blocked(
                "MEDIA_PROBE_UNAVAILABLE",
                format!("failed to execute ffprobe: {error}"),
                "Install ffprobe and ensure it is available on PATH",
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(CommandError::failure(
            "MEDIA_PROBE_FAILED",
            format!("ffprobe could not inspect `{}`: {stderr}", path.display()),
            "Check that the media file is valid and supported by ffprobe",
        ));
    }

    let parsed: FfprobeResponse = serde_json::from_slice(&output.stdout).map_err(|error| {
        CommandError::failure(
            "MEDIA_PROBE_FAILED",
            format!(
                "ffprobe returned invalid JSON for `{}`: {error}",
                path.display()
            ),
            "Check that ffprobe is working correctly and retry",
        )
    })?;

    let stream = parsed.streams.into_iter().next().ok_or_else(|| {
        CommandError::failure(
            "MEDIA_PROBE_FAILED",
            format!(
                "ffprobe did not return a video stream for `{}`",
                path.display()
            ),
            "Check that the media file is valid and supported by ffprobe",
        )
    })?;

    let width = stream.width.ok_or_else(|| {
        CommandError::failure(
            "MEDIA_PROBE_FAILED",
            format!(
                "ffprobe did not return width information for `{}`",
                path.display()
            ),
            "Check that the media file is valid and supported by ffprobe",
        )
    })?;
    let height = stream.height.ok_or_else(|| {
        CommandError::failure(
            "MEDIA_PROBE_FAILED",
            format!(
                "ffprobe did not return height information for `{}`",
                path.display()
            ),
            "Check that the media file is valid and supported by ffprobe",
        )
    })?;

    let frame_rate = match kind {
        MediaKind::Image => None,
        MediaKind::Video => stream
            .avg_frame_rate
            .as_deref()
            .and_then(parse_ffprobe_frame_rate),
    };

    Ok(MediaProbe {
        width,
        height,
        frame_rate,
    })
}

fn parse_ffprobe_frame_rate(raw: &str) -> Option<f64> {
    let (left, right) = raw.split_once('/')?;
    let numerator = left.parse::<f64>().ok()?;
    let denominator = right.parse::<f64>().ok()?;
    if denominator <= 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

#[derive(Debug, Deserialize)]
struct FfprobeResponse {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::media::MediaKind;

    use super::detect_kind_and_content_type_from_name;

    #[test]
    fn detects_remote_image_kind_from_extension() {
        let (kind, content_type) =
            detect_kind_and_content_type_from_name("asset.JPG").expect("kind");
        assert_eq!(kind, MediaKind::Image);
        assert_eq!(content_type, "image/jpeg");
    }

    #[test]
    fn detects_remote_video_kind_from_extension() {
        let (kind, content_type) =
            detect_kind_and_content_type_from_name("clip.mp4").expect("kind");
        assert_eq!(kind, MediaKind::Video);
        assert_eq!(content_type, "video/mp4");
    }
}
