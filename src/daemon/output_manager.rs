use smithay_client_toolkit::shell::wlr_layer::Layer;
use std::{collections::HashMap, path::Path};
use wayland_client::protocol::wl_output;

use crate::{
    error::{FluffyError, Result},
    ipc::protocol::{OutputApplyResult, SetVideoResult},
    playback::{GstVideoPlayer, VideoPlayer},
    wayland::{OutputGeometry, WallpaperSurface, WaylandContext},
};

#[derive(Debug, Clone)]
pub struct GenerationTracker {
    next_generation: u64,
    applied_generations: HashMap<String, u64>,
}

impl Default for GenerationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GenerationTracker {
    pub fn new() -> Self {
        Self {
            next_generation: 1,
            applied_generations: HashMap::new(),
        }
    }

    pub fn register_output(&mut self, name: &str) {
        self.applied_generations
            .entry(name.to_string())
            .or_insert(0);
    }

    pub fn unregister_output(&mut self, name: &str) {
        self.applied_generations.remove(name);
    }

    pub fn current_generation(&self, name: &str) -> Option<u64> {
        self.applied_generations.get(name).copied()
    }

    /// Allocates a monotonic generation number at request arrival time.
    /// If `explicit_generation` is given (e.g. from CLI --generation), validates that
    /// it is not already stale and advances the monotonic counter past it.
    pub fn allocate(
        &mut self,
        target_output: Option<&str>,
        explicit_generation: Option<u64>,
    ) -> Result<u64> {
        if let Some(explicit) = explicit_generation {
            if let Some(target) = target_output {
                if let Some(&curr) = self
                    .applied_generations
                    .get(target)
                    .filter(|&&c| explicit < c)
                {
                    return Err(FluffyError::StaleGeneration {
                        current: curr,
                        requested: explicit,
                    });
                }
            } else {
                for &curr in self.applied_generations.values() {
                    if explicit < curr {
                        return Err(FluffyError::StaleGeneration {
                            current: curr,
                            requested: explicit,
                        });
                    }
                }
            }

            if explicit >= self.next_generation {
                self.next_generation = explicit.saturating_add(1);
            }
            Ok(explicit)
        } else {
            let allocated = self.next_generation;
            self.next_generation = self.next_generation.saturating_add(1);
            Ok(allocated)
        }
    }

    /// Validates whether a completed job's generation is fresh enough to be applied.
    /// If any target output is already at a strictly higher generation, returns
    /// `Err(FluffyError::Ipc("Stale request generation..."))` and discards application.
    pub fn validate_and_apply(
        &mut self,
        target_output: Option<&str>,
        job_generation: u64,
    ) -> Result<()> {
        let sid = crate::benchmark::session_id();
        if let Some(target) = target_output {
            if let Some(&curr) = self
                .applied_generations
                .get(target)
                .filter(|&&c| job_generation < c)
            {
                tracing::warn!(
                    event = "stale_request_rejected",
                    output = %target,
                    generation = job_generation,
                    current_generation = curr,
                    session_id = %sid,
                    "[GenerationTracker] Stale request rejected: job_gen < current_gen"
                );
                return Err(FluffyError::StaleGeneration {
                    current: curr,
                    requested: job_generation,
                });
            }
            self.applied_generations
                .insert(target.to_string(), job_generation);
        } else {
            // Validate all outputs atomically before making any state mutations
            for (_name, &curr) in &self.applied_generations {
                if job_generation < curr {
                    tracing::warn!(
                        event = "stale_request_rejected",
                        output = %_name,
                        generation = job_generation,
                        current_generation = curr,
                        session_id = %sid,
                        "[GenerationTracker] Stale request rejected on output: job_gen < current_gen"
                    );
                    return Err(FluffyError::StaleGeneration {
                        current: curr,
                        requested: job_generation,
                    });
                }
            }
            for curr in self.applied_generations.values_mut() {
                *curr = job_generation;
            }
        }
        Ok(())
    }
}

pub struct ManagedOutput {
    pub name: String,
    pub output: wl_output::WlOutput,
    pub surface: WallpaperSurface,
    pub player: GstVideoPlayer,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub logical_position: (i32, i32),
    pub description: Option<String>,
}

pub struct OutputManager {
    outputs: HashMap<String, ManagedOutput>,
    generation_tracker: GenerationTracker,
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            outputs: HashMap::new(),
            generation_tracker: GenerationTracker::new(),
        }
    }

    /// Allocates a monotonic request generation at request arrival time.
    pub fn allocate_generation(
        &mut self,
        target_output: Option<&str>,
        explicit_generation: Option<u64>,
    ) -> Result<u64> {
        self.generation_tracker
            .allocate(target_output, explicit_generation)
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

        let player = GstVideoPlayer::new_with_name(
            &name,
            ctx.raw_display_ptr(),
            surface.raw_surface_ptr,
            width,
            height,
        )?;

        let geom = ctx.output_geometry(&wl_out);
        let scale = geom.as_ref().map(|g| g.scale).unwrap_or(1);
        let logical_position = geom.as_ref().map(|g| g.logical_position).unwrap_or((0, 0));
        let description = geom.and_then(|g| g.description);

        self.generation_tracker.register_output(&name);

        let sid = crate::benchmark::session_id();
        tracing::info!(
            event = "output_added",
            output = %name,
            width,
            height,
            scale,
            session_id = %sid,
            "[OutputManager] Output added and initialized"
        );

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
                scale,
                logical_position,
                description,
            },
        );

        Ok(())
    }

    /// Updates existing output geometry (resolution, scale, position, description) if changed.
    /// Reconfigures WallpaperSurface backing buffer and GStreamer video overlay rendering rectangle.
    pub fn update_output_geometry(
        &mut self,
        ctx: &mut WaylandContext,
        name: &str,
        new_geom: OutputGeometry,
    ) -> Result<bool> {
        let Some(out) = self.outputs.get_mut(name) else {
            return Ok(false);
        };

        let size_changed = out.width != new_geom.width || out.height != new_geom.height;
        let scale_changed = out.scale != new_geom.scale;
        let pos_changed = out.logical_position != new_geom.logical_position;
        let desc_changed = out.description != new_geom.description;

        if !size_changed && !scale_changed && !pos_changed && !desc_changed {
            return Ok(false);
        }

        tracing::info!(
            "[OutputManager] Output '{}' geometry changed: {}x{} (scale {}, pos {:?}) -> {}x{} (scale {}, pos {:?})",
            name,
            out.width,
            out.height,
            out.scale,
            out.logical_position,
            new_geom.width,
            new_geom.height,
            new_geom.scale,
            new_geom.logical_position
        );

        out.scale = new_geom.scale;
        out.logical_position = new_geom.logical_position;
        out.description = new_geom.description;

        if size_changed {
            out.width = new_geom.width;
            out.height = new_geom.height;

            // Update surface backing buffer
            out.surface
                .update_geometry(ctx, new_geom.width, new_geom.height)?;

            // Update video player overlay rendering rectangle
            out.player
                .update_geometry(new_geom.width, new_geom.height)?;
        }

        let sid = crate::benchmark::session_id();
        tracing::info!(
            event = "output_configured",
            output = %name,
            width = new_geom.width,
            height = new_geom.height,
            scale = new_geom.scale,
            session_id = %sid,
            "[OutputManager] Output geometry configured"
        );

        Ok(true)
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

        if let Some(name) = name_to_remove
            && let Some(mut out) = self.outputs.remove(&name)
        {
            let sid = crate::benchmark::session_id();
            tracing::info!(
                event = "output_removed",
                output = %name,
                session_id = %sid,
                "[OutputManager] Output removed and cleaned up"
            );
            self.generation_tracker.unregister_output(&name);
            if let Err(e) = out.player.stop() {
                tracing::warn!(output = %name, error = %e, "[OutputManager] Error stopping player during output removal");
            }
            out.surface.destroy(ctx);
            return Some(name);
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

    /// Sets video wallpaper on either a specific output or all managed outputs using best-effort semantics.
    /// Performs atomic stale generation verification using GenerationTracker before
    /// reconfiguring any playback pipeline.
    pub fn set_video(
        &mut self,
        target_output: Option<&str>,
        video_path: &Path,
        req_generation: Option<u64>,
    ) -> Result<SetVideoResult> {
        let target_gen = match req_generation {
            Some(g) => g,
            None => self.generation_tracker.allocate(target_output, None)?,
        };

        // 1. Validate staleness and update generation tracking atomically
        self.generation_tracker
            .validate_and_apply(target_output, target_gen)?;

        let video_id = crate::benchmark::safe_video_id(video_path);
        let sid = crate::benchmark::session_id();
        let mut apply_results = Vec::new();

        // 2. Apply wallpaper transition to hardware surface(s) with best-effort semantics
        if let Some(target) = target_output {
            let out = self
                .outputs
                .get_mut(target)
                .ok_or_else(|| FluffyError::OutputNotFound(target.to_string()))?;

            let old_gen = out.generation;
            out.generation = target_gen;
            tracing::info!(
                event = "video_switch_requested",
                output = %target,
                generation = target_gen,
                video_id = %video_id,
                session_id = %sid,
                "[OutputManager] Video switch requested"
            );
            match out
                .player
                .play_with_generation(video_path, target_gen, old_gen)
            {
                Ok(()) => {
                    apply_results.push(OutputApplyResult {
                        name: target.to_string(),
                        success: true,
                        error: None,
                    });
                }
                Err(e) => {
                    apply_results.push(OutputApplyResult {
                        name: target.to_string(),
                        success: false,
                        error: Some(e.to_string()),
                    });
                    return Err(e);
                }
            }
        } else {
            let mut any_success = false;
            for (name, out) in self.outputs.iter_mut() {
                let old_gen = out.generation;
                out.generation = target_gen;
                tracing::info!(
                    event = "video_switch_requested",
                    output = %name,
                    generation = target_gen,
                    video_id = %video_id,
                    session_id = %sid,
                    "[OutputManager] Video switch requested"
                );
                match out
                    .player
                    .play_with_generation(video_path, target_gen, old_gen)
                {
                    Ok(()) => {
                        any_success = true;
                        apply_results.push(OutputApplyResult {
                            name: name.clone(),
                            success: true,
                            error: None,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[OutputManager] Failed to set video on output '{}': {e}",
                            name
                        );
                        apply_results.push(OutputApplyResult {
                            name: name.clone(),
                            success: false,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }

            // If there were managed outputs and ALL of them failed, return error
            if !self.outputs.is_empty() && !any_success {
                let err_details = apply_results
                    .iter()
                    .filter_map(|r| r.error.as_deref())
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(FluffyError::Playback(format!(
                    "All outputs failed to play video: {err_details}"
                )));
            }
        }

        Ok(SetVideoResult {
            generation: target_gen,
            video_path: video_path.to_path_buf(),
            outputs: apply_results,
        })
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
                tracing::error!(
                    "[OutputManager] Output '{}' playback encountered fatal error",
                    name
                );
            }
        }
        Ok(())
    }

    /// Stops players and destroys surfaces for all outputs cleanly.
    pub fn teardown_all(&mut self, ctx: &mut WaylandContext) {
        for (name, mut out) in self.outputs.drain() {
            tracing::info!(output = %name, "[OutputManager] Stopping player on output...");
            if let Err(e) = out.player.stop() {
                tracing::warn!(output = %name, error = %e, "[OutputManager] Error stopping player during daemon teardown");
            }
            tracing::info!(output = %name, "[OutputManager] Destroying surface on output...");
            out.surface.destroy(ctx);
        }
        tracing::info!("[OutputManager] Teardown complete. All desktop wallpapers restored.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_tracker_a_b_normal_order() {
        let mut tracker = GenerationTracker::new();
        tracker.register_output("DP-1");

        // Request A received -> generation 1 allocated
        let gen_a = tracker.allocate(Some("DP-1"), None).unwrap();
        assert_eq!(gen_a, 1);

        // Request B received -> generation 2 allocated
        let gen_b = tracker.allocate(Some("DP-1"), None).unwrap();
        assert_eq!(gen_b, 2);

        // Job A completes and applies
        assert!(tracker.validate_and_apply(Some("DP-1"), gen_a).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(1));

        // Job B completes and applies
        assert!(tracker.validate_and_apply(Some("DP-1"), gen_b).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(2));
    }

    #[test]
    fn test_generation_tracker_a_b_reverse_completion_order() {
        let mut tracker = GenerationTracker::new();
        tracker.register_output("DP-1");

        // Request A received (takes long to transcode)
        let gen_a = tracker.allocate(Some("DP-1"), None).unwrap();
        assert_eq!(gen_a, 1);

        // Request B received (faster / cached)
        let gen_b = tracker.allocate(Some("DP-1"), None).unwrap();
        assert_eq!(gen_b, 2);

        // Job B finishes FIRST and applies
        assert!(tracker.validate_and_apply(Some("DP-1"), gen_b).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(2));

        // Job A finishes LATER -> must be rejected as stale!
        let stale_res = tracker.validate_and_apply(Some("DP-1"), gen_a);
        assert!(stale_res.is_err());
        assert!(
            stale_res
                .unwrap_err()
                .to_string()
                .contains("Stale request generation: 1 < current 2")
        );

        // Current output generation remains B (2)
        assert_eq!(tracker.current_generation("DP-1"), Some(2));
    }

    #[test]
    fn test_generation_tracker_a_b_c_completion_orders() {
        // Scenario 1: B finished -> C finished -> A finished (Final must be C)
        {
            let mut tracker = GenerationTracker::new();
            tracker.register_output("DP-1");

            let gen_a = tracker.allocate(Some("DP-1"), None).unwrap();
            let gen_b = tracker.allocate(Some("DP-1"), None).unwrap();
            let gen_c = tracker.allocate(Some("DP-1"), None).unwrap();
            assert_eq!((gen_a, gen_b, gen_c), (1, 2, 3));

            // B finishes
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_b).is_ok());
            assert_eq!(tracker.current_generation("DP-1"), Some(2));

            // C finishes
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_c).is_ok());
            assert_eq!(tracker.current_generation("DP-1"), Some(3));

            // A finishes -> stale, rejected
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_a).is_err());
            assert_eq!(tracker.current_generation("DP-1"), Some(3));
        }

        // Scenario 2: A finished -> C finished -> B finished (Final must be C)
        {
            let mut tracker = GenerationTracker::new();
            tracker.register_output("DP-1");

            let gen_a = tracker.allocate(Some("DP-1"), None).unwrap();
            let gen_b = tracker.allocate(Some("DP-1"), None).unwrap();
            let gen_c = tracker.allocate(Some("DP-1"), None).unwrap();

            // A finishes
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_a).is_ok());
            assert_eq!(tracker.current_generation("DP-1"), Some(1));

            // C finishes
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_c).is_ok());
            assert_eq!(tracker.current_generation("DP-1"), Some(3));

            // B finishes -> stale (2 < 3), rejected
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_b).is_err());
            assert_eq!(tracker.current_generation("DP-1"), Some(3));
        }

        // Scenario 3: C finished -> B finished -> A finished (Final must be C)
        {
            let mut tracker = GenerationTracker::new();
            tracker.register_output("DP-1");

            let gen_a = tracker.allocate(Some("DP-1"), None).unwrap();
            let gen_b = tracker.allocate(Some("DP-1"), None).unwrap();
            let gen_c = tracker.allocate(Some("DP-1"), None).unwrap();

            // C finishes
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_c).is_ok());
            assert_eq!(tracker.current_generation("DP-1"), Some(3));

            // B finishes -> stale, rejected
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_b).is_err());
            // A finishes -> stale, rejected
            assert!(tracker.validate_and_apply(Some("DP-1"), gen_a).is_err());

            assert_eq!(tracker.current_generation("DP-1"), Some(3));
        }
    }

    #[test]
    fn test_generation_tracker_same_generation_allowed() {
        let mut tracker = GenerationTracker::new();
        tracker.register_output("DP-1");

        let g = tracker.allocate(Some("DP-1"), None).unwrap();
        assert!(tracker.validate_and_apply(Some("DP-1"), g).is_ok());
        // Same generation can be re-applied (e.g. reload)
        assert!(tracker.validate_and_apply(Some("DP-1"), g).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(g));
    }

    #[test]
    fn test_generation_tracker_explicit_cli_generation() {
        let mut tracker = GenerationTracker::new();
        tracker.register_output("DP-1");

        // Explicit generation from CLI --generation 100
        let g = tracker.allocate(Some("DP-1"), Some(100)).unwrap();
        assert_eq!(g, 100);
        assert!(tracker.validate_and_apply(Some("DP-1"), 100).is_ok());

        // An older explicit request (< 100) must be rejected at allocation time
        let stale_alloc = tracker.allocate(Some("DP-1"), Some(99));
        assert!(stale_alloc.is_err());

        // Subsequent automatic allocation advances monotonically past 100
        let next_auto = tracker.allocate(Some("DP-1"), None).unwrap();
        assert_eq!(next_auto, 101);
    }

    #[test]
    fn test_generation_tracker_multi_output_consistency() {
        let mut tracker = GenerationTracker::new();
        tracker.register_output("DP-1");
        tracker.register_output("HDMI-A-1");

        // DP-1 advances to generation 5
        assert!(tracker.validate_and_apply(Some("DP-1"), 5).is_ok());
        // HDMI-A-1 is still at generation 0
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(0));

        // Applying generation 4 to ALL outputs (None) must fail atomically
        // because DP-1 is already at generation 5 (4 < 5)
        let multi_res = tracker.validate_and_apply(None, 4);
        assert!(multi_res.is_err());
        assert!(
            multi_res
                .unwrap_err()
                .to_string()
                .contains("Stale request generation")
        );

        // Neither output state is corrupted
        assert_eq!(tracker.current_generation("DP-1"), Some(5));
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(0));

        // Applying generation 6 to ALL outputs succeeds
        assert!(tracker.validate_and_apply(None, 6).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(6));
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(6));
    }

    #[test]
    fn test_output_geometry_diff_and_no_op() {
        let initial = OutputGeometry {
            name: "DP-1".to_string(),
            width: 2560,
            height: 1440,
            scale: 1,
            logical_position: (0, 0),
            description: Some("Dell Inc. 27-inch".to_string()),
        };

        // 1. Same geometry -> no changes
        assert_eq!(initial, initial.clone());

        // 2. Existing output geometry update: 2560x1440 -> 3840x2160
        let mut geom_resized = initial.clone();
        geom_resized.width = 3840;
        geom_resized.height = 2160;
        assert_ne!(initial, geom_resized);
        assert_eq!((geom_resized.width, geom_resized.height), (3840, 2160));

        // 3. Scale update: 1 -> 2
        let mut geom_scaled = geom_resized.clone();
        geom_scaled.scale = 2;
        assert_ne!(geom_resized, geom_scaled);
        assert_eq!(geom_scaled.scale, 2);

        // 4. Logical position update: (0, 0) -> (2560, 0)
        let mut geom_moved = geom_scaled.clone();
        geom_moved.logical_position = (2560, 0);
        assert_ne!(geom_scaled, geom_moved);
        assert_eq!(geom_moved.logical_position, (2560, 0));

        // 5. Connector metadata update
        let mut geom_desc = geom_moved.clone();
        geom_desc.description = Some("Dell Inc. 4K Monitor".to_string());
        assert_ne!(geom_moved, geom_desc);
    }

    #[test]
    fn test_output_tracking_add_remove_multiple() {
        let mut tracker = GenerationTracker::new();

        // 1. Add DP-1
        tracker.register_output("DP-1");
        assert_eq!(tracker.current_generation("DP-1"), Some(0));

        // 2. Add HDMI-A-1
        tracker.register_output("HDMI-A-1");
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(0));

        // 3. Update DP-1 to generation 10
        assert!(tracker.validate_and_apply(Some("DP-1"), 10).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(10));
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(0));

        // 4. Update HDMI-A-1 to generation 12
        assert!(tracker.validate_and_apply(Some("HDMI-A-1"), 12).is_ok());
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(12));

        // 5. Remove DP-1
        tracker.unregister_output("DP-1");
        assert_eq!(tracker.current_generation("DP-1"), None);
        // HDMI-A-1 remains unaffected
        assert_eq!(tracker.current_generation("HDMI-A-1"), Some(12));
    }

    #[test]
    fn test_all_output_best_effort_semantics_partial_failure() {
        use std::path::PathBuf;

        // Simulating the result of two outputs: DP-1 succeeds, DP-2 fails
        let apply_results = vec![
            OutputApplyResult {
                name: "DP-1".to_string(),
                success: true,
                error: None,
            },
            OutputApplyResult {
                name: "DP-2".to_string(),
                success: false,
                error: Some("Pipeline error: waylandsink failed to attach subsurface".to_string()),
            },
        ];

        let result = SetVideoResult {
            generation: 42,
            video_path: PathBuf::from("/tmp/video.mp4"),
            outputs: apply_results,
        };

        // Validate best-effort semantics:
        // 1. Exactly 2 outputs reported
        assert_eq!(result.outputs.len(), 2);

        // 2. DP-1 succeeded
        let dp1 = result.outputs.iter().find(|o| o.name == "DP-1").unwrap();
        assert!(dp1.success);
        assert!(dp1.error.is_none());

        // 3. DP-2 failed with specific error, but did not abort DP-1
        let dp2 = result.outputs.iter().find(|o| o.name == "DP-2").unwrap();
        assert!(!dp2.success);
        assert!(dp2.error.as_deref().unwrap().contains("waylandsink failed"));

        // 4. JSON serialization roundtrip for IPC response
        let json_val = serde_json::to_value(&result).unwrap();
        let deserialized: SetVideoResult = serde_json::from_value(json_val).unwrap();
        assert_eq!(result, deserialized);
    }
}
