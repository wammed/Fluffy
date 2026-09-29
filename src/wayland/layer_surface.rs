use smithay_client_toolkit::{
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::slot::SlotPool,
};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    protocol::{wl_output, wl_shm},
};

use super::connection::{WaylandContext, WaylandState};
use crate::error::{FluffyError, Result};

pub struct WallpaperSurface {
    pub layer_surface: Option<LayerSurface>,
    pub raw_surface_ptr: usize,
    pub width: u32,
    pub height: u32,
    pool: SlotPool,
}

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        tracing::warn!(
            operation = "layer_closed",
            "[Wayland] Layer surface closed by compositor"
        );
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (w, h) = configure.new_size;
        tracing::debug!(
            operation = "layer_configure",
            width = w,
            height = h,
            "[Wayland] Compositor configured layer size"
        );
    }
}

impl WallpaperSurface {
    pub fn new(
        ctx: &mut WaylandContext,
        target_output: Option<&wl_output::WlOutput>,
        layer: Layer,
    ) -> Result<Self> {
        let surface = ctx.state.compositor_state.create_surface(&ctx.qh);
        let raw_surface_ptr = surface.id().as_ptr() as usize;

        let layer_surface = ctx.state.layer_shell.create_layer_surface(
            &ctx.qh,
            surface,
            layer,
            Some("fluffy-video-wallpaper"),
            target_output,
        );

        layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.commit();

        let pool = SlotPool::new(3840 * 2160 * 4, &ctx.state.shm)
            .map_err(|e| FluffyError::Wayland(format!("Failed to create SHM slot pool: {e}")))?;

        let (width, height) = target_output
            .and_then(|o| ctx.state.output_state.info(o))
            .and_then(|info| {
                info.logical_size
                    .map(|(w, h)| (w as u32, h as u32))
                    .or_else(|| {
                        info.modes
                            .iter()
                            .find(|m| m.current)
                            .map(|m| (m.dimensions.0 as u32, m.dimensions.1 as u32))
                    })
            })
            .unwrap_or((2560, 1440));

        let mut instance = Self {
            layer_surface: Some(layer_surface),
            raw_surface_ptr,
            width,
            height,
            pool,
        };

        // Roundtrip to receive initial configure
        ctx.event_queue.roundtrip(&mut ctx.state)?;

        // Map initial base buffer
        instance.attach_initial_base_buffer()?;
        ctx.event_queue.roundtrip(&mut ctx.state)?;

        Ok(instance)
    }

    pub fn attach_initial_base_buffer(&mut self) -> Result<()> {
        let Some(layer_surface) = self.layer_surface.as_ref() else {
            return Ok(());
        };

        let width = self.width;
        let height = self.height;
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|e| FluffyError::Wayland(format!("Failed to create shm buffer: {e}")))?;

        // Fully transparent base (0x00000000) so underlying desktop wallpaper remains
        // visible without turning into an annoying black screen during transcoding/preroll.
        for chunk in canvas.as_chunks_mut::<4>().0 {
            chunk[0] = 0x00; // Blue
            chunk[1] = 0x00; // Green
            chunk[2] = 0x00; // Red
            chunk[3] = 0x00; // Alpha = 0 (transparent)
        }

        layer_surface
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(layer_surface.wl_surface())
            .map_err(|e| FluffyError::Wayland(format!("Buffer attach failed: {e}")))?;
        layer_surface.commit();

        Ok(())
    }

    pub fn attach_black_base_buffer(&mut self) -> Result<()> {
        let Some(layer_surface) = self.layer_surface.as_ref() else {
            return Ok(());
        };

        let width = self.width;
        let height = self.height;
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|e| FluffyError::Wayland(format!("Failed to create shm buffer: {e}")))?;

        // Opaque black base (0xFF000000)
        for chunk in canvas.as_chunks_mut::<4>().0 {
            chunk[0] = 0x00; // Blue
            chunk[1] = 0x00; // Green
            chunk[2] = 0x00; // Red
            chunk[3] = 0xFF; // Alpha = fully opaque
        }

        layer_surface
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(layer_surface.wl_surface())
            .map_err(|e| FluffyError::Wayland(format!("Buffer attach failed: {e}")))?;
        layer_surface.commit();

        Ok(())
    }

    pub fn update_geometry(
        &mut self,
        ctx: &mut WaylandContext,
        new_width: u32,
        new_height: u32,
    ) -> Result<bool> {
        if self.width == new_width && self.height == new_height {
            return Ok(false);
        }

        self.width = new_width;
        self.height = new_height;

        self.attach_initial_base_buffer()?;
        if let Err(e) = ctx.conn.flush() {
            tracing::warn!(operation = "layer_geometry_flush", error = %e, "[Wayland] Failed to flush connection during geometry update");
        }
        Ok(true)
    }

    pub fn destroy(&mut self, ctx: &mut WaylandContext) {
        if let Some(layer) = self.layer_surface.take() {
            layer.wl_surface().attach(None, 0, 0);
            layer.commit();
            drop(layer);
        }
        if let Err(e) = ctx.event_queue.roundtrip(&mut ctx.state) {
            tracing::debug!(operation = "layer_destroy_roundtrip", error = %e, "[Wayland] Roundtrip error during layer surface destruction");
        }
        if let Err(e) = ctx.conn.flush() {
            tracing::debug!(operation = "layer_destroy_flush", error = %e, "[Wayland] Flush error during layer surface destruction");
        }
    }
}
