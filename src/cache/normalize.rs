use std::{
    fs,
    path::Path,
    process::Command,
};

use crate::error::{FluffyError, Result};
use super::probe::VideoStreamInfo;

pub const DEFAULT_FPS: u32 = 30;
pub const DEFAULT_CRF: u32 = 22;
pub const DEFAULT_PRESET: &str = "medium";

/// Normalizes dimensions so both width and height are even integers (divisible by 2).
/// H.264 yuv420p strictly requires even dimensions.
pub fn normalize_even_dimensions(width: u32, height: u32) -> (u32, u32) {
    let even_w = width & !1;
    let even_h = height & !1;
    (even_w.max(2), even_h.max(2))
}

/// Transcodes an input video into the standard playback profile:
/// - MP4 container
/// - H.264 video codec (libx264)
/// - yuv420p pixel format
/// - 30 fps
/// - No audio, no subtitles
/// - Even dimensions
pub fn transcode_video<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
    probe_info: &VideoStreamInfo,
) -> Result<()> {
    let input = input_path.as_ref();
    let output = output_path.as_ref();

    let (target_w, target_h) = normalize_even_dimensions(probe_info.width, probe_info.height);

    let scale_filter = format!("scale={}:{}", target_w, target_h);

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",               // Overwrite output file
        "-v", "error",      // Suppress normal banners
        "-i",
    ])
    .arg(input)
    .args([
        "-an",              // Drop audio
        "-sn",              // Drop subtitles
        "-c:v", "libx264",  // H.264 video codec
        "-pix_fmt", "yuv420p",
        "-r", &DEFAULT_FPS.to_string(),
        "-vf", &scale_filter,
        "-crf", &DEFAULT_CRF.to_string(),
        "-preset", DEFAULT_PRESET,
        "-movflags", "+faststart",
    ])
    .arg(output);

    println!(
        "[Normalize] Transcoding {:?} -> {:?} (target: {}x{}, {} fps, CRF {})",
        input, output, target_w, target_h, DEFAULT_FPS, DEFAULT_CRF
    );

    let output_res = cmd.output().map_err(|e| {
        // Clean up partial output on execution failure
        let _ = fs::remove_file(output);
        FluffyError::Conversion(format!("Failed to execute ffmpeg: {e}"))
    })?;

    if !output_res.status.success() {
        // Ensure failed partial conversion is removed
        let _ = fs::remove_file(output);
        let stderr = String::from_utf8_lossy(&output_res.stderr);
        return Err(FluffyError::Conversion(format!(
            "ffmpeg transcoding failed: {}",
            stderr.trim()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_odd_dimension_normalization() {
        assert_eq!(normalize_even_dimensions(1920, 1080), (1920, 1080));
        assert_eq!(normalize_even_dimensions(1921, 1080), (1920, 1080));
        assert_eq!(normalize_even_dimensions(1920, 1081), (1920, 1080));
        assert_eq!(normalize_even_dimensions(1921, 1081), (1920, 1080));
        assert_eq!(normalize_even_dimensions(3839, 2159), (3838, 2158));
        // Boundary case min 2
        assert_eq!(normalize_even_dimensions(1, 1), (2, 2));
    }
}
