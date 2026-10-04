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
    transcode_video_with_crossfade(input_path, output_path, probe_info, cancel_token, None)
}

/// Transcodes an input video with optional cancellation token and seamless loop crossfade.
/// When `crossfade_secs` is provided and the video duration is sufficient, the video's tail
/// is blended into the head using ffmpeg's xfade filter, producing a seamlessly looping video.
pub fn transcode_video_with_crossfade<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
    probe_info: &VideoStreamInfo,
    cancel_token: Option<&std::sync::atomic::AtomicBool>,
    crossfade_secs: Option<f64>,
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
    .arg(input);

    let use_crossfade = if let Some(fade_sec) = crossfade_secs {
        if let Some(duration) = probe_info.duration_secs {
            duration > fade_sec * 2.0 && fade_sec >= 0.05
        } else {
            false
        }
    } else {
        false
    };

    if use_crossfade {
        let fade_sec = crossfade_secs.unwrap();
        let duration = probe_info.duration_secs.unwrap();
        let effective_fade = fade_sec.min(duration / 3.0);
        let split_time = duration - effective_fade;

        let filter_complex = format!(
            "[0:v]split=2[v_base][v_tail];\
             [v_tail]trim=start={split_time:.3}:end={duration:.3},setpts=PTS-STARTPTS[part_tail];\
             [v_base]trim=start=0:end={split_time:.3},setpts=PTS-STARTPTS[part_main];\
             [part_tail][part_main]xfade=transition=fade:duration={effective_fade:.3}:offset=0,scale={target_w}:{target_h}[outv]"
        );

        tracing::info!(
            operation = "ffmpeg_xfade_loop",
            duration = duration,
            fade_sec = effective_fade,
            "[Normalize] Applying seamless loop crossfade filter via ffmpeg xfade"
        );

        cmd.args(["-filter_complex", &filter_complex, "-map", "[outv]"]);
    } else {
        cmd.args(["-vf", &scale_filter]);
    }

    cmd.args([
        "-an", // Drop audio
        "-sn", // Drop subtitles
        "-c:v",
        "libx264", // H.264 video codec
        "-pix_fmt",
        "yuv420p",
        "-r",
        &DEFAULT_FPS.to_string(),
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

    #[test]
    fn test_transcode_video_with_seamless_crossfade() {
        let temp_dir = std::env::temp_dir();
        let pid = std::process::id();
        let in_file = temp_dir.join(format!("fluffy_test_xfade_in_{pid}.mp4"));
        let out_file = temp_dir.join(format!("fluffy_test_xfade_out_{pid}.mp4"));

        // Create 4-second test video
        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=4:size=320x240:rate=30",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&in_file)
            .status();

        let Ok(status) = gen_status else {
            return; // ffmpeg not found in environment, skip gracefully
        };
        if !status.success() {
            let _ = fs::remove_file(&in_file);
            return;
        }

        let probe_info = match crate::cache::probe::probe_video(&in_file) {
            Ok(info) => info,
            Err(_) => {
                let _ = fs::remove_file(&in_file);
                return;
            }
        };

        // Transcode with 1.0s crossfade
        let res = transcode_video_with_crossfade(&in_file, &out_file, &probe_info, None, Some(1.0));
        assert!(res.is_ok(), "Transcode with crossfade failed: {:?}", res);
        assert!(out_file.exists());
        assert!(fs::metadata(&out_file).unwrap().len() > 0);

        // Verify output duration is trimmed by ~1.0s (4.0s - 1.0s = 3.0s)
        if let Ok(out_probe) = crate::cache::probe::probe_video(&out_file)
            && let Some(dur) = out_probe.duration_secs
        {
            assert!(
                (2.8..=3.2).contains(&dur),
                "Expected duration ~3.0s, got {dur}"
            );
        }

        let _ = fs::remove_file(&in_file);
        let _ = fs::remove_file(&out_file);
    }
}
