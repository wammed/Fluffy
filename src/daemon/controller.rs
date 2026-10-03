use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use super::{
    job_manager::{JobId, JobManager},
    output_manager::OutputManager,
};
use crate::{
    cache::CacheManager,
    error::{FluffyError, Result},
    ipc::{
        IpcServer,
        protocol::{CommandType, DaemonStatus, OutputStatus, RequestEnvelope, ResponseEnvelope},
        server::{ClientId, PendingRequest},
    },
    playback::VideoPlayer,
    wayland::{WaylandContext, WaylandOutputEvent},
};

struct TranscodeJobResult {
    job_id: JobId,
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
    pub job_manager: JobManager,
}

impl WallpaperDaemon {
    /// Initializes the wallpaper daemon on all detected outputs (or a specifically requested output),
    /// binding the IPC server to the provided socket path.
    pub fn new<P: AsRef<Path>>(
        socket_path: P,
        requested_output: Option<&str>,
        exit_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let sid = crate::benchmark::session_id();
        let pid = std::process::id();
        tracing::info!(
            event = "daemon_started",
            session_id = %sid,
            pid = pid,
            version = env!("CARGO_PKG_VERSION"),
            socket = ?socket_path.as_ref(),
            requested_output = ?requested_output,
            "[Daemon] Initializing Fluffy wallpaper daemon..."
        );

        let cache = CacheManager::new(CacheManager::default_storage_dir())?;
        if let Err(e) = cache.cleanup_stale_temp_files() {
            tracing::debug!(error = %e, "[Storage] cleanup_stale_temp_files encountered an error on daemon startup");
        }
        let mut wayland_ctx = WaylandContext::init()?;

        let outputs_info = wayland_ctx.outputs();
        if outputs_info.is_empty() {
            return Err(FluffyError::Wayland(
                "No Wayland outputs detected".to_string(),
            ));
        }

        tracing::info!("[Daemon] Discovered Wayland outputs:");
        for (name, _) in &outputs_info {
            tracing::info!(output = %name, "  - Discovered output");
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
            count = output_manager.len(),
            "[Daemon] Initialized active output(s) concurrently"
        );

        let ipc_server = IpcServer::bind(socket_path)?;
        let (transcode_tx, transcode_rx) = mpsc::channel();

        let mut daemon = Self {
            wayland_ctx,
            outputs: output_manager,
            cache,
            ipc_server,
            exit_flag,
            requested_output: requested_output.map(|s| s.to_string()),
            transcode_tx,
            transcode_rx,
            job_manager: JobManager::new(),
        };

        // Opt-in restoration: only restores if enabled in settings
        let _ = daemon.restore_saved_state_if_enabled();

        Ok(daemon)
    }

    /// Checks settings and restores previous wallpaper state only if opt-in is enabled.
    pub fn restore_saved_state_if_enabled(&mut self) -> Result<()> {
        let config = crate::config::FluffyConfig::load();
        if !config.startup_and_wallpaper.restore_on_startup {
            tracing::debug!(
                "[Daemon] restore_on_startup is disabled in settings; starting in stateless mode"
            );
            return Ok(());
        }

        tracing::info!(
            "[Daemon] restore_on_startup is enabled in settings; restoring saved wallpaper..."
        );
        let state = crate::config::DaemonState::load();
        for (output_name, video_path_str) in &state.outputs {
            let path = Path::new(video_path_str);
            if !path.exists() {
                tracing::warn!(output = %output_name, path = %video_path_str, "[Daemon] Saved wallpaper video file not found");
                continue;
            }

            if self
                .requested_output
                .as_ref()
                .is_none_or(|req| req == output_name)
            {
                if let Err(e) = self.outputs.set_video(Some(output_name), path, None) {
                    tracing::warn!(output = %output_name, error = %e, "[Daemon] Failed to restore saved wallpaper for output");
                } else {
                    tracing::info!(output = %output_name, path = %video_path_str, "[Daemon] Successfully restored saved wallpaper for output");
                }
            }
        }
        Ok(())
    }

    fn save_output_wallpaper_state(&self, target_output: Option<&str>, video_path: &Path) {
        let mut state = crate::config::DaemonState::load();
        if let Some(target) = target_output {
            state.record_output_video(target, video_path);
        } else {
            for (out_name, _) in self.outputs.iter() {
                state.record_output_video(out_name, video_path);
            }
        }
        if let Err(e) = state.save() {
            tracing::debug!(error = %e, "[Daemon] Failed to save wallpaper state");
        }
    }

    /// Handles dynamic Wayland output addition, update, and removal (hotplug).
    pub fn handle_output_events(&mut self) -> Result<()> {
        let events = self.wayland_ctx.take_output_events();
        for event in events {
            match event {
                WaylandOutputEvent::AddedOrUpdated(wl_out) => {
                    if let Some(name) = self.wayland_ctx.output_name(&wl_out) {
                        // If daemon was started for a specific output, ignore other outputs
                        if let Some(ref req) = self.requested_output
                            && req != &name
                        {
                            continue;
                        }

                        if !self.outputs.contains(&name) {
                            tracing::info!(output = %name, "[Daemon] Discovered newly attached Wayland output");
                            match self.outputs.init_output(
                                &mut self.wayland_ctx,
                                name.clone(),
                                wl_out,
                            ) {
                                Ok(()) => {
                                    tracing::info!(output = %name, "[Daemon] Successfully initialized hotplugged output");
                                    // If another output is already playing a wallpaper, match it
                                    if let Some(active_vid) = self.outputs.default_active_video() {
                                        tracing::info!(
                                            output = %name,
                                            video = ?active_vid,
                                            "[Daemon] Automatically applying active wallpaper to hotplugged output"
                                        );
                                        if let Err(e) =
                                            self.outputs.set_video(Some(&name), &active_vid, None)
                                        {
                                            tracing::warn!(output = %name, error = %e, "[Daemon] Failed to apply active wallpaper to hotplugged output");
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        output = %name,
                                        error = %e,
                                        "[Daemon] Failed to initialize newly attached output"
                                    );
                                }
                            }
                        } else {
                            if let Some(geom) = self.wayland_ctx.output_geometry(&wl_out) {
                                match self.outputs.update_output_geometry(
                                    &mut self.wayland_ctx,
                                    &name,
                                    geom,
                                ) {
                                    Ok(true) => {
                                        tracing::info!(output = %name, "[Daemon] Output geometry updated successfully");
                                    }
                                    Ok(false) => {}
                                    Err(e) => {
                                        tracing::warn!(output = %name, error = %e, "[Daemon] Failed to update geometry for output");
                                    }
                                }
                            }
                        }
                    }
                }
                WaylandOutputEvent::Destroyed(wl_out) => {
                    if let Some(removed_name) = self
                        .outputs
                        .remove_output_by_wl(&mut self.wayland_ctx, &wl_out)
                    {
                        tracing::info!(output = %removed_name, "[Daemon] Output disconnected and cleaned up");
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
                        width: out.width,
                        height: out.height,
                        scale: out.scale,
                    });
                }
                output_statuses.sort_by(|a, b| a.name.cmp(&b.name));

                let status = DaemonStatus {
                    daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                    outputs: output_statuses,
                    is_converting: self.job_manager.is_converting(),
                    converting_file: self.job_manager.current_converting_file(),
                    active_jobs: self.job_manager.active_jobs(),
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

                if let Some(ref target_name) = req.output
                    && !self.outputs.contains(target_name)
                {
                    return ResponseEnvelope::failure(
                        req.request_id,
                        format!("Output '{}' not managed by daemon", target_name),
                    );
                }

                let generation = match self
                    .outputs
                    .allocate_generation(req.output.as_deref(), req.generation)
                {
                    Ok(g) => g,
                    Err(e) => return ResponseEnvelope::failure(req.request_id, e.to_string()),
                };

                let cached_path = match self.cache.import_video(path) {
                    Ok(p) => p,
                    Err(e) => {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Video import/normalization failed: {e}"),
                        );
                    }
                };

                let apply_result = match self.outputs.set_video(
                    req.output.as_deref(),
                    &cached_path,
                    Some(generation),
                ) {
                    Ok(r) => {
                        self.save_output_wallpaper_state(req.output.as_deref(), &cached_path);
                        r
                    }
                    Err(e) => return ResponseEnvelope::failure(req.request_id, e.to_string()),
                };

                let data = serde_json::to_value(&apply_result).ok();
                ResponseEnvelope::success(req.request_id, data)
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

            CommandType::Mark => {
                let Some(ref label) = req.label else {
                    return ResponseEnvelope::failure(
                        req.request_id,
                        "'mark' command requires 'label'",
                    );
                };
                let sid = crate::benchmark::session_id();
                tracing::info!(
                    event = "benchmark_mark",
                    label = %label,
                    session_id = %sid,
                    "[Benchmark] Benchmark mark recorded"
                );
                ResponseEnvelope::success(
                    req.request_id,
                    Some(serde_json::json!({
                        "label": label,
                        "session_id": sid,
                    })),
                )
            }
        }
    }

    fn send_ipc_response(&mut self, client_id: ClientId, resp: &ResponseEnvelope) {
        if let Err(e) = self.ipc_server.respond(client_id, resp) {
            tracing::debug!(
                client_id = client_id.0,
                request_id = resp.request_id,
                success = resp.success,
                error = %e,
                "[Daemon] Failed to deliver IPC response to client (client disconnected or timed out)"
            );
        }
    }

    /// Handles an incoming IPC pending request. `SetVideo` commands are dispatched
    /// asynchronously to worker threads so the main event loop and ongoing video playback
    /// are never blocked, preventing black screens and stalled frame rendering.
    fn handle_pending_request(&mut self, pending: PendingRequest) {
        let req = pending.request;
        let client_id = pending.client_id;

        tracing::debug!(
            client_id = client_id.0,
            request_id = req.request_id,
            command = ?req.command,
            output = ?req.output,
            "[Daemon] Received IPC request"
        );

        match req.command {
            CommandType::SetVideo => {
                let Some(ref path) = req.path else {
                    let resp = ResponseEnvelope::failure(
                        req.request_id,
                        "'set_video' command requires 'path'",
                    );
                    self.send_ipc_response(client_id, &resp);
                    return;
                };

                if !path.exists() {
                    let resp = ResponseEnvelope::failure(
                        req.request_id,
                        format!("Video file does not exist: {:?}", path),
                    );
                    self.send_ipc_response(client_id, &resp);
                    return;
                }

                if let Some(target_name) = req.output.as_deref()
                    && !self.outputs.contains(target_name)
                {
                    let resp = ResponseEnvelope::failure(
                        req.request_id,
                        format!("Output '{}' not managed by daemon", target_name),
                    );
                    self.send_ipc_response(client_id, &resp);
                    return;
                }

                let generation = match self
                    .outputs
                    .allocate_generation(req.output.as_deref(), req.generation)
                {
                    Ok(g) => g,
                    Err(e) => {
                        let resp = ResponseEnvelope::failure(req.request_id, e.to_string());
                        self.send_ipc_response(client_id, &resp);
                        return;
                    }
                };

                let sid = crate::benchmark::session_id();
                let video_id = crate::benchmark::safe_video_id(path);
                tracing::info!(
                    event = "ipc_set_video_received",
                    request_id = req.request_id,
                    output = ?req.output,
                    generation,
                    video_id = %video_id,
                    session_id = %sid,
                    "[Daemon] IPC set-video request received"
                );

                // Fast check: if the path is ALREADY inside the persistent cache videos directory,
                // apply it immediately without spawning a worker thread.
                let is_already_cached = path
                    .parent()
                    .map(|p| p == self.cache.videos_dir())
                    .unwrap_or(false);
                if is_already_cached && path.exists() {
                    let has_metadata = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(|hash| {
                            self.cache
                                .metadata_dir()
                                .join(format!("{hash}.json"))
                                .exists()
                        })
                        .unwrap_or(false);

                    if has_metadata {
                        tracing::info!(
                            operation = "cache_hit_direct",
                            path = ?path,
                            generation,
                            target = ?req.output,
                            "[Daemon] Direct cache hit: video already exists in persistent storage"
                        );
                        match self
                            .outputs
                            .set_video(req.output.as_deref(), path, Some(generation))
                        {
                            Ok(apply_result) => {
                                self.save_output_wallpaper_state(req.output.as_deref(), path);
                                let data = serde_json::to_value(&apply_result).ok();
                                let resp = ResponseEnvelope::success(req.request_id, data);
                                self.send_ipc_response(client_id, &resp);
                                return;
                            }
                            Err(e) => {
                                let resp = ResponseEnvelope::failure(req.request_id, e.to_string());
                                self.send_ipc_response(client_id, &resp);
                                return;
                            }
                        }
                    }
                }

                // Canonicalize path for stable in-flight job deduplication without blocking on full-file SHA-256
                let canonical_source = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

                // Register with JobManager (in-flight deduplication attaches subscriber without spawning duplicate worker)
                let (job_id, is_new) = self.job_manager.register_job(
                    &canonical_source,
                    None,
                    generation,
                    req.output.clone(),
                    req.request_id,
                    Some(client_id),
                );

                tracing::info!(
                    operation = "job_registered",
                    job_id,
                    generation,
                    is_new,
                    target = ?req.output,
                    path = ?canonical_source,
                    "[Daemon] Transcode job registered"
                );

                if is_new {
                    let path_buf = canonical_source;
                    let storage_dir = self.cache.root_dir().to_path_buf();
                    let tx = self.transcode_tx.clone();

                    // Run video probe, content hashing, and transcoding on a background worker thread
                    // to completely avoid blocking the daemon main loop and Wayland event dispatch.
                    thread::spawn(move || {
                        let manager = CacheManager::new(&storage_dir);
                        let result = match manager {
                            Ok(mgr) => mgr.import_video(&path_buf),
                            Err(e) => Err(e),
                        };

                        if let Err(e) = tx.send(TranscodeJobResult { job_id, result }) {
                            tracing::warn!(job_id, error = %e, "[Daemon] Failed to send transcode result to main loop (channel closed)");
                        }
                    });
                }
            }

            _ => {
                let resp = self.handle_request(&req);
                self.send_ipc_response(client_id, &resp);
            }
        }
    }

    /// Performs one iteration of Wayland event dispatch, player polling, IPC handling,
    /// and background transcode job completion.
    pub fn step(&mut self) -> Result<()> {
        // 0. Poll completed background transcode jobs
        while let Ok(msg) = self.transcode_rx.try_recv() {
            let Some(job) = self.job_manager.get_job(msg.job_id).cloned() else {
                continue;
            };

            match msg.result {
                Ok(cached_path) => {
                    let mut any_success = false;
                    let mut all_stale = true;

                    // Apply wallpaper for each subscriber to its specific target output and generation.
                    // This guarantees that concurrent requests for different outputs sharing the same
                    // in-flight conversion job are each applied to their respective output correctly.
                    for sub in &job.subscribers {
                        let apply_res = self.outputs.set_video(
                            sub.target_output.as_deref(),
                            &cached_path,
                            Some(sub.generation),
                        );
                        match apply_res {
                            Ok(set_video_result) => {
                                any_success = true;
                                all_stale = false;
                                tracing::info!(
                                    operation = "job_subscriber_applied",
                                    job_id = job.id,
                                    request_id = sub.request_id,
                                    generation = sub.generation,
                                    target = ?sub.target_output,
                                    cached_path = ?cached_path,
                                    "[Daemon] Wallpaper applied successfully for subscriber"
                                );
                                self.save_output_wallpaper_state(
                                    sub.target_output.as_deref(),
                                    &cached_path,
                                );
                                if let Some(client_id) = sub.client_id {
                                    let data = serde_json::to_value(&set_video_result).ok();
                                    let resp = ResponseEnvelope::success(sub.request_id, data);
                                    self.send_ipc_response(client_id, &resp);
                                }
                            }
                            Err(FluffyError::StaleGeneration { current, requested }) => {
                                let sid = crate::benchmark::session_id();
                                let err_msg = format!(
                                    "Stale request generation: {requested} < current {current}"
                                );
                                tracing::warn!(
                                    event = "stale_request_rejected",
                                    operation = "job_subscriber_stale",
                                    job_id = job.id,
                                    request_id = sub.request_id,
                                    generation = sub.generation,
                                    output = ?sub.target_output,
                                    session_id = %sid,
                                    reason = %err_msg,
                                    "[Daemon] Subscriber request was superseded by a newer generation"
                                );
                                if let Some(client_id) = sub.client_id {
                                    let resp = ResponseEnvelope::failure(sub.request_id, err_msg);
                                    self.send_ipc_response(client_id, &resp);
                                }
                            }
                            Err(FluffyError::Ipc(ref err_msg))
                                if err_msg.starts_with("Stale request generation") =>
                            {
                                let sid = crate::benchmark::session_id();
                                tracing::warn!(
                                    event = "stale_request_rejected",
                                    operation = "job_subscriber_stale",
                                    job_id = job.id,
                                    request_id = sub.request_id,
                                    generation = sub.generation,
                                    output = ?sub.target_output,
                                    session_id = %sid,
                                    reason = %err_msg,
                                    "[Daemon] Subscriber request was superseded by a newer generation"
                                );
                                if let Some(client_id) = sub.client_id {
                                    let resp =
                                        ResponseEnvelope::failure(sub.request_id, err_msg.clone());
                                    self.send_ipc_response(client_id, &resp);
                                }
                            }
                            Err(e) => {
                                all_stale = false;
                                tracing::error!(
                                    operation = "job_subscriber_failed",
                                    job_id = job.id,
                                    request_id = sub.request_id,
                                    generation = sub.generation,
                                    target = ?sub.target_output,
                                    error = %e,
                                    "[Daemon] Subscriber failed to apply wallpaper"
                                );
                                if let Some(client_id) = sub.client_id {
                                    let resp =
                                        ResponseEnvelope::failure(sub.request_id, e.to_string());
                                    self.send_ipc_response(client_id, &resp);
                                }
                            }
                        }
                    }

                    if any_success {
                        self.job_manager.complete_job(job.id);
                    } else if all_stale {
                        self.job_manager.mark_stale(job.id);
                    } else {
                        self.job_manager
                            .fail_job(job.id, "All outputs failed to apply wallpaper".to_string());
                    }
                }
                Err(e) => {
                    self.job_manager.fail_job(job.id, e.to_string());
                    tracing::error!(
                        operation = "job_transcode_failed",
                        job_id = job.id,
                        error = %e,
                        "[Daemon] Background video normalization failed"
                    );
                    for sub in &job.subscribers {
                        if let Some(client_id) = sub.client_id {
                            let resp = ResponseEnvelope::failure(
                                sub.request_id,
                                format!("Video normalization failed: {e}"),
                            );
                            self.send_ipc_response(client_id, &resp);
                        }
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

        let sid = crate::benchmark::session_id();
        let pid = std::process::id();
        tracing::info!(
            event = "daemon_shutdown",
            session_id = %sid,
            pid = pid,
            "[Daemon] Shutdown signal detected. Performing clean teardown..."
        );
        self.teardown();
        tracing::info!("[Daemon] Daemon teardown completed. Clean shutdown.");
        Ok(())
    }

    /// Performs clean teardown of all players and layer surfaces, restoring desktop wallpaper.
    pub fn teardown(&mut self) {
        self.outputs.teardown_all(&mut self.wayland_ctx);
    }
}
