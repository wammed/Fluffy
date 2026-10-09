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

            enum GstPrerollEventKind {
                AsyncStart,
                AsyncDone,
                StateChanged {
                    old: gstreamer::State,
                    current: gstreamer::State,
                    pending: gstreamer::State,
                },
                Error {
                    error: String,
                    debug_info: Option<String>,
                },
            }

            struct GstPrerollEvent {
                elapsed_ms: f64,
                src: String,
                kind: GstPrerollEventKind,
            }

            struct BusSyncHandlerGuard<'a>(&'a gstreamer::Bus);

            impl Drop for BusSyncHandlerGuard<'_> {
                fn drop(&mut self) {
                    self.0.unset_sync_handler();
                }
            }

            // Transition to Paused so preroll renders the first frame into waylandsink subsurface
            let paused_start = std::time::Instant::now();
            tracing::info!(
                event = "pipeline_set_paused_started",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                "[Player] Setting new pipeline state to PAUSED started"
            );

            let recorded_events = std::sync::Arc::new(std::sync::Mutex::new(
                Vec::<GstPrerollEvent>::with_capacity(16),
            ));
            let events_cb = recorded_events.clone();
            let p_ref = new_handle.pipeline.clone();
            let s_ref = new_handle.sink.clone();

            new_handle.bus.set_sync_handler(move |_bus, msg| {
                use gstreamer::MessageView;

                let is_pipeline = msg.src() == Some(p_ref.upcast_ref());
                let is_sink = msg.src() == Some(s_ref.upcast_ref());

                let event_opt = match msg.view() {
                    MessageView::AsyncStart(..) if is_pipeline || is_sink => {
                        Some(GstPrerollEvent {
                            elapsed_ms: paused_start.elapsed().as_secs_f64() * 1000.0,
                            src: if is_pipeline {
                                "pipeline".to_string()
                            } else {
                                "waylandsink".to_string()
                            },
                            kind: GstPrerollEventKind::AsyncStart,
                        })
                    }
                    MessageView::AsyncDone(..) if is_pipeline || is_sink => Some(GstPrerollEvent {
                        elapsed_ms: paused_start.elapsed().as_secs_f64() * 1000.0,
                        src: if is_pipeline {
                            "pipeline".to_string()
                        } else {
                            "waylandsink".to_string()
                        },
                        kind: GstPrerollEventKind::AsyncDone,
                    }),
                    MessageView::StateChanged(sc) if is_pipeline || is_sink => {
                        Some(GstPrerollEvent {
                            elapsed_ms: paused_start.elapsed().as_secs_f64() * 1000.0,
                            src: if is_pipeline {
                                "pipeline".to_string()
                            } else {
                                "waylandsink".to_string()
                            },
                            kind: GstPrerollEventKind::StateChanged {
                                old: sc.old(),
                                current: sc.current(),
                                pending: sc.pending(),
                            },
                        })
                    }
                    MessageView::Error(err) => Some(GstPrerollEvent {
                        elapsed_ms: paused_start.elapsed().as_secs_f64() * 1000.0,
                        src: msg
                            .src()
                            .map(|s| s.name().to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                        kind: GstPrerollEventKind::Error {
                            error: err.error().to_string(),
                            debug_info: err.debug().map(|d| d.to_string()),
                        },
                    }),
                    _ => None,
                };

                if let Some(ev) = event_opt {
                    if let Ok(mut lock) = events_cb.lock() {
                        lock.push(ev);
                    }
                }

                gstreamer::BusSyncReply::Pass
            });
            let sync_guard = BusSyncHandlerGuard(&new_handle.bus);

            new_handle.pipeline.set_state(gstreamer::State::Paused)?;

            let paused_elapsed_ms = paused_start.elapsed().as_secs_f64() * 1000.0;
            tracing::info!(
                event = "pipeline_set_paused_returned",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                elapsed_ms = paused_elapsed_ms,
                "[Player] Setting new pipeline state to PAUSED returned"
            );

            // Wait for preroll to complete (first frame is committed to the compositor)
            let preroll_start = std::time::Instant::now();
            tracing::info!(
                event = "preroll_wait_started",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                "[Player] Waiting for new pipeline preroll started"
            );

            let (state_change_res, current_st, pending_st) = new_handle
                .pipeline
                .state(gstreamer::ClockTime::from_mseconds(500));

            let preroll_elapsed_ms = preroll_start.elapsed().as_secs_f64() * 1000.0;
            tracing::info!(
                event = "preroll_wait_returned",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                elapsed_ms = preroll_elapsed_ms,
                state_change_res = ?state_change_res,
                current_st = ?current_st,
                pending_st = ?pending_st,
                "[Player] Waiting for new pipeline preroll returned"
            );

            // Disarm sync handler before flushing recorded events
            drop(sync_guard);

            // Emit structured events recorded during PAUSED transition & preroll
            if let Ok(mut events) = recorded_events.lock() {
                for ev in events.drain(..) {
                    match ev.kind {
                        GstPrerollEventKind::AsyncStart => {
                            tracing::info!(
                                event = "gst_sync_async_start",
                                output = %output,
                                generation = generation,
                                session_id = %sid,
                                elapsed_ms = ev.elapsed_ms,
                                src = %ev.src,
                                "[Player] GStreamer sync event: ASYNC_START"
                            );
                        }
                        GstPrerollEventKind::AsyncDone => {
                            tracing::info!(
                                event = "gst_sync_async_done",
                                output = %output,
                                generation = generation,
                                session_id = %sid,
                                elapsed_ms = ev.elapsed_ms,
                                src = %ev.src,
                                "[Player] GStreamer sync event: ASYNC_DONE"
                            );
                        }
                        GstPrerollEventKind::StateChanged {
                            old,
                            current,
                            pending,
                        } => {
                            tracing::info!(
                                event = "gst_sync_state_changed",
                                output = %output,
                                generation = generation,
                                session_id = %sid,
                                elapsed_ms = ev.elapsed_ms,
                                src = %ev.src,
                                old_state = ?old,
                                current_state = ?current,
                                pending_state = ?pending,
                                "[Player] GStreamer sync event: STATE_CHANGED"
                            );
                        }
                        GstPrerollEventKind::Error { error, debug_info } => {
                            tracing::error!(
                                event = "gst_sync_error",
                                output = %output,
                                generation = generation,
                                session_id = %sid,
                                elapsed_ms = ev.elapsed_ms,
                                src = %ev.src,
                                error = %error,
                                debug = ?debug_info,
                                "[Player] GStreamer sync event: ERROR"
                            );
                        }
                    }
                }
            }

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
            let playing_start = std::time::Instant::now();
            tracing::info!(
                event = "pipeline_set_playing_started",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                "[Player] Setting new pipeline state to PLAYING started"
            );

            new_handle.pipeline.set_state(gstreamer::State::Playing)?;

            let playing_elapsed_ms = playing_start.elapsed().as_secs_f64() * 1000.0;
            tracing::info!(
                event = "pipeline_set_playing_returned",
                output = %output,
                generation = generation,
                video_id = %video_id,
                session_id = %sid,
                elapsed_ms = playing_elapsed_ms,
                "[Player] Setting new pipeline state to PLAYING returned"
            );

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
