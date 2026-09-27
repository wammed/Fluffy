use smithay_client_toolkit::{
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::slot::SlotPool,
};
use wayland_client::{
    protocol::{wl_output, wl_shm},
    Connection, Proxy, QueueHandle,
};

use crate::error::{FluffyError, Result};
use super::connection::{WaylandContext, WaylandState};

pub struct WallpaperSurface {
    pub layer_surface: Option<LayerSurface>,
    pub raw_surface_ptr: usize,
    pub width: u32,
    pub height: u32,
    pool: SlotPool,
}

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        println!("[Wayland] Layer surface closed by compositor");
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
        println!("[Wayland] Compositor configured layer size: {w}x{h}");
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

        let mut instance = Self {
            layer_surface: Some(layer_surface),
            raw_surface_ptr,
            width: 2560,
            height: 1440,
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

        // Opaque black base (0xFF000000) so underlying desktop wallpaper never flashes through
        for chunk in canvas.chunks_exact_mut(4) {
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

    pub fn destroy(&mut self, ctx: &mut WaylandContext) {
        if let Some(layer) = self.layer_surface.take() {
            layer.wl_surface().attach(None, 0, 0);
            layer.commit();
            drop(layer);
        }
        let _ = ctx.event_queue.roundtrip(&mut ctx.state);
        let _ = ctx.conn.flush();
    }
}
