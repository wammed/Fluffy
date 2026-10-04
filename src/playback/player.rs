use std::path::{Path, PathBuf};

use gstreamer::prelude::*;
use gstreamer_video::prelude::*;

use super::pipeline::PipelineHandle;
use super::state::PlaybackState;
use crate::error::{FluffyError, Result};

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
    output_name: String,
    raw_display_ptr: *mut std::ffi::c_void,
    raw_surface_ptr: usize,
    width: u32,
    height: u32,
    pipeline_handle: Option<PipelineHandle>,
    current_video: Option<PathBuf>,
    state: PlaybackState,
    generation: u64,
    loop_count: u64,
}

impl GstVideoPlayer {
    pub fn new(
        raw_display_ptr: *mut std::ffi::c_void,
        raw_surface_ptr: usize,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        Self::new_with_name("default", raw_display_ptr, raw_surface_ptr, width, height)
    }

    pub fn new_with_name(
        output_name: impl Into<String>,
        raw_display_ptr: *mut std::ffi::c_void,
        raw_surface_ptr: usize,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        gstreamer::init()?;

        Ok(Self {
            output_name: output_name.into(),
            raw_display_ptr,
            raw_surface_ptr,
            width,
            height,
            pipeline_handle: None,
            current_video: None,
            state: PlaybackState::Stopped,
            generation: 0,
            loop_count: 0,
        })
    }

    pub fn loop_count(&self) -> u64 {
        if let Some(ref handle) = self.pipeline_handle {
            handle.loop_count.load(std::sync::atomic::Ordering::SeqCst)
        } else {
            self.loop_count
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn output_name(&self) -> &str {
        &self.output_name
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

    /// Plays a video with generation tracking and benchmark lifecycle event emissions.
    pub fn play_with_generation(
        &mut self,
        video: &Path,
        generation: u64,
        old_generation: u64,
    ) -> Result<()> {
        let sid = crate::benchmark::session_id();
        let video_id = crate::benchmark::safe_video_id(video);
        let output = &self.output_name;

        // Seamless transition with Dual Pipeline:
        // 1. If an active pipeline exists, keep it running so old frames remain visible.
        // 2. Instantiate a new pipeline for the incoming video targeting the same layer surface.
        // 3. Set the new pipeline to PAUSED to preroll the first frame onto its new subsurface.
        // 4. Wait for preroll to complete (first frame is committed to the compositor).
        // 5. Transition the new pipeline to PLAYING.
        // 6. Tear down the old pipeline (its subsurface is cleanly removed underneath).
        if self.pipeline_handle.is_some() {
            tracing::info!(
                event = "new_pipeline_created",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                "[Player] New pipeline created for video switch"
            );

            let new_handle = unsafe {
                PipelineHandle::new(
                    &self.output_name,
                    generation,
                    self.raw_display_ptr,
                    self.raw_surface_ptr,
                    self.width,
                    self.height,
                    video,
                )?
            };

            tracing::info!(
                event = "new_pipeline_preroll_started",
                output = %output,
                generation = generation,
                session_id = %sid,
                "[Player] New pipeline preroll started"
            );

            // Transition to Paused so preroll renders the first frame into waylandsink subsurface
            new_handle.pipeline.set_state(gstreamer::State::Paused)?;

            // Wait for preroll to complete (first frame is committed to the compositor)
            let (state_change_res, current_st, pending_st) = new_handle
                .pipeline
                .state(gstreamer::ClockTime::from_mseconds(500));
            tracing::debug!(
                operation = "preroll",
                res = ?state_change_res,
                current = ?current_st,
                pending = ?pending_st,
                "[Player] Preroll completed"
            );

            match state_change_res {
                Ok(gstreamer::StateChangeSuccess::Success)
                | Ok(gstreamer::StateChangeSuccess::NoPreroll) => {
                    // Preroll successful
                }
                other => {
                    let _ = new_handle.pipeline.set_state(gstreamer::State::Null);
                    tracing::error!(
                        operation = "preroll_failed",
                        output = %output,
                        generation = generation,
                        result = ?other,
                        current = ?current_st,
                        pending = ?pending_st,
                        "[Player] Preroll failed or timed out; preserving current playback"
                    );
                    return Err(FluffyError::Playback(format!(
                        "Pipeline preroll failed for video {:?}: result={:?}, current={:?}, pending={:?}",
                        video, other, current_st, pending_st
                    )));
                }
            }

            tracing::info!(
                event = "new_pipeline_displayable",
                output = %output,
                generation = generation,
                session_id = %sid,
                "[Player] New pipeline first frame displayable"
            );

            // Now transition new pipeline to Playing
            new_handle.pipeline.set_state(gstreamer::State::Playing)?;

            tracing::debug!(
                event = "pipeline_playing",
                output = %output,
                generation = generation,
                session_id = %sid,
                "[Player] New pipeline transitioned to Playing"
            );

            // Swap out old handle and tear it down cleanly
            let old_handle = self.pipeline_handle.replace(new_handle);
            tracing::info!(
                event = "video_switch_committed",
                output = %output,
                generation = generation,
                session_id = %sid,
                "[Player] Video switch committed"
            );

            if let Some(old) = old_handle {
                tracing::info!(
                    event = "old_pipeline_teardown_started",
                    output = %output,
                    generation = old_generation,
                    session_id = %sid,
                    "[Player] Old pipeline teardown started"
                );
                if let Err(e) = old.pipeline.set_state(gstreamer::State::Null) {
                    tracing::warn!(
                        event = "pipeline_error",
                        output = %output,
                        generation = old_generation,
                        session_id = %sid,
                        error = %e,
                        "[Player] Failed to set old pipeline state to Null"
                    );
                }
                tracing::info!(
                    event = "old_pipeline_set_null_returned",
                    output = %output,
                    generation = old_generation,
                    session_id = %sid,
                    "[Player] Old pipeline set_state(NULL) returned (API returned; internal resources may still be releasing asynchronously)"
                );
                tracing::info!(
                    event = "old_pipeline_teardown_completed",
                    output = %output,
                    generation = old_generation,
                    session_id = %sid,
                    "[Player] Old pipeline teardown completed"
                );
            }

            self.current_video = Some(video.to_path_buf());
            self.state = PlaybackState::Playing;
            self.generation = generation;
            self.loop_count = 0;

            tracing::info!(
                event = "playback_started",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                "[Player] Playback started after video switch"
            );
            return Ok(());
        }

        // Initial playback: create pipeline and start
        tracing::info!(
            event = "pipeline_created",
            output = %output,
            generation = generation,
            video_id = %video_id,
            session_id = %sid,
            "[Player] Pipeline created for initial playback"
        );

        let handle = unsafe {
            PipelineHandle::new(
                &self.output_name,
                generation,
                self.raw_display_ptr,
                self.raw_surface_ptr,
                self.width,
                self.height,
                video,
            )?
        };

        handle.pipeline.set_state(gstreamer::State::Playing)?;

        tracing::debug!(
            event = "pipeline_playing",
            output = %output,
            generation = generation,
            session_id = %sid,
            "[Player] Initial pipeline transitioned to Playing"
        );

        self.pipeline_handle = Some(handle);
        self.current_video = Some(video.to_path_buf());
        self.state = PlaybackState::Playing;
        self.generation = generation;
        self.loop_count = 0;

        tracing::info!(
            event = "playback_started",
            output = %output,
            generation = generation,
            video_id = %video_id,
            session_id = %sid,
            "[Player] Initial playback started"
        );
        Ok(())
    }
}

impl VideoPlayer for GstVideoPlayer {
    fn play(&mut self, video: &Path) -> Result<()> {
        let current_gen = self.generation;
        self.play_with_generation(video, current_gen.saturating_add(1), current_gen)
    }

    fn pause(&mut self) -> Result<()> {
        if let Some(ref handle) = self.pipeline_handle {
            handle.pipeline.set_state(gstreamer::State::Paused)?;
            self.state = PlaybackState::Paused;
            let sid = crate::benchmark::session_id();
            tracing::debug!(
                event = "pipeline_paused",
                output = %self.output_name,
                generation = self.generation,
                session_id = %sid,
                "[Player] Pipeline state set to Paused"
            );
            tracing::info!(
                event = "playback_paused",
                output = %self.output_name,
                generation = self.generation,
                session_id = %sid,
                "[Player] Playback PAUSED"
            );
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if let Some(ref handle) = self.pipeline_handle {
            handle.pipeline.set_state(gstreamer::State::Playing)?;
            self.state = PlaybackState::Playing;
            let sid = crate::benchmark::session_id();
            tracing::debug!(
                event = "pipeline_playing",
                output = %self.output_name,
                generation = self.generation,
                session_id = %sid,
                "[Player] Pipeline state set to Playing"
            );
            tracing::info!(
                event = "playback_resumed",
                output = %self.output_name,
                generation = self.generation,
                session_id = %sid,
                "[Player] Playback RESUMED"
            );
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.pipeline_handle.take() {
            let sid = crate::benchmark::session_id();
            if let Err(e) = handle.pipeline.set_state(gstreamer::State::Null) {
                tracing::warn!(
                    event = "pipeline_error",
                    output = %self.output_name,
                    generation = self.generation,
                    session_id = %sid,
                    error = %e,
                    "[Player] Failed to set pipeline state to Null during stop"
                );
            }
            tracing::debug!(
                event = "pipeline_stopped",
                output = %self.output_name,
                generation = self.generation,
                session_id = %sid,
                "[Player] Pipeline state set to Null"
            );
            self.loop_count = handle.loop_count.load(std::sync::atomic::Ordering::SeqCst);
            self.state = PlaybackState::Stopped;
            self.current_video = None;
            tracing::info!(
                event = "playback_stopped",
                output = %self.output_name,
                generation = self.generation,
                session_id = %sid,
                "[Player] Playback STOPPED"
            );
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

        while let Some(msg) = handle.bus.pop() {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    let count = handle
                        .loop_count
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                        + 1;
                    let sid = crate::benchmark::session_id();
                    tracing::debug!(
                        event = "playback_loop_eos_fallback",
                        output = %self.output_name,
                        generation = self.generation,
                        loop_count = count,
                        session_id = %sid,
                        "[Player] EOS reached; seeking to 0 (fallback loop)"
                    );
                    let res = handle.pipeline.seek_simple(
                        gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::ACCURATE,
                        gstreamer::ClockTime::ZERO,
                    );
                    if let Err(e) = res {
                        tracing::warn!(operation = "seek_loop", error = ?e, "[Player] Seek to 0 failed during loop");
                    }
                }
                MessageView::Error(err) => {
                    let sid = crate::benchmark::session_id();
                    tracing::error!(
                        event = "pipeline_error",
                        output = %self.output_name,
                        generation = self.generation,
                        session_id = %sid,
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
                    if let Err(e) = handle.overlay.set_render_rectangle(
                        0,
                        0,
                        self.width as i32,
                        self.height as i32,
                    ) {
                        tracing::trace!(operation = "render_rectangle", error = ?e, "[Player] Overlay set_render_rectangle ignored");
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }
}
