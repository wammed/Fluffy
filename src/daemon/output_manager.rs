use std::{
    collections::HashMap,
    path::Path,
};
use smithay_client_toolkit::shell::wlr_layer::Layer;
use wayland_client::protocol::wl_output;

use crate::{
    error::{FluffyError, Result},
    playback::{GstVideoPlayer, VideoPlayer},
    wayland::{WallpaperSurface, WaylandContext},
};

pub struct ManagedOutput {
    pub name: String,
    pub output: wl_output::WlOutput,
    pub surface: WallpaperSurface,
    pub player: GstVideoPlayer,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
}

pub struct OutputManager {
    outputs: HashMap<String, ManagedOutput>,
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            outputs: HashMap::new(),
        }
    }

    /// Initializes a managed wallpaper surface and video player for a single Wayland output.
    pub fn init_output(
        &mut self,
        ctx: &mut WaylandContext,
        name: String,
        wl_out: wl_output::WlOutput,
    ) -> Result<()> {
        tracing::info!("[OutputManager] Initializing output '{name}'...");
        let surface = WallpaperSurface::new(ctx, Some(&wl_out), Layer::Bottom)?;
        let width = surface.width;
        let height = surface.height;

        let player = GstVideoPlayer::new(
            ctx.raw_display_ptr(),
            surface.raw_surface_ptr,
            width,
            height,
        )?;

        self.outputs.insert(
            name.clone(),
            ManagedOutput {
                name,
                output: wl_out,
                surface,
                player,
                generation: 0,
                width,
                height,
            },
        );

        Ok(())
    }

    pub fn remove_output_by_wl(
        &mut self,
        ctx: &mut WaylandContext,
        wl_out: &wl_output::WlOutput,
    ) -> Option<String> {
        let name_to_remove = self
            .outputs
            .iter()
            .find(|(_, out)| &out.output == wl_out)
            .map(|(name, _)| name.clone());

        if let Some(name) = name_to_remove {
            if let Some(mut out) = self.outputs.remove(&name) {
                tracing::info!("[OutputManager] Output '{name}' disconnected. Tearing down playback & surface...");
                let _ = out.player.stop();
                out.surface.destroy(ctx);
                return Some(name);
            }
        }
        None
    }

    /// Finds any currently playing video across managed outputs to use as default for newly attached displays.
    pub fn default_active_video(&self) -> Option<std::path::PathBuf> {
        self.outputs
            .values()
            .find_map(|out| out.player.current_video().map(|p| p.to_path_buf()))
    }

    pub fn len(&self) -> usize {
        self.outputs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.outputs.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&ManagedOutput> {
        self.outputs.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ManagedOutput> {
        self.outputs.get_mut(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ManagedOutput)> {
        self.outputs.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut ManagedOutput)> {
        self.outputs.iter_mut()
    }

    /// Sets video wallpaper on either a specific output or all managed outputs.
    pub fn set_video(
        &mut self,
        target_output: Option<&str>,
        video_path: &Path,
        req_generation: Option<u64>,
    ) -> Result<()> {
        if let Some(target) = target_output {
            let out = self
                .outputs
                .get_mut(target)
                .ok_or_else(|| FluffyError::OutputNotFound(target.to_string()))?;

            if let Some(target_gen) = req_generation {
                if target_gen < out.generation {
                    return Err(FluffyError::Ipc(format!(
                        "Stale request generation: {target_gen} < current {}",
                        out.generation
                    )));
                }
                out.generation = target_gen;
            } else {
                out.generation += 1;
            }

            tracing::info!(
                "[OutputManager] Setting video for output '{}' (gen: {}): {:?}",
                target, out.generation, video_path
            );
            out.player.play(video_path)?;
        } else {
            // Apply to all managed outputs
            for (name, out) in self.outputs.iter_mut() {
                if let Some(target_gen) = req_generation {
                    if target_gen < out.generation {
                        return Err(FluffyError::Ipc(format!(
                            "Stale request generation: {target_gen} < current {}",
                            out.generation
                        )));
                    }
                    out.generation = target_gen;
                } else {
                    out.generation += 1;
                }

                tracing::info!(
                    "[OutputManager] Setting video for output '{}' (gen: {}): {:?}",
                    name, out.generation, video_path
                );
                out.player.play(video_path)?;
            }
        }

        Ok(())
    }

    pub fn pause(&mut self, target_output: Option<&str>) -> Result<()> {
        if let Some(target) = target_output {
            let out = self
                .outputs
                .get_mut(target)
                .ok_or_else(|| FluffyError::OutputNotFound(target.to_string()))?;
            out.player.pause()?;
        } else {
            for out in self.outputs.values_mut() {
                out.player.pause()?;
            }
        }
        Ok(())
    }

    pub fn resume(&mut self, target_output: Option<&str>) -> Result<()> {
        if let Some(target) = target_output {
            let out = self
                .outputs
                .get_mut(target)
                .ok_or_else(|| FluffyError::OutputNotFound(target.to_string()))?;
            out.player.resume()?;
        } else {
            for out in self.outputs.values_mut() {
                out.player.resume()?;
            }
        }
        Ok(())
    }

    pub fn stop(&mut self, target_output: Option<&str>) -> Result<()> {
        if let Some(target) = target_output {
            let out = self
                .outputs
                .get_mut(target)
                .ok_or_else(|| FluffyError::OutputNotFound(target.to_string()))?;
            out.player.stop()?;
        } else {
            for out in self.outputs.values_mut() {
                out.player.stop()?;
            }
        }
        Ok(())
    }

    pub fn reload(&mut self, target_output: Option<&str>) -> Result<()> {
        if let Some(target) = target_output {
            let out = self
                .outputs
                .get_mut(target)
                .ok_or_else(|| FluffyError::OutputNotFound(target.to_string()))?;
            if let Some(curr) = out.player.current_video().map(|p| p.to_path_buf()) {
                out.player.play(&curr)?;
            }
        } else {
            for out in self.outputs.values_mut() {
                if let Some(curr) = out.player.current_video().map(|p| p.to_path_buf()) {
                    out.player.play(&curr)?;
                }
            }
        }
        Ok(())
    }

    /// Polls GStreamer events for all managed outputs.
    pub fn poll_events(&mut self) -> Result<()> {
        for (name, out) in self.outputs.iter_mut() {
            if !out.player.poll_events()? {
                tracing::error!("[OutputManager] Output '{}' playback encountered fatal error", name);
            }
        }
        Ok(())
    }

    /// Stops players and destroys surfaces for all outputs cleanly.
    pub fn teardown_all(&mut self, ctx: &mut WaylandContext) {
        for (name, mut out) in self.outputs.drain() {
            tracing::info!("[OutputManager] Stopping player on output '{}'...", name);
            let _ = out.player.stop();
            tracing::info!("[OutputManager] Destroying surface on output '{}'...", name);
            out.surface.destroy(ctx);
        }
        tracing::info!("[OutputManager] Teardown complete. All desktop wallpapers restored.");
    }
}
