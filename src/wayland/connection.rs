use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::LayerShell,
    shm::Shm,
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_surface},
    Connection, EventQueue, QueueHandle,
};

use crate::error::{FluffyError, Result};

pub struct WaylandContext {
    pub conn: Connection,
    pub event_queue: EventQueue<WaylandState>,
    pub qh: QueueHandle<WaylandState>,
    pub state: WaylandState,
}

#[derive(Debug, Clone)]
pub enum WaylandOutputEvent {
    AddedOrUpdated(wl_output::WlOutput),
    Destroyed(wl_output::WlOutput),
}

pub struct WaylandState {
    pub registry_state: RegistryState,
    pub output_state: OutputState,
    pub compositor_state: CompositorState,
    pub layer_shell: LayerShell,
    pub shm: Shm,
    pub output_events: Vec<WaylandOutputEvent>,
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl smithay_client_toolkit::output::OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        tracing::debug!("[Wayland] new_output event received");
        self.output_events.push(WaylandOutputEvent::AddedOrUpdated(output));
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        tracing::debug!("[Wayland] update_output event received");
        self.output_events.push(WaylandOutputEvent::AddedOrUpdated(output));
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        tracing::info!("[Wayland] output_destroyed event received");
        self.output_events.push(WaylandOutputEvent::Destroyed(output));
    }
}

impl smithay_client_toolkit::compositor::CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl smithay_client_toolkit::shm::ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

smithay_client_toolkit::delegate_registry!(WaylandState);
smithay_client_toolkit::delegate_dispatch2!(WaylandState);

impl WaylandContext {
    pub fn init() -> Result<Self> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut event_queue) = registry_queue_init(&conn)?;
        let qh = event_queue.handle();

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);
        let compositor_state = CompositorState::bind(&globals, &qh)
            .map_err(|e| FluffyError::Wayland(format!("Failed to bind wl_compositor: {e}")))?;
        let layer_shell = LayerShell::bind(&globals, &qh)
            .map_err(|e| FluffyError::Wayland(format!("Failed to bind layer_shell: {e}")))?;
        let shm = Shm::bind(&globals, &qh)
            .map_err(|e| FluffyError::Wayland(format!("Failed to bind wl_shm: {e}")))?;

        let mut state = WaylandState {
            registry_state,
            output_state,
            compositor_state,
            layer_shell,
            shm,
            output_events: Vec::new(),
        };

        // Roundtrip to enumerate globals & outputs
        event_queue.roundtrip(&mut state)?;
        // Clear initial enumeration events so only post-startup changes are reported
        state.output_events.clear();

        Ok(Self {
            conn,
            event_queue,
            qh,
            state,
        })
    }

    pub fn outputs(&self) -> Vec<(String, wl_output::WlOutput)> {
        self.state
            .output_state
            .outputs()
            .map(|o| {
                let name = self
                    .state
                    .output_state
                    .info(&o)
                    .and_then(|info| info.name)
                    .unwrap_or_else(|| "unknown".into());
                (name, o)
            })
            .collect()
    }

    pub fn find_output(&self, name: &str) -> Option<wl_output::WlOutput> {
        self.outputs()
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, o)| o)
    }

    pub fn raw_display_ptr(&self) -> *mut std::ffi::c_void {
        self.conn.backend().display_ptr() as *mut std::ffi::c_void
    }

    pub fn output_name(&self, output: &wl_output::WlOutput) -> Option<String> {
        self.state
            .output_state
            .info(output)
            .and_then(|info| info.name)
    }

    pub fn take_output_events(&mut self) -> Vec<WaylandOutputEvent> {
        std::mem::take(&mut self.state.output_events)
    }

    pub fn dispatch_pending(&mut self) -> Result<()> {
        let _ = self.event_queue.dispatch_pending(&mut self.state);
        let _ = self.conn.flush();
        Ok(())
    }
}
