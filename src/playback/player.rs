use std::path::{Path, PathBuf};

use gstreamer::prelude::*;
use gstreamer_video::prelude::*;

use crate::error::Result;
use super::pipeline::PipelineHandle;
use super::state::PlaybackState;

pub trait VideoPlayer {
    fn play(&mut self, video: &Path) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn state(&self) -> PlaybackState;
    fn current_video(&self) -> Option<&Path>;
    fn poll_events(&mut self) -> Result<bool>; // Returns false if playback encountered a fatal error
}

pub struct GstVideoPlayer {
    raw_display_ptr: *mut std::ffi::c_void,
    raw_surface_ptr: usize,
    width: u32,
    height: u32,
    pipeline_handle: Option<PipelineHandle>,
    current_video: Option<PathBuf>,
    state: PlaybackState,
    loop_count: u64,
}

impl GstVideoPlayer {
    pub fn new(
        raw_display_ptr: *mut std::ffi::c_void,
        raw_surface_ptr: usize,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        gstreamer::init()?;

        Ok(Self {
            raw_display_ptr,
            raw_surface_ptr,
            width,
            height,
            pipeline_handle: None,
            current_video: None,
            state: PlaybackState::Stopped,
            loop_count: 0,
        })
    }

    pub fn loop_count(&self) -> u64 {
        self.loop_count
    }

    pub fn update_geometry(&mut self, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        if let Some(ref handle) = self.pipeline_handle
            && let Err(e) = handle
                .overlay
                .set_render_rectangle(0, 0, width as i32, height as i32)
        {
            tracing::trace!(operation = "render_rectangle", error = ?e, "[Player] set_render_rectangle ignored during geometry update");
        }
        Ok(())
    }
}

impl VideoPlayer for GstVideoPlayer {
    fn play(&mut self, video: &Path) -> Result<()> {
        // Seamless transition with Dual Pipeline:
        // 1. If an active pipeline exists, keep it running so old frames remain visible.
        // 2. Instantiate a new pipeline for the incoming video targeting the same layer surface.
        // 3. Set the new pipeline to PAUSED to preroll the first frame onto its new subsurface.
        // 4. Wait for preroll to complete (first frame is committed to the compositor).
        // 5. Transition the new pipeline to PLAYING.
        // 6. Tear down the old pipeline (its subsurface is cleanly removed underneath).
        if self.pipeline_handle.is_some() {
            tracing::info!(
                operation = "pipeline_switch",
                video = ?video,
                "[Player] Seamless transition: prerolling new video before releasing old"
            );

            let new_handle = unsafe {
                PipelineHandle::new(
                    self.raw_display_ptr,
                    self.raw_surface_ptr,
                    self.width,
                    self.height,
                    video,
                )?
            };

            // Transition to Paused so preroll renders the first frame into waylandsink subsurface
            new_handle.pipeline.set_state(gstreamer::State::Paused)?;

            // Wait for preroll to complete (first frame is committed to the compositor)
            let (state_change_res, current_st, pending_st) =
                new_handle.pipeline.state(gstreamer::ClockTime::from_seconds(3));
            tracing::debug!(
                operation = "preroll",
                res = ?state_change_res,
                current = ?current_st,
                pending = ?pending_st,
                "[Player] Preroll completed"
            );

            // Now transition new pipeline to Playing
            new_handle.pipeline.set_state(gstreamer::State::Playing)?;

            // Swap out old handle and tear it down cleanly
            let old_handle = self.pipeline_handle.replace(new_handle);
            if let Some(old) = old_handle
                && let Err(e) = old.pipeline.set_state(gstreamer::State::Null)
            {
                tracing::warn!(operation = "pipeline_teardown", error = %e, "[Player] Failed to set old pipeline state to Null");
            }

            self.current_video = Some(video.to_path_buf());
            self.state = PlaybackState::Playing;
            self.loop_count = 0;

            tracing::info!(operation = "pipeline_switch_complete", video = ?video, "[Player] Seamless transition finished");
            return Ok(());
        }

        // Initial playback: create pipeline and start
        let handle = unsafe {
            PipelineHandle::new(
                self.raw_display_ptr,
                self.raw_surface_ptr,
                self.width,
                self.height,
                video,
            )?
        };

        handle.pipeline.set_state(gstreamer::State::Playing)?;

        self.pipeline_handle = Some(handle);
        self.current_video = Some(video.to_path_buf());
        self.state = PlaybackState::Playing;
        self.loop_count = 0;

        tracing::info!(operation = "pipeline_start", video = ?video, "[Player] Started initial playback");
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        if let Some(ref handle) = self.pipeline_handle {
            handle.pipeline.set_state(gstreamer::State::Paused)?;
            self.state = PlaybackState::Paused;
            tracing::info!(operation = "playback_pause", "[Player] Playback PAUSED");
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if let Some(ref handle) = self.pipeline_handle {
            handle.pipeline.set_state(gstreamer::State::Playing)?;
            self.state = PlaybackState::Playing;
            tracing::info!(operation = "playback_resume", "[Player] Playback RESUMED");
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.pipeline_handle.take() {
            if let Err(e) = handle.pipeline.set_state(gstreamer::State::Null) {
                tracing::warn!(operation = "pipeline_teardown", error = %e, "[Player] Failed to set pipeline state to Null during stop");
            }
            self.state = PlaybackState::Stopped;
            self.current_video = None;
            tracing::info!(operation = "playback_stop", "[Player] Playback STOPPED");
        }
        Ok(())
    }

    fn state(&self) -> PlaybackState {
        self.state
    }

    fn current_video(&self) -> Option<&Path> {
        self.current_video.as_deref()
    }

    fn poll_events(&mut self) -> Result<bool> {
        let Some(ref handle) = self.pipeline_handle else {
            return Ok(true);
        };

        while let Some(msg) = handle.bus.timed_pop(gstreamer::ClockTime::from_mseconds(50)) {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    self.loop_count += 1;
                    tracing::debug!(
                        operation = "eos_loop",
                        cycle = self.loop_count,
                        "[Player] EOS reached; seeking to 0 for seamless loop"
                    );
                    let res = handle.pipeline.seek_simple(
                        gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                        gstreamer::ClockTime::ZERO,
                    );
                    if let Err(e) = res {
                        tracing::warn!(operation = "seek_loop", error = ?e, "[Player] Seek to 0 failed during loop");
                    }
                }
                MessageView::Error(err) => {
                    tracing::error!(
                        operation = "gst_bus_error",
                        error = %err.error(),
                        debug = ?err.debug(),
                        "[Player] GStreamer Error received from bus"
                    );
                    self.state = PlaybackState::Stopped;
                    return Ok(false);
                }
                MessageView::NeedContext(msg)
                    if msg.context_type() == "GstWaylandDisplayHandleContextType"
                        || msg.context_type() == "GstWlDisplayHandleContextType" =>
                {
                    if let Some(elem) = msg
                        .src()
                        .and_then(|src| src.clone().downcast::<gstreamer::Element>().ok())
                    {
                        elem.set_context(&handle.gst_wl_context);
                    }
                }
                MessageView::Element(msg)
                    if gstreamer_video::is_video_overlay_prepare_window_handle_message(msg) =>
                {
                    unsafe {
                        handle.overlay.set_window_handle(self.raw_surface_ptr);
                    }
                    if let Err(e) = handle
                        .overlay
                        .set_render_rectangle(0, 0, self.width as i32, self.height as i32)
                    {
                        tracing::trace!(operation = "render_rectangle", error = ?e, "[Player] Overlay set_render_rectangle ignored");
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }
}
