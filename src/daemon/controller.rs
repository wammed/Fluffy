use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

use crate::{
    cache::CacheManager,
    error::{FluffyError, Result},
    ipc::{
        protocol::{
            CommandType, DaemonStatus, OutputStatus, RequestEnvelope, ResponseEnvelope,
        },
        server::{ClientId, PendingRequest},
        IpcServer,
    },
    playback::VideoPlayer,
    wayland::{WaylandContext, WaylandOutputEvent},
};
use super::output_manager::OutputManager;

struct TranscodeJobResult {
    request_id: u64,
    client_id: Option<ClientId>,
    target_output: Option<String>,
    generation: Option<u64>,
    result: Result<std::path::PathBuf>,
}

pub struct WallpaperDaemon {
    pub wayland_ctx: WaylandContext,
    pub outputs: OutputManager,
    pub cache: CacheManager,
    pub ipc_server: IpcServer,
    pub exit_flag: Arc<AtomicBool>,
    pub requested_output: Option<String>,
    transcode_tx: mpsc::Sender<TranscodeJobResult>,
    transcode_rx: mpsc::Receiver<TranscodeJobResult>,
    pub current_converting: Option<String>,
}

impl WallpaperDaemon {
    /// Initializes the wallpaper daemon on all detected outputs (or a specifically requested output),
    /// binding the IPC server to the provided socket path.
    pub fn new<P: AsRef<Path>>(
        socket_path: P,
        requested_output: Option<&str>,
        exit_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let cache = CacheManager::new(CacheManager::default_storage_dir())?;
        let mut wayland_ctx = WaylandContext::init()?;

        let outputs_info = wayland_ctx.outputs();
        if outputs_info.is_empty() {
            return Err(FluffyError::Wayland("No Wayland outputs detected".to_string()));
        }

        tracing::info!("[Daemon] Discovered Wayland outputs:");
        for (name, _) in &outputs_info {
            tracing::info!("  - {name}");
        }

        let mut output_manager = OutputManager::new();

        // If a specific output is requested, bind only to it;
        // otherwise, bind to ALL discovered outputs concurrently.
        let target_outputs: Vec<(String, _)> = if let Some(req_name) = requested_output {
            let found = outputs_info
                .into_iter()
                .find(|(name, _)| name == req_name)
                .ok_or_else(|| FluffyError::OutputNotFound(req_name.to_string()))?;
            vec![found]
        } else {
            outputs_info
        };

        for (name, wl_out) in target_outputs {
            output_manager.init_output(&mut wayland_ctx, name, wl_out)?;
        }

        tracing::info!(
            "[Daemon] Initialized {} active output(s) concurrently",
            output_manager.len()
        );

        let ipc_server = IpcServer::bind(socket_path)?;
        let (transcode_tx, transcode_rx) = mpsc::channel();

        Ok(Self {
            wayland_ctx,
            outputs: output_manager,
            cache,
            ipc_server,
            exit_flag,
            requested_output: requested_output.map(|s| s.to_string()),
            transcode_tx,
            transcode_rx,
            current_converting: None,
        })
    }


    /// Handles dynamic Wayland output addition, update, and removal (hotplug).
    pub fn handle_output_events(&mut self) -> Result<()> {
        let events = self.wayland_ctx.take_output_events();
        for event in events {
            match event {
                WaylandOutputEvent::AddedOrUpdated(wl_out) => {
                    if let Some(name) = self.wayland_ctx.output_name(&wl_out) {
                        // If daemon was started for a specific output, ignore other outputs
                        if let Some(ref req) = self.requested_output {
                            if req != &name {
                                continue;
                            }
                        }

                        if !self.outputs.contains(&name) {
                            tracing::info!("[Daemon] Discovered newly attached Wayland output: {name}");
                            match self.outputs.init_output(&mut self.wayland_ctx, name.clone(), wl_out) {
                                Ok(()) => {
                                    tracing::info!("[Daemon] Successfully initialized hotplugged output '{name}'");
                                    // If another output is already playing a wallpaper, match it
                                    if let Some(active_vid) = self.outputs.default_active_video() {
                                        tracing::info!(
                                            "[Daemon] Automatically applying active wallpaper to hotplugged output '{name}': {:?}",
                                            active_vid
                                        );
                                        let _ = self.outputs.set_video(Some(&name), &active_vid, None);
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "[Daemon] Failed to initialize newly attached output '{name}': {e}"
                                    );
                                }
                            }
                        }
                    }
                }
                WaylandOutputEvent::Destroyed(wl_out) => {
                    if let Some(removed_name) = self.outputs.remove_output_by_wl(&mut self.wayland_ctx, &wl_out) {
                        tracing::info!("[Daemon] Output '{removed_name}' disconnected and cleaned up.");
                    }
                }
            }
        }
        Ok(())
    }

    /// Handles a single IPC request envelope synchronously (used during initial daemon setup or direct calls).
    pub fn handle_request(&mut self, req: &RequestEnvelope) -> ResponseEnvelope {
        match req.command {
            CommandType::Status => {
                let mut output_statuses = Vec::new();
                for (name, out) in self.outputs.iter() {
                    output_statuses.push(OutputStatus {
                        name: name.clone(),
                        state: out.player.state().to_string(),
                        current_video: out
                            .player
                            .current_video()
                            .map(|p| p.to_string_lossy().to_string()),
                        generation: out.generation,
                        loop_count: out.player.loop_count(),
                    });
                }
                output_statuses.sort_by(|a, b| a.name.cmp(&b.name));

                let status = DaemonStatus {
                    daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                    outputs: output_statuses,
                    is_converting: self.current_converting.is_some(),
                    converting_file: self.current_converting.clone(),
                };

                match serde_json::to_value(&status) {
                    Ok(val) => ResponseEnvelope::success(req.request_id, Some(val)),
                    Err(e) => ResponseEnvelope::failure(req.request_id, e.to_string()),
                }
            }

            CommandType::SetVideo => {
                let Some(ref path) = req.path else {
                    return ResponseEnvelope::failure(
                        req.request_id,
                        "'set_video' command requires 'path'",
                    );
                };

                if !path.exists() {
                    return ResponseEnvelope::failure(
                        req.request_id,
                        format!("Video file does not exist: {:?}", path),
                    );
                }

                if let Some(ref target_name) = req.output {
                    if !self.outputs.contains(target_name) {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Output '{}' not managed by daemon", target_name),
                        );
                    }
                }

                let cached_path = match self.cache.import_video(path) {
                    Ok(p) => p,
                    Err(e) => {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Video import/normalization failed: {e}"),
                        );
                    }
                };

                if let Err(e) =
                    self.outputs
                        .set_video(req.output.as_deref(), &cached_path, req.generation)
                {
                    return ResponseEnvelope::failure(req.request_id, e.to_string());
                }

                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Pause => {
                if let Err(e) = self.outputs.pause(req.output.as_deref()) {
                    return ResponseEnvelope::failure(req.request_id, e.to_string());
                }
                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Resume => {
                if let Err(e) = self.outputs.resume(req.output.as_deref()) {
                    return ResponseEnvelope::failure(req.request_id, e.to_string());
                }
                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Stop => {
                if let Err(e) = self.outputs.stop(req.output.as_deref()) {
                    return ResponseEnvelope::failure(req.request_id, e.to_string());
                }
                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Reload => {
                if let Err(e) = self.outputs.reload(req.output.as_deref()) {
                    return ResponseEnvelope::failure(req.request_id, e.to_string());
                }
                ResponseEnvelope::success(req.request_id, None)
            }
        }
    }

    /// Handles an incoming IPC pending request. `SetVideo` commands are dispatched
    /// asynchronously to worker threads so the main event loop and ongoing video playback
    /// are never blocked, preventing black screens and stalled frame rendering.
    fn handle_pending_request(&mut self, pending: PendingRequest) {
        let req = pending.request;
        let client_id = pending.client_id;

        match req.command {
            CommandType::SetVideo => {
                let Some(ref path) = req.path else {
                    let resp = ResponseEnvelope::failure(
                        req.request_id,
                        "'set_video' command requires 'path'",
                    );
                    let _ = self.ipc_server.respond(client_id, &resp);
                    return;
                };

                if !path.exists() {
                    let resp = ResponseEnvelope::failure(
                        req.request_id,
                        format!("Video file does not exist: {:?}", path),
                    );
                    let _ = self.ipc_server.respond(client_id, &resp);
                    return;
                }

                if let Some(target_name) = req.output.as_deref() {
                    if !self.outputs.contains(target_name) {
                        let resp = ResponseEnvelope::failure(
                            req.request_id,
                            format!("Output '{}' not managed by daemon", target_name),
                        );
                        let _ = self.ipc_server.respond(client_id, &resp);
                        return;
                    }
                }

                let path_buf = path.clone();
                let storage_dir = self.cache.root_dir().to_path_buf();
                let tx = self.transcode_tx.clone();
                let request_id = req.request_id;
                let target_output = req.output.clone();
                let generation = req.generation;

                let file_name_str = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string());
                self.current_converting = Some(file_name_str);

                tracing::info!(
                    "[Daemon] Asynchronously normalizing and storing video: {:?} (target: {:?})",
                    path, target_output
                );

                // Run import_video on worker thread to never block main loop & Wayland dispatch
                thread::spawn(move || {
                    let manager = CacheManager::new(&storage_dir);
                    let result = match manager {
                        Ok(mgr) => mgr.import_video(&path_buf),
                        Err(e) => Err(e),
                    };

                    let _ = tx.send(TranscodeJobResult {
                        request_id,
                        client_id: Some(client_id),
                        target_output,
                        generation,
                        result,
                    });
                });
            }

            _ => {
                let resp = self.handle_request(&req);
                let _ = self.ipc_server.respond(client_id, &resp);
            }
        }
    }

    /// Performs one iteration of Wayland event dispatch, player polling, IPC handling,
    /// and background transcode job completion.
    pub fn step(&mut self) -> Result<()> {
        // 0. Poll completed background transcode jobs
        while let Ok(job) = self.transcode_rx.try_recv() {
            self.current_converting = None;
            match job.result {
                Ok(cached_path) => {
                    tracing::info!(
                        "[Daemon] Normalization complete, applying wallpaper seamlessly: {:?}",
                        cached_path
                    );
                    let apply_res = self.outputs.set_video(
                        job.target_output.as_deref(),
                        &cached_path,
                        job.generation,
                    );
                    if let Some(client_id) = job.client_id {
                        let resp = match apply_res {
                            Ok(()) => ResponseEnvelope::success(job.request_id, None),
                            Err(e) => ResponseEnvelope::failure(job.request_id, e.to_string()),
                        };
                        let _ = self.ipc_server.respond(client_id, &resp);
                    }
                }
                Err(e) => {
                    tracing::error!("[Daemon] Background normalization failed: {e}");
                    if let Some(client_id) = job.client_id {
                        let resp = ResponseEnvelope::failure(
                            job.request_id,
                            format!("Video normalization failed: {e}"),
                        );
                        let _ = self.ipc_server.respond(client_id, &resp);
                    }
                }
            }
        }

        // 1. Dispatch pending Wayland compositor events
        self.wayland_ctx.dispatch_pending()?;

        // 1b. Check for dynamic output hotplug events (attach / detach)
        self.handle_output_events()?;

        // 2. Poll GStreamer bus events for all managed outputs
        self.outputs.poll_events()?;

        // 3. Poll and process IPC requests
        let pending_requests = self.ipc_server.poll_requests()?;
        for pending in pending_requests {
            self.handle_pending_request(pending);
        }

        Ok(())
    }


    /// Runs the daemon main loop until a shutdown signal is received.
    pub fn run(&mut self) -> Result<()> {
        tracing::info!("[Daemon] Daemon main loop started. Ready for IPC commands.");

        while !self.exit_flag.load(Ordering::SeqCst) {
            self.step()?;
            // Sleep briefly to prevent busy-waiting when idle
            thread::sleep(Duration::from_millis(5));
        }

        tracing::info!("[Daemon] Shutdown signal detected. Performing clean teardown...");
        self.teardown();
        Ok(())
    }

    /// Performs clean teardown of all players and layer surfaces, restoring desktop wallpaper.
    pub fn teardown(&mut self) {
        self.outputs.teardown_all(&mut self.wayland_ctx);
    }
}
