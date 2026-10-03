use std::{fs, path::Path, process::Command};

use super::probe::VideoStreamInfo;
use crate::error::{FluffyError, Result};

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
    transcode_video_with_cancel(input_path, output_path, probe_info, None)
}

/// Transcodes an input video with an optional cancellation token.
/// If `cancel_token` is set to true during processing, the `ffmpeg` subprocess is
/// immediately killed, temporary files are removed, and `FluffyError::JobCancelled` is returned.
pub fn transcode_video_with_cancel<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
    probe_info: &VideoStreamInfo,
    cancel_token: Option<&std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    let input = input_path.as_ref();
    let output = output_path.as_ref();

    let (target_w, target_h) = normalize_even_dimensions(probe_info.width, probe_info.height);
    let scale_filter = format!("scale={}:{}", target_w, target_h);

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y", // Overwrite output file
        "-v", "error", // Suppress normal banners
        "-i",
    ])
    .arg(input)
    .args([
        "-an", // Drop audio
        "-sn", // Drop subtitles
        "-c:v",
        "libx264", // H.264 video codec
        "-pix_fmt",
        "yuv420p",
        "-r",
        &DEFAULT_FPS.to_string(),
        "-vf",
        &scale_filter,
        "-crf",
        &DEFAULT_CRF.to_string(),
        "-preset",
        DEFAULT_PRESET,
        "-movflags",
        "+faststart",
    ])
    .arg(output);

    tracing::info!(
        operation = "ffmpeg_transcode",
        input = ?input,
        output = ?output,
        width = target_w,
        height = target_h,
        fps = DEFAULT_FPS,
        crf = DEFAULT_CRF,
        "[Normalize] Transcoding video to standardized storage format"
    );

    let cleanup_file = |path: &Path| {
        if let Err(cleanup_err) = fs::remove_file(path)
            && cleanup_err.kind() != std::io::ErrorKind::NotFound
        {
            tracing::debug!(
                operation = "cleanup",
                error = %cleanup_err,
                output = ?path,
                "[Normalize] Failed to remove partial file"
            );
        }
    };

    let mut child = cmd.spawn().map_err(|e| {
        cleanup_file(output);
        tracing::error!(operation = "ffmpeg_exec", error = %e, "[Normalize] Failed to execute ffmpeg");
        FluffyError::Conversion(format!("Failed to execute ffmpeg: {e}"))
    })?;

    // Monitor ffmpeg subprocess execution with cancellation polling
    loop {
        if let Some(token) = cancel_token
            && token.load(Ordering::Relaxed)
        {
            tracing::info!(
                operation = "ffmpeg_cancel",
                input = ?input,
                output = ?output,
                "[Normalize] Transcode job was cancelled; killing ffmpeg subprocess"
            );
            let _ = child.kill();
            let _ = child.wait();
            cleanup_file(output);
            return Err(FluffyError::JobCancelled);
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    cleanup_file(output);
                    tracing::error!(
                        operation = "ffmpeg_transcode",
                        status = ?status,
                        "[Normalize] ffmpeg transcoding failed with non-zero exit status"
                    );
                    return Err(FluffyError::Conversion(format!(
                        "ffmpeg transcoding failed with exit status: {status}"
                    )));
                }
                break;
            }
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                cleanup_file(output);
                tracing::error!(operation = "ffmpeg_wait", error = %e, "[Normalize] Error waiting for ffmpeg");
                return Err(FluffyError::Conversion(format!(
                    "Error waiting for ffmpeg: {e}"
                )));
            }
        }
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
