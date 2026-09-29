use serde::Deserialize;
use std::{path::Path, process::Command};

use crate::error::{FluffyError, Result};

pub const MAX_WIDTH: u32 = 3840;
pub const MAX_HEIGHT: u32 = 2160;

#[derive(Debug, Clone, PartialEq)]
pub struct VideoStreamInfo {
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub pix_fmt: Option<String>,
    pub fps: f64,
    pub duration_secs: Option<f64>,
    pub has_audio: bool,
    pub container_format: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    streams: Option<Vec<FfprobeStream>>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    r_frame_rate: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

pub fn probe_video<P: AsRef<Path>>(path: P) -> Result<VideoStreamInfo> {
    let path = path.as_ref();
    if !path.exists() {
        tracing::warn!(operation = "ffprobe", path = ?path, "[Probe] Video file does not exist");
        return Err(FluffyError::Probe(format!(
            "File does not exist: {:?}",
            path
        )));
    }

    tracing::debug!(operation = "ffprobe", path = ?path, "[Probe] Probing video file with ffprobe");

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
        .map_err(|e| {
            tracing::error!(operation = "ffprobe", error = %e, path = ?path, "[Probe] Failed to execute ffprobe process");
            FluffyError::Probe(format!("Failed to execute ffprobe: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            operation = "ffprobe",
            path = ?path,
            stderr = %stderr.trim(),
            "[Probe] ffprobe exited with error"
        );
        return Err(FluffyError::Probe(format!(
            "ffprobe exited with error: {}",
            stderr.trim()
        )));
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| {
            tracing::warn!(operation = "ffprobe_parse", path = ?path, error = %e, "[Probe] Failed to parse ffprobe json output");
            FluffyError::Probe(format!("Failed to parse ffprobe json: {e}"))
        })?;

    let streams = parsed
        .streams
        .ok_or_else(|| FluffyError::Probe("No streams found in media file".to_string()))?;

    let video_stream = streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .ok_or_else(|| FluffyError::Probe("No video stream found in media file".to_string()))?;

    let width = video_stream
        .width
        .ok_or_else(|| FluffyError::Probe("Missing video width".to_string()))?;
    let height = video_stream
        .height
        .ok_or_else(|| FluffyError::Probe("Missing video height".to_string()))?;

    let codec = video_stream
        .codec_name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let pix_fmt = video_stream.pix_fmt.clone();

    let fps = parse_frame_rate(video_stream.r_frame_rate.as_deref().unwrap_or("30/1"));

    let duration_secs = video_stream
        .duration
        .as_deref()
        .or_else(|| parsed.format.as_ref().and_then(|f| f.duration.as_deref()))
        .and_then(|s| s.parse::<f64>().ok());

    let has_audio = streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("audio"));

    let container_format = parsed.format.as_ref().and_then(|f| f.format_name.clone());

    let info = VideoStreamInfo {
        width,
        height,
        codec,
        pix_fmt,
        fps,
        duration_secs,
        has_audio,
        container_format,
    };

    validate_video_dimensions(info.width, info.height)?;

    tracing::debug!(
        operation = "probe_success",
        codec = %info.codec,
        container = ?info.container_format,
        width = info.width,
        height = info.height,
        fps = info.fps,
        has_audio = info.has_audio,
        "[Probe] Probed video stream info successfully"
    );

    Ok(info)
}

/// Validates that video dimensions satisfy the 4K boundary policy:
/// `width <= 3840 && height <= 2160`
pub fn validate_video_dimensions(width: u32, height: u32) -> Result<()> {
    if width > MAX_WIDTH || height > MAX_HEIGHT {
        return Err(FluffyError::Probe(format!(
            "Video dimensions {width}x{height} exceed maximum allowed 4K boundary ({MAX_WIDTH}x{MAX_HEIGHT})"
        )));
    }
    if width == 0 || height == 0 {
        return Err(FluffyError::Probe(format!(
            "Invalid video dimensions: {width}x{height}"
        )));
    }
    Ok(())
}

fn parse_frame_rate(fps_str: &str) -> f64 {
    if let Some((num_str, den_str)) = fps_str.split_once('/') {
        let num: f64 = num_str.parse().unwrap_or(30.0);
        let den: f64 = den_str.parse().unwrap_or(1.0);
        if den > 0.0 {
            return num / den;
        }
    }
    fps_str.parse().unwrap_or(30.0)
}

impl VideoStreamInfo {
    /// Checks whether the container format matches the MP4 / QuickTime profile family ("mp4", "mov", "m4a").
    pub fn is_compatible_container(&self) -> bool {
        match &self.container_format {
            Some(fmt) => fmt.split(',').any(|part| {
                let p = part.trim();
                p == "mp4" || p == "mov" || p == "m4a"
            }),
            None => false,
        }
    }

    /// Checks whether the video is already fully compliant with the playback profile:
    /// - Container Format: MP4 / QuickTime ("mp4", "mov", "m4a")
    /// - Video Codec: H.264 ("h264" or "avc1")
    /// - Pixel Format: "yuv420p"
    /// - Even Dimensions: width and height are divisible by 2
    /// - Dimensions within 4K boundary: width <= 3840 && height <= 2160
    /// - Frame Rate: <= 30.5 fps (supports 23.976, 24, 25, 29.97, 30 fps)
    pub fn is_compatible_profile(&self) -> bool {
        let container_ok = self.is_compatible_container();
        let codec_ok = self.codec == "h264" || self.codec == "avc1";
        let pix_fmt_ok = self.pix_fmt.as_deref() == Some("yuv420p");
        let dims_even = self.width.is_multiple_of(2) && self.height.is_multiple_of(2);
        let dims_ok = self.width <= MAX_WIDTH
            && self.height <= MAX_HEIGHT
            && self.width > 0
            && self.height > 0;
        let fps_ok = self.fps <= 30.5;

        container_ok && codec_ok && pix_fmt_ok && dims_even && dims_ok && fps_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_4k_boundary_validation() {
        // Accepted dimensions
        assert!(validate_video_dimensions(3840, 2160).is_ok());
        assert!(validate_video_dimensions(3840, 1600).is_ok());
        assert!(validate_video_dimensions(2560, 1440).is_ok());
        assert!(validate_video_dimensions(1920, 1080).is_ok());

        // Oversized dimensions rejected before encoding
        assert!(validate_video_dimensions(5120, 1440).is_err());
        assert!(validate_video_dimensions(7680, 2160).is_err());
        assert!(validate_video_dimensions(3841, 2160).is_err());
        assert!(validate_video_dimensions(3840, 2161).is_err());

        // Zero dimensions rejected
        assert!(validate_video_dimensions(0, 1080).is_err());
        assert!(validate_video_dimensions(1920, 0).is_err());
    }

    #[test]
    fn test_parse_frame_rate() {
        assert!((parse_frame_rate("30/1") - 30.0).abs() < 1e-4);
        assert!((parse_frame_rate("24/1") - 24.0).abs() < 1e-4);
        assert!((parse_frame_rate("30000/1001") - 29.97).abs() < 1e-2);
        assert!((parse_frame_rate("60/1") - 60.0).abs() < 1e-4);
    }

    #[test]
    fn test_is_compatible_profile() {
        let valid_mp4 = VideoStreamInfo {
            width: 1920,
            height: 1080,
            codec: "h264".to_string(),
            pix_fmt: Some("yuv420p".to_string()),
            fps: 30.0,
            duration_secs: Some(10.0),
            has_audio: false,
            container_format: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
        };
        // MP4 / H.264 / yuv420p -> compatible
        assert!(valid_mp4.is_compatible_profile());

        // MKV / H.264 / yuv420p -> incompatible (must be transcoded to MP4)
        let mut mkv_h264 = valid_mp4.clone();
        mkv_h264.container_format = Some("matroska,webm".to_string());
        assert!(!mkv_h264.is_compatible_profile());

        // WebM / VP9 -> incompatible
        let mut webm_vp9 = valid_mp4.clone();
        webm_vp9.container_format = Some("matroska,webm".to_string());
        webm_vp9.codec = "vp9".to_string();
        assert!(!webm_vp9.is_compatible_profile());

        // MP4 / H.265 (HEVC) -> incompatible
        let mut mp4_hevc = valid_mp4.clone();
        mp4_hevc.codec = "hevc".to_string();
        assert!(!mp4_hevc.is_compatible_profile());

        // MP4 / H.264 / yuv444p -> incompatible
        let mut mp4_yuv444p = valid_mp4.clone();
        mp4_yuv444p.pix_fmt = Some("yuv444p".to_string());
        assert!(!mp4_yuv444p.is_compatible_profile());

        // MP4 / H.264 / out-of-range FPS (60fps) -> incompatible
        let mut mp4_high_fps = valid_mp4.clone();
        mp4_high_fps.fps = 60.0;
        assert!(!mp4_high_fps.is_compatible_profile());

        // Missing container format -> incompatible
        let mut no_container = valid_mp4.clone();
        no_container.container_format = None;
        assert!(!no_container.is_compatible_profile());

        // Odd dimension -> incompatible
        let mut odd = valid_mp4.clone();
        odd.width = 1921;
        assert!(!odd.is_compatible_profile());
    }

    #[test]
    fn test_probe_nonexistent_file_fails() {
        let nonexistent = Path::new("/tmp/fluffy_definitely_nonexistent_video.mp4");
        let res = probe_video(nonexistent);
        assert!(res.is_err());
        match res {
            Err(FluffyError::Probe(msg)) => {
                assert!(msg.contains("does not exist"));
            }
            other => panic!("Expected FluffyError::Probe, got {other:?}"),
        }
    }

    #[test]
    fn test_probe_corrupted_file_fails() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_corrupted_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let corrupt_path = temp_dir.join("corrupted.mp4");
        std::fs::write(&corrupt_path, b"not a real video file content").unwrap();

        let res = probe_video(&corrupt_path);
        assert!(res.is_err());
        match res {
            Err(FluffyError::Probe(msg)) => {
                assert!(
                    msg.contains("ffprobe") || msg.contains("No streams") || msg.contains("json")
                );
            }
            other => panic!("Expected FluffyError::Probe, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
