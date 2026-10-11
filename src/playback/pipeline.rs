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
    fn gst_wl_window_get_wl_surface(window: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
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
    pub loop_count: std::sync::Arc<std::sync::atomic::AtomicU64>,
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
        let uri = gstreamer::glib::filename_to_uri(&abs_path, None)
            .map_err(|e| FluffyError::Playback(format!("Failed to build URI from path: {e}")))?;

        // 1. Create Wayland display handle context
        let raw = unsafe { gst_wl_display_handle_context_new(raw_display_ptr) };
        if raw.is_null() {
            return Err(FluffyError::Playback(
                "gst_wl_display_handle_context_new returned NULL pointer".to_string(),
            ));
        }
        let gst_wl_context: gstreamer::Context = unsafe { FromGlibPtrFull::from_glib_full(raw) };

        // 2. Create dedicated waylandsink and apply context immediately
        let sink = gstreamer::ElementFactory::make("waylandsink")
            .name("fluffy-waylandsink")
            .property("show-preroll-frame", false)
            .property("force-aspect-ratio", false)
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
            .property("uri", &uri)
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

        let loop_count = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        {
            let uri_clone = uri.clone();
            let loop_count_cb = loop_count.clone();
            let out_name_cb = output_name.clone();
            let gen_cb = generation;

            pipeline.connect("about-to-finish", false, move |values| {
                if let Ok(elem) = values[0].get::<gstreamer::Element>() {
                    elem.set_property("uri", &uri_clone);
                    let count = loop_count_cb.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    let sid = crate::benchmark::session_id();
                    tracing::debug!(
                        event = "playback_loop_gapless",
                        output = %out_name_cb,
                        generation = gen_cb,
                        loop_count = count,
                        session_id = %sid,
                        "[Pipeline] about-to-finish signal triggered: queued next URI for seamless loop"
                    );
                }
                None
            });
        }

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
            loop_count,
        })
    }

    /// # Safety
    /// Compatibility alias for `new`. Initial alpha is ignored as alpha is managed via Wayland GPU modifiers.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new_with_alpha(
        output_name: impl Into<String>,
        generation: u64,
        raw_display_ptr: *mut std::ffi::c_void,
        raw_surface_ptr: usize,
        width: u32,
        height: u32,
        video_path: &Path,
        _initial_alpha: Option<f64>,
    ) -> Result<Self> {
        unsafe {
            Self::new(
                output_name,
                generation,
                raw_display_ptr,
                raw_surface_ptr,
                width,
                height,
                video_path,
            )
        }
    }

    /// Retrieves the raw `wl_surface` pointer created by `waylandsink`'s internal window.
    /// Returns `None` if the sink has not initialized its window yet (e.g. before preroll).
    pub fn get_wayland_surface(&self) -> Option<*mut std::ffi::c_void> {
        self.get_wayland_surfaces().map(|(video, _)| video)
    }

    /// Retrieves both `video_surface` and `area_surface` pointers from `waylandsink`'s window.
    /// Fading both surfaces ensures background border letterboxes (if any) do not obscure
    /// the underlying old video during crossfade transition.
    pub fn get_wayland_surfaces(
        &self,
    ) -> Option<(*mut std::ffi::c_void, Option<*mut std::ffi::c_void>)> {
        let sink_ptr = self.sink.as_ptr();
        if sink_ptr.is_null() {
            return None;
        }
        unsafe {
            // Offset of `window` in `GstWaylandSink` struct (ext/wayland/gstwaylandsink.h)
            // On x86_64: sizeof(GstVideoSink)=712, sizeof(GMutex)=8 -> offset=728 (0x2d8)
            let window_ptr = *((sink_ptr as *const u8).add(728) as *const *mut std::ffi::c_void);
            if window_ptr.is_null() {
                None
            } else {
                let video_surf = gst_wl_window_get_wl_surface(window_ptr);
                if video_surf.is_null() {
                    None
                } else {
                    // In GstWlWindowPrivate, video_surface_wrapper is at private_offset + 0x50,
                    // and area_surface_wrapper is at private_offset + 0x30 (exactly 0x20 bytes before).
                    // In GLib, private_offset is negative relative to GObject instance window_ptr.
                    // We dynamically find the pointer slot holding video_surf in window_ptr memory
                    // and read area_surface_wrapper 32 bytes before it.
                    let win_bytes = window_ptr as *const u8;
                    let mut area_surf = None;
                    for off in (-0x400..0x0).step_by(8) {
                        let ptr = *(win_bytes.offset(off) as *const *mut std::ffi::c_void);
                        if ptr == video_surf {
                            let area_ptr =
                                *(win_bytes.offset(off - 0x20) as *const *mut std::ffi::c_void);
                            if !area_ptr.is_null() {
                                area_surf = Some(area_ptr);
                            }
                            break;
                        }
                    }

                    Some((video_surf, area_surf))
                }
            }
        }
    }
}

impl Drop for PipelineHandle {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gstreamer::State::Null);
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
    fn test_inspect_sink_offsets() {
        let _ = gstreamer::init();
        let path = Path::new("/home/susie/Videos/Fluffy-Loops/cyberpunk-night-city.1920x1080.mp4");
        if !path.exists() {
            return;
        }
        let playbin = gstreamer::ElementFactory::make("playbin")
            .property("uri", format!("file://{}", path.display()))
            .build()
            .unwrap();
        let sink = gstreamer::ElementFactory::make("waylandsink")
            .build()
            .unwrap();
        playbin.set_property("video-sink", &sink);
        let _ = playbin.set_state(gstreamer::State::Paused);
        let _ = playbin.state(gstreamer::ClockTime::from_seconds(2));

        let sink_ptr = sink.as_ptr() as *const u8;
        let window_ptr = unsafe { *(sink_ptr.add(0x2d8) as *const *mut std::ffi::c_void) };
        if !window_ptr.is_null() {
            unsafe {
                use wayland_client::Proxy;
                let surf = gst_wl_window_get_wl_surface(window_ptr);
                if !surf.is_null() {
                    let win_bytes = window_ptr as *const u8;
                    let mut found_area = None;
                    for off in (-0x400..0x0).step_by(8) {
                        let ptr = *(win_bytes.offset(off) as *const *mut std::ffi::c_void);
                        if ptr == surf {
                            let area_ptr =
                                *(win_bytes.offset(off - 0x20) as *const *mut std::ffi::c_void);
                            found_area = Some(area_ptr);
                            break;
                        }
                    }
                    if let Some(area_ptr) = found_area {
                        assert!(!area_ptr.is_null());
                        let obj_id = wayland_client::backend::ObjectId::from_ptr(
                            &wayland_client::protocol::wl_surface::WlSurface::interface(),
                            area_ptr as *mut _,
                        );
                        assert!(obj_id.is_ok());
                    }
                }
            }
        }
        let _ = playbin.set_state(gstreamer::State::Null);
    }

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
