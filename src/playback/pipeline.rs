use std::path::Path;

use gstreamer::glib::translate::FromGlibPtrFull;
use gstreamer::prelude::*;
use gstreamer_video::prelude::*;

use crate::error::{FluffyError, Result};

#[link(name = "gstwayland-1.0")]
unsafe extern "C" {
    fn gst_wl_display_handle_context_new(
        display: *mut std::ffi::c_void,
    ) -> *mut gstreamer::ffi::GstContext;
}

pub struct PipelineHandle {
    pub pipeline: gstreamer::Pipeline,
    pub sink: gstreamer::Element,
    pub overlay: gstreamer_video::VideoOverlay,
    pub bus: gstreamer::Bus,
    pub gst_wl_context: gstreamer::Context,
    pub output_name: String,
    pub generation: u64,
    pub video_id: String,
    pub session_id: String,
}

impl PipelineHandle {
    /// # Safety
    /// `raw_display_ptr` must be a valid pointer to a `wl_display`.
    pub unsafe fn new(
        output_name: impl Into<String>,
        generation: u64,
        raw_display_ptr: *mut std::ffi::c_void,
        raw_surface_ptr: usize,
        width: u32,
        height: u32,
        video_path: &Path,
    ) -> Result<Self> {
        let abs_path = video_path.canonicalize()?;
        let uri = format!("file://{}", abs_path.display());

        // 1. Create Wayland display handle context
        let gst_wl_context: gstreamer::Context = unsafe {
            let raw = gst_wl_display_handle_context_new(raw_display_ptr);
            FromGlibPtrFull::from_glib_full(raw)
        };

        // 2. Create dedicated waylandsink and apply context immediately
        let sink = gstreamer::ElementFactory::make("waylandsink")
            .name("fluffy-waylandsink")
            .build()
            .map_err(|e| FluffyError::Playback(format!("Failed to create waylandsink: {e}")))?;
        sink.set_context(&gst_wl_context);

        let overlay = sink
            .clone()
            .dynamic_cast::<gstreamer_video::VideoOverlay>()
            .map_err(|_| {
                FluffyError::Playback("waylandsink does not implement VideoOverlay".into())
            })?;

        // Pre-configure native window handle & render rectangle
        unsafe {
            overlay.set_window_handle(raw_surface_ptr);
        }
        if let Err(e) = overlay.set_render_rectangle(0, 0, width as i32, height as i32) {
            tracing::trace!(operation = "render_rectangle", error = ?e, "[Pipeline] Initial overlay set_render_rectangle ignored (benign on some sinks)");
        }

        // 3. Create playbin with pre-configured sink and silent audio sink
        let audio_sink = gstreamer::ElementFactory::make("fakesink")
            .name("fluffy-audio-fakesink")
            .build()
            .ok();

        let mut playbin_builder = gstreamer::ElementFactory::make("playbin")
            .property("uri", uri)
            .property("video-sink", &sink)
            .property("volume", 0.0f64);

        if let Some(ref asink) = audio_sink {
            playbin_builder = playbin_builder.property("audio-sink", asink);
        }

        let playbin = playbin_builder
            .build()
            .map_err(|e| FluffyError::Playback(format!("Failed to create playbin: {e}")))?;

        let pipeline = playbin
            .dynamic_cast::<gstreamer::Pipeline>()
            .map_err(|_| FluffyError::Playback("playbin is not a Pipeline".into()))?;

        pipeline.set_context(&gst_wl_context);

        let bus = pipeline
            .bus()
            .ok_or_else(|| FluffyError::Playback("Failed to get pipeline bus".into()))?;

        tracing::info!(
            operation = "pipeline_create",
            video = ?video_path,
            width,
            height,
            "[Pipeline] Created GStreamer playbin pipeline with waylandsink"
        );

        let output_name = output_name.into();
        let video_id = crate::benchmark::safe_video_id(video_path);
        let session_id = crate::benchmark::session_id().to_string();

        Ok(Self {
            pipeline,
            sink,
            overlay,
            bus,
            gst_wl_context,
            output_name,
            generation,
            video_id,
            session_id,
        })
    }
}

impl Drop for PipelineHandle {
    fn drop(&mut self) {
        tracing::info!(
            event = "old_pipeline_handle_dropped",
            output = %self.output_name,
            generation = self.generation,
            video_id = %self.video_id,
            session_id = %self.session_id,
            "[Pipeline] Old pipeline handle dropped in Rust (Rust object dropped; GStreamer resources may still be releasing asynchronously)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation_nonexistent_file_fails() {
        let _ = gstreamer::init();
        let nonexistent = Path::new("/tmp/fluffy_nonexistent_pipeline_video.mp4");
        let res = unsafe {
            PipelineHandle::new(
                "default",
                1,
                std::ptr::null_mut(),
                0,
                1920,
                1080,
                nonexistent,
            )
        };
        assert!(res.is_err());
    }
}
