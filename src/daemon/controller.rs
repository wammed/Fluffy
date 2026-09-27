use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use smithay_client_toolkit::shell::wlr_layer::Layer;

use crate::{
    error::{FluffyError, Result},
    ipc::{
        protocol::{
            CommandType, DaemonStatus, OutputStatus, RequestEnvelope, ResponseEnvelope,
        },
        IpcServer,
    },
    playback::{GstVideoPlayer, VideoPlayer},
    wayland::{WallpaperSurface, WaylandContext},
};

pub struct ManagedOutput {
    pub name: String,
    pub surface: WallpaperSurface,
    pub player: GstVideoPlayer,
    pub generation: u64,
}

pub struct WallpaperDaemon {
    pub wayland_ctx: WaylandContext,
    pub outputs: HashMap<String, ManagedOutput>,
    pub ipc_server: IpcServer,
    pub exit_flag: Arc<AtomicBool>,
}

impl WallpaperDaemon {
    /// Initializes the wallpaper daemon on the specified output (or all outputs),
    /// binding the IPC server to the provided socket path.
    pub fn new<P: AsRef<Path>>(
        socket_path: P,
        requested_output: Option<&str>,
        exit_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let mut wayland_ctx = WaylandContext::init()?;

        let outputs_info = wayland_ctx.outputs();
        if outputs_info.is_empty() {
            return Err(FluffyError::Wayland("No Wayland outputs detected".to_string()));
        }

        println!("[Daemon] Discovered Wayland outputs:");
        for (name, _) in &outputs_info {
            println!("  - {name}");
        }

        let mut managed_outputs = HashMap::new();

        // If a specific output is requested, use it; otherwise initialize the first output
        let target_outputs: Vec<(String, _)> = if let Some(req_name) = requested_output {
            let found = outputs_info
                .into_iter()
                .find(|(name, _)| name == req_name)
                .ok_or_else(|| FluffyError::OutputNotFound(req_name.to_string()))?;
            vec![found]
        } else {
            // Select default primary output
            vec![outputs_info.into_iter().next().unwrap()]
        };

        for (name, wl_out) in target_outputs {
            println!("[Daemon] Initializing wallpaper surface for output '{name}'...");
            let surface = WallpaperSurface::new(&mut wayland_ctx, Some(&wl_out), Layer::Bottom)?;

            let player = GstVideoPlayer::new(
                wayland_ctx.raw_display_ptr(),
                surface.raw_surface_ptr,
                surface.width,
                surface.height,
            )?;

            managed_outputs.insert(
                name.clone(),
                ManagedOutput {
                    name,
                    surface,
                    player,
                    generation: 0,
                },
            );
        }

        let ipc_server = IpcServer::bind(socket_path)?;

        Ok(Self {
            wayland_ctx,
            outputs: managed_outputs,
            ipc_server,
            exit_flag,
        })
    }

    /// Handles a single IPC request envelope and produces an IPC response.
    pub fn handle_request(&mut self, req: &RequestEnvelope) -> ResponseEnvelope {
        match req.command {
            CommandType::Status => {
                let mut output_statuses = Vec::new();
                for (name, out) in &self.outputs {
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
                    if !self.outputs.contains_key(target_name) {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Output '{}' not managed by daemon", target_name),
                        );
                    }
                }

                // Apply to targeted output or all outputs
                for (name, out) in self.outputs.iter_mut() {
                    if let Some(ref target_name) = req.output {
                        if name != target_name {
                            continue;
                        }
                    }

                    // Generation validation (Section 9.3)
                    if let Some(req_gen) = req.generation {
                        if req_gen < out.generation {
                            return ResponseEnvelope::failure(
                                req.request_id,
                                format!(
                                    "Stale request generation: {} < current {}",
                                    req_gen, out.generation
                                ),
                            );
                        }
                        out.generation = req_gen;
                    } else {
                        out.generation += 1;
                    }

                    println!(
                        "[Daemon] Setting video for output '{}' (gen: {}): {:?}",
                        name, out.generation, path
                    );

                    if let Err(e) = out.player.play(path) {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Failed to play video on '{}': {e}", name),
                        );
                    }
                }

                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Pause => {
                for (name, out) in self.outputs.iter_mut() {
                    if let Some(ref target_name) = req.output {
                        if name != target_name {
                            continue;
                        }
                    }
                    if let Err(e) = out.player.pause() {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Failed to pause output '{}': {e}", name),
                        );
                    }
                }
                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Resume => {
                for (name, out) in self.outputs.iter_mut() {
                    if let Some(ref target_name) = req.output {
                        if name != target_name {
                            continue;
                        }
                    }
                    if let Err(e) = out.player.resume() {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Failed to resume output '{}': {e}", name),
                        );
                    }
                }
                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Stop => {
                for (name, out) in self.outputs.iter_mut() {
                    if let Some(ref target_name) = req.output {
                        if name != target_name {
                            continue;
                        }
                    }
                    if let Err(e) = out.player.stop() {
                        return ResponseEnvelope::failure(
                            req.request_id,
                            format!("Failed to stop output '{}': {e}", name),
                        );
                    }
                }
                ResponseEnvelope::success(req.request_id, None)
            }

            CommandType::Reload => {
                for (name, out) in self.outputs.iter_mut() {
                    if let Some(ref target_name) = req.output {
                        if name != target_name {
                            continue;
                        }
                    }
                    if let Some(curr) = out.player.current_video().map(|p| p.to_path_buf()) {
                        if let Err(e) = out.player.play(&curr) {
                            return ResponseEnvelope::failure(
                                req.request_id,
                                format!("Failed to reload output '{}': {e}", name),
                            );
                        }
                    }
                }
                ResponseEnvelope::success(req.request_id, None)
            }
        }
    }

    /// Performs one iteration of Wayland event dispatch, player polling, and IPC handling.
    pub fn step(&mut self) -> Result<()> {
        // 1. Dispatch pending Wayland compositor events
        self.wayland_ctx.dispatch_pending()?;

        // 2. Poll GStreamer bus events for all outputs
        for (name, out) in self.outputs.iter_mut() {
            if !out.player.poll_events()? {
                eprintln!("[Daemon] Output '{}' playback encountered fatal error", name);
            }
        }

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
        for (name, mut out) in self.outputs.drain() {
            println!("[Daemon] Stopping player on output '{}'...", name);
            let _ = out.player.stop();
            println!("[Daemon] Destroying surface on output '{}'...", name);
            out.surface.destroy(&mut self.wayland_ctx);
        }
        println!("[Daemon] Teardown complete. Desktop wallpaper cleanly restored.");
    }
}
