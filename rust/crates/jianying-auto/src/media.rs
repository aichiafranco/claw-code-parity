//! Probe local media with ffprobe when available.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

const VIDEO_EXTS: &[&str] = &["mp4", "mov", "mkv", "avi", "webm", "m4v", "flv"];
const AUDIO_EXTS: &[&str] = &["mp3", "wav", "aac", "m4a", "flac", "ogg"];
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
    Image,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaFile {
    pub path: PathBuf,
    pub name: String,
    pub kind: MediaKind,
    pub duration_us: u64,
    pub width: u32,
    pub height: u32,
}

impl MediaKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Image => "image",
        }
    }
}

impl Serialize for MediaKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug)]
pub enum MediaError {
    NotFound(PathBuf),
    Probe(String),
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "media path not found: {}", path.display()),
            Self::Probe(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for MediaError {}

#[must_use]
pub fn extension_kind(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if VIDEO_EXTS.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else if AUDIO_EXTS.contains(&ext.as_str()) {
        Some(MediaKind::Audio)
    } else if IMAGE_EXTS.contains(&ext.as_str()) {
        Some(MediaKind::Image)
    } else {
        None
    }
}

/// Collect video/image files from a file or directory (non-recursive).
pub fn collect_visuals(root: &Path) -> Result<Vec<PathBuf>, MediaError> {
    collect_by_kind(root, &[MediaKind::Video, MediaKind::Image])
}

pub fn collect_audios(root: &Path) -> Result<Vec<PathBuf>, MediaError> {
    collect_by_kind(root, &[MediaKind::Audio])
}

fn collect_by_kind(root: &Path, kinds: &[MediaKind]) -> Result<Vec<PathBuf>, MediaError> {
    if !root.exists() {
        return Err(MediaError::NotFound(root.to_path_buf()));
    }
    let mut files = Vec::new();
    if root.is_file() {
        if extension_kind(root).is_some_and(|kind| kinds.contains(&kind)) {
            files.push(root.to_path_buf());
        }
        return Ok(files);
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
        .map_err(|err| MediaError::Probe(format!("cannot read {}: {err}", root.display())))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| extension_kind(path).is_some_and(|kind| kinds.contains(&kind)))
        .collect();
    entries.sort();
    Ok(entries)
}

pub fn probe_file(path: &Path, default_image_us: u64) -> Result<MediaFile, MediaError> {
    let kind = extension_kind(path)
        .ok_or_else(|| MediaError::Probe(format!("unsupported media type: {}", path.display())))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("clip")
        .to_string();

    if kind == MediaKind::Image {
        let (width, height) = probe_streams(path).unwrap_or((1080, 1920));
        return Ok(MediaFile {
            path: path.to_path_buf(),
            name,
            kind,
            duration_us: default_image_us,
            width,
            height,
        });
    }

    if let Ok(probed) = probe_with_ffprobe(path, kind, &name) {
        return Ok(probed);
    }

    Err(MediaError::Probe(format!(
        "ffprobe failed for {} — install ffmpeg/ffprobe, or pass --assume-seconds",
        path.display()
    )))
}

pub fn probe_file_or_assume(
    path: &Path,
    default_image_us: u64,
    assume_seconds: Option<f64>,
) -> Result<MediaFile, MediaError> {
    match probe_file(path, default_image_us) {
        Ok(file) => Ok(file),
        Err(err) => {
            let Some(seconds) = assume_seconds else {
                return Err(err);
            };
            let kind = extension_kind(path).ok_or(err)?;
            Ok(MediaFile {
                path: path.to_path_buf(),
                name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("clip")
                    .to_string(),
                kind,
                duration_us: seconds_to_us(seconds),
                width: 1080,
                height: 1920,
            })
        }
    }
}

fn probe_with_ffprobe(path: &Path, kind: MediaKind, name: &str) -> Result<MediaFile, MediaError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|err| MediaError::Probe(format!("ffprobe not available: {err}")))?;
    if !output.status.success() {
        return Err(MediaError::Probe(
            "ffprobe returned a non-zero status".into(),
        ));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| MediaError::Probe(format!("ffprobe json: {err}")))?;
    let duration_s = json
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(json_number_or_str)
        .ok_or_else(|| MediaError::Probe("ffprobe missing duration".into()))?;
    let mut width = 1080;
    let mut height = 1920;
    if let Some(streams) = json.get("streams").and_then(serde_json::Value::as_array) {
        for stream in streams {
            if stream.get("codec_type").and_then(serde_json::Value::as_str) == Some("video") {
                width = stream
                    .get("width")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1080) as u32;
                height = stream
                    .get("height")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1920) as u32;
                break;
            }
        }
    }
    Ok(MediaFile {
        path: path.to_path_buf(),
        name: name.to_string(),
        kind,
        duration_us: seconds_to_us(duration_s),
        width,
        height,
    })
}

fn probe_streams(path: &Path) -> Option<(u32, u32)> {
    probe_with_ffprobe(path, MediaKind::Image, "img")
        .ok()
        .map(|file| (file.width, file.height))
}

fn json_number_or_str(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

#[must_use]
pub fn seconds_to_us(seconds: f64) -> u64 {
    (seconds * 1_000_000.0).round().max(0.0) as u64
}

#[must_use]
pub fn us_to_seconds(us: u64) -> f64 {
    us as f64 / 1_000_000.0
}
