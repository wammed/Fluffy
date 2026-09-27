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
}

impl VideoPlayer for GstVideoPlayer {
    fn play(&mut self, video: &Path) -> Result<()> {
        let abs_path = video.canonicalize()?;
        let uri = format!("file://{}", abs_path.display());

        // Seamless switching: If pipeline already exists, reuse it and change URI
        // to avoid destroying waylandsink and flashing the underlying desktop wallpaper.
        if let Some(ref handle) = self.pipeline_handle {
            println!("[Player] Seamlessly switching video to: {:?}", video);

            // Change to Ready to reset demuxer/decoders while preserving waylandsink surface
            handle.pipeline.set_state(gstreamer::State::Ready)?;
            handle.pipeline.set_property("uri", uri);

            // Transition to Paused for prerolling the first frame
            handle.pipeline.set_state(gstreamer::State::Paused)?;
            let _ = handle.pipeline.state(gstreamer::ClockTime::from_mseconds(500));

            // Start playing new video
            handle.pipeline.set_state(gstreamer::State::Playing)?;

            self.current_video = Some(video.to_path_buf());
            self.state = PlaybackState::Playing;
            self.loop_count = 0;

            println!("[Player] Switched playback to: {:?}", video);
            return Ok(());
        }

        // Initial playback: create pipeline and start
        let handle = PipelineHandle::new(
            self.raw_display_ptr,
            self.raw_surface_ptr,
            self.width,
            self.height,
            video,
        )?;

        handle.pipeline.set_state(gstreamer::State::Playing)?;

        self.pipeline_handle = Some(handle);
        self.current_video = Some(video.to_path_buf());
        self.state = PlaybackState::Playing;
        self.loop_count = 0;

        println!("[Player] Started initial playback for: {:?}", video);
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        if let Some(ref handle) = self.pipeline_handle {
            handle.pipeline.set_state(gstreamer::State::Paused)?;
            self.state = PlaybackState::Paused;
            println!("[Player] Playback PAUSED");
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if let Some(ref handle) = self.pipeline_handle {
            handle.pipeline.set_state(gstreamer::State::Playing)?;
            self.state = PlaybackState::Playing;
            println!("[Player] Playback RESUMED");
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.pipeline_handle.take() {
            let _ = handle.pipeline.set_state(gstreamer::State::Null);
            self.state = PlaybackState::Stopped;
            self.current_video = None;
            println!("[Player] Playback STOPPED");
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
                    println!(
                        "[Player] EOS reached! Seamless loop (cycle #{}). Seeking to 0...",
                        self.loop_count
                    );
                    let res = handle.pipeline.seek_simple(
                        gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                        gstreamer::ClockTime::ZERO,
                    );
                    if let Err(e) = res {
                        eprintln!("[Player] Seek to 0 failed: {e:?}");
                    }
                }
                MessageView::Error(err) => {
                    eprintln!(
                        "[Player] GStreamer Error: {} ({:?})",
                        err.error(),
                        err.debug()
                    );
                    self.state = PlaybackState::Stopped;
                    return Ok(false);
                }
                MessageView::NeedContext(msg) => {
                    let ctx_type = msg.context_type();
                    if ctx_type == "GstWaylandDisplayHandleContextType"
                        || ctx_type == "GstWlDisplayHandleContextType"
                    {
                        if let Some(src) = msg.src() {
                            if let Ok(elem) = src.clone().downcast::<gstreamer::Element>() {
                                elem.set_context(&handle.gst_wl_context);
                            }
                        }
                    }
                }
                MessageView::Element(msg) => {
                    if gstreamer_video::is_video_overlay_prepare_window_handle_message(msg) {
                        unsafe {
                            handle.overlay.set_window_handle(self.raw_surface_ptr);
                        }
                        let _ = handle
                            .overlay
                            .set_render_rectangle(0, 0, self.width as i32, self.height as i32);
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }
}
