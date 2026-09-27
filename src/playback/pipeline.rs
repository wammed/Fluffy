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
}

impl PipelineHandle {
    /// # Safety
    /// `raw_display_ptr` must be a valid pointer to a `wl_display`.
    pub unsafe fn new(
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
            .map_err(|_| FluffyError::Playback("waylandsink does not implement VideoOverlay".into()))?;

        // Pre-configure native window handle & render rectangle
        unsafe {
            overlay.set_window_handle(raw_surface_ptr);
        }
        let _ = overlay.set_render_rectangle(0, 0, width as i32, height as i32);

        // 3. Create playbin with pre-configured sink
        let playbin = gstreamer::ElementFactory::make("playbin")
            .property("uri", uri)
            .property("video-sink", &sink)
            .build()
            .map_err(|e| FluffyError::Playback(format!("Failed to create playbin: {e}")))?;

        let pipeline = playbin
            .dynamic_cast::<gstreamer::Pipeline>()
            .map_err(|_| FluffyError::Playback("playbin is not a Pipeline".into()))?;

        pipeline.set_context(&gst_wl_context);

        let bus = pipeline
            .bus()
            .ok_or_else(|| FluffyError::Playback("Failed to get pipeline bus".into()))?;

        Ok(Self {
            pipeline,
            sink,
            overlay,
            bus,
            gst_wl_context,
        })
    }
}
