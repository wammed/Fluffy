use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
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
        IpcServer,
    },
    playback::VideoPlayer,
    wayland::WaylandContext,
};
use super::output_manager::OutputManager;

pub struct WallpaperDaemon {
    pub wayland_ctx: WaylandContext,
    pub outputs: OutputManager,
    pub cache: CacheManager,
    pub ipc_server: IpcServer,
    pub exit_flag: Arc<AtomicBool>,
}

impl WallpaperDaemon {
    /// Initializes the wallpaper daemon on all detected outputs (or a specifically requested output),
    /// binding the IPC server to the provided socket path.
    pub fn new<P: AsRef<Path>>(
        socket_path: P,
        requested_output: Option<&str>,
        exit_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let cache = CacheManager::new(CacheManager::default_cache_dir())?;
        let mut wayland_ctx = WaylandContext::init()?;

        let outputs_info = wayland_ctx.outputs();
        if outputs_info.is_empty() {
            return Err(FluffyError::Wayland("No Wayland outputs detected".to_string()));
        }

        println!("[Daemon] Discovered Wayland outputs:");
        for (name, _) in &outputs_info {
            println!("  - {name}");
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

        println!(
            "[Daemon] Initialized {} active output(s) concurrently",
            output_manager.len()
        );

        let ipc_server = IpcServer::bind(socket_path)?;

        Ok(Self {
            wayland_ctx,
            outputs: output_manager,
            cache,
            ipc_server,
            exit_flag,
        })
    }

    /// Handles a single IPC request envelope and produces an IPC response.
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
                // Sort by name for deterministic order
                output_statuses.sort_by(|a, b| a.name.cmp(&b.name));

                let status = DaemonStatus {
                    daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                    outputs: output_statuses,
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

                // If specific output was requested, verify it exists
                if let Some(ref target_name) = req.output {
                    if !self.outputs.contains(target_name) {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Output '{}' not managed by daemon", target_name),
                        );
                    }
                }

                // Import, validate (4K policy) and normalize into cache
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

    /// Performs one iteration of Wayland event dispatch, player polling, and IPC handling.
    pub fn step(&mut self) -> Result<()> {
        // 1. Dispatch pending Wayland compositor events
        self.wayland_ctx.dispatch_pending()?;

        // 2. Poll GStreamer bus events for all managed outputs
        self.outputs.poll_events()?;

        // 3. Poll and process IPC requests
        let pending_requests = self.ipc_server.poll_requests()?;
        for pending in pending_requests {
            let response = self.handle_request(&pending.request);
            let _ = self.ipc_server.respond(pending.client_id, &response);
        }

        Ok(())
    }

    /// Runs the daemon main loop until a shutdown signal is received.
    pub fn run(&mut self) -> Result<()> {
        println!("[Daemon] Daemon main loop started. Ready for IPC commands.");

        while !self.exit_flag.load(Ordering::SeqCst) {
            self.step()?;
            // Sleep briefly to prevent busy-waiting when idle
            thread::sleep(Duration::from_millis(5));
        }

        println!("[Daemon] Shutdown signal detected. Performing clean teardown...");
        self.teardown();
        Ok(())
    }

    /// Performs clean teardown of all players and layer surfaces, restoring desktop wallpaper.
    pub fn teardown(&mut self) {
        self.outputs.teardown_all(&mut self.wayland_ctx);
    }
}
