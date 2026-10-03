use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1::{self, ZcosmicToplevelHandleV1},
    zcosmic_toplevel_info_v1::{self, ZcosmicToplevelInfoV1},
};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::LayerShell,
    shm::Shm,
};
use wayland_client::{
    Connection, EventQueue, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_surface},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

use crate::error::{FluffyError, Result};

pub struct WaylandContext {
    pub conn: Connection,
    pub event_queue: EventQueue<WaylandState>,
    pub qh: QueueHandle<WaylandState>,
    pub state: WaylandState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputGeometry {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub logical_position: (i32, i32),
    pub description: Option<String>,
}

use std::collections::{HashMap, HashSet};

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
    pub cosmic_toplevel_info: Option<ZcosmicToplevelInfoV1>,
    pub foreign_toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
    pub fullscreen_toplevels: HashSet<wayland_client::backend::ObjectId>,
    pub pending_toplevel_states: HashMap<wayland_client::backend::ObjectId, bool>,
    pub fullscreen_events: Vec<bool>,
    pub supports_fullscreen_detection: bool,
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
        self.output_events
            .push(WaylandOutputEvent::AddedOrUpdated(output));
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        tracing::debug!("[Wayland] update_output event received");
        self.output_events
            .push(WaylandOutputEvent::AddedOrUpdated(output));
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        tracing::info!("[Wayland] output_destroyed event received");
        self.output_events
            .push(WaylandOutputEvent::Destroyed(output));
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

impl wayland_client::Dispatch<ZwlrForeignToplevelManagerV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel: _ } => {
                // New toplevel handle created, queue handles tracking via ZwlrForeignToplevelHandleV1 dispatch
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                tracing::info!("[Wayland] foreign toplevel manager finished");
                state.foreign_toplevel_manager = None;
                if state.cosmic_toplevel_info.is_none() {
                    state.supports_fullscreen_detection = false;
                }
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(
        WaylandState,
        ZwlrForeignToplevelManagerV1,
        [
            zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
        ]
    );
}

impl wayland_client::Dispatch<ZwlrForeignToplevelHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::State { state: state_bytes } => {
                let is_fullscreen = state_bytes.as_chunks::<4>().0.iter().any(|chunk| {
                    let val = u32::from_ne_bytes(*chunk);
                    val == zwlr_foreign_toplevel_handle_v1::State::Fullscreen as u32
                });
                state
                    .pending_toplevel_states
                    .insert(proxy.id(), is_fullscreen);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                let was_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                if let Some(&is_fullscreen) = state.pending_toplevel_states.get(&proxy.id()) {
                    if is_fullscreen {
                        state.fullscreen_toplevels.insert(proxy.id());
                    } else {
                        state.fullscreen_toplevels.remove(&proxy.id());
                    }
                }
                let is_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                if was_any_fullscreen != is_any_fullscreen {
                    state.fullscreen_events.push(is_any_fullscreen);
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                let was_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                state.pending_toplevel_states.remove(&proxy.id());
                state.fullscreen_toplevels.remove(&proxy.id());
                let is_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                if was_any_fullscreen != is_any_fullscreen {
                    state.fullscreen_events.push(is_any_fullscreen);
                }
            }
            _ => {}
        }
    }
}

impl wayland_client::Dispatch<ZcosmicToplevelInfoV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZcosmicToplevelInfoV1,
        event: zcosmic_toplevel_info_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zcosmic_toplevel_info_v1::Event::Toplevel { toplevel: _ } => {
                // New toplevel handle created, queue handles tracking via ZcosmicToplevelHandleV1 dispatch
            }
            zcosmic_toplevel_info_v1::Event::Finished => {
                tracing::info!("[Wayland] COSMIC toplevel info finished");
                state.cosmic_toplevel_info = None;
                if state.foreign_toplevel_manager.is_none() {
                    state.supports_fullscreen_detection = false;
                }
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(
        WaylandState,
        ZcosmicToplevelInfoV1,
        [
            zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (ZcosmicToplevelHandleV1, ()),
        ]
    );
}

impl wayland_client::Dispatch<ZcosmicToplevelHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zcosmic_toplevel_handle_v1::Event::State { state: state_bytes } => {
                const COSMIC_STATE_FULLSCREEN: u32 = 3;
                let is_fullscreen = state_bytes.as_chunks::<4>().0.iter().any(|chunk| {
                    let val = u32::from_ne_bytes(*chunk);
                    val == COSMIC_STATE_FULLSCREEN
                });
                state
                    .pending_toplevel_states
                    .insert(proxy.id(), is_fullscreen);
            }
            zcosmic_toplevel_handle_v1::Event::Done => {
                let was_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                if let Some(&is_fullscreen) = state.pending_toplevel_states.get(&proxy.id()) {
                    if is_fullscreen {
                        state.fullscreen_toplevels.insert(proxy.id());
                    } else {
                        state.fullscreen_toplevels.remove(&proxy.id());
                    }
                }
                let is_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                if was_any_fullscreen != is_any_fullscreen {
                    state.fullscreen_events.push(is_any_fullscreen);
                }
            }
            zcosmic_toplevel_handle_v1::Event::Closed => {
                let was_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                state.pending_toplevel_states.remove(&proxy.id());
                state.fullscreen_toplevels.remove(&proxy.id());
                let is_any_fullscreen = !state.fullscreen_toplevels.is_empty();
                if was_any_fullscreen != is_any_fullscreen {
                    state.fullscreen_events.push(is_any_fullscreen);
                }
            }
            _ => {}
        }
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

        // Pre-check if compositor advertises COSMIC or wlroots foreign-toplevel protocol
        let has_cosmic_toplevel = globals.contents().with_list(|list| {
            list.iter()
                .any(|g| g.interface == "zcosmic_toplevel_info_v1")
        });
        let has_wlr_toplevel = globals.contents().with_list(|list| {
            list.iter()
                .any(|g| g.interface == "zwlr_foreign_toplevel_manager_v1")
        });

        let (cosmic_toplevel_info, foreign_toplevel_manager) = if has_cosmic_toplevel {
            tracing::info!(
                operation = "toplevel_detect",
                "[Wayland] Compositor supports COSMIC toplevel protocol: fullscreen detection available"
            );
            (
                registry_state
                    .bind_one::<ZcosmicToplevelInfoV1, _, _>(&qh, 1..=3, ())
                    .ok(),
                None,
            )
        } else if has_wlr_toplevel {
            tracing::info!(
                operation = "toplevel_detect",
                "[Wayland] Compositor supports wlroots foreign-toplevel protocol: fullscreen detection available"
            );
            (
                None,
                registry_state
                    .bind_one::<ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ())
                    .ok(),
            )
        } else {
            tracing::warn!(
                operation = "toplevel_detect",
                "[Wayland] Compositor does NOT advertise COSMIC or wlroots toplevel protocol: auto-pause on fullscreen unavailable"
            );
            (None, None)
        };
        let supports_fullscreen_detection =
            cosmic_toplevel_info.is_some() || foreign_toplevel_manager.is_some();

        let mut state = WaylandState {
            registry_state,
            output_state,
            compositor_state,
            layer_shell,
            shm,
            output_events: Vec::new(),
            cosmic_toplevel_info,
            foreign_toplevel_manager,
            fullscreen_toplevels: HashSet::new(),
            pending_toplevel_states: HashMap::new(),
            fullscreen_events: Vec::new(),
            supports_fullscreen_detection,
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

    pub fn output_geometry(&self, output: &wl_output::WlOutput) -> Option<OutputGeometry> {
        let info = self.state.output_state.info(output)?;
        let name = info.name.clone().unwrap_or_else(|| "unknown".into());
        let (width, height) = info
            .logical_size
            .map(|(w, h)| (w.max(1) as u32, h.max(1) as u32))
            .or_else(|| {
                info.modes
                    .iter()
                    .find(|m| m.current)
                    .map(|m| (m.dimensions.0.max(1) as u32, m.dimensions.1.max(1) as u32))
            })
            .unwrap_or((2560, 1440));
        let scale = info.scale_factor;
        let logical_position = info.location;
        let description = info.description.clone();

        Some(OutputGeometry {
            name,
            width,
            height,
            scale,
            logical_position,
            description,
        })
    }

    pub fn take_output_events(&mut self) -> Vec<WaylandOutputEvent> {
        std::mem::take(&mut self.state.output_events)
    }

    pub fn dispatch_pending(&mut self) -> Result<()> {
        // 1. Try reading any new events from the Wayland socket without blocking
        if let Some(guard) = self.conn.prepare_read()
            && let Err(e) = guard.read()
        {
            let is_would_block = match &e {
                wayland_client::backend::WaylandError::Io(io_err) => {
                    io_err.kind() == std::io::ErrorKind::WouldBlock
                }
                _ => false,
            };
            if !is_would_block {
                tracing::warn!(error = %e, "[Wayland] Error reading events from Wayland socket");
            }
        }

        // 2. Dispatch pending events in the queue
        if let Err(e) = self.event_queue.dispatch_pending(&mut self.state) {
            tracing::warn!(error = %e, "[Wayland] Error dispatching pending events");
        }

        // 3. Flush any pending requests to compositor
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "[Wayland] Error flushing connection during dispatch_pending");
        }
        Ok(())
    }

    /// Whether the connected compositor advertises and supports the foreign-toplevel protocol
    /// used for automatically pausing playback when windows are fullscreen.
    pub fn supports_fullscreen_detection(&self) -> bool {
        self.state.supports_fullscreen_detection
    }

    /// Takes any queued fullscreen state changes (true = fullscreen active, false = fullscreen cleared).
    pub fn take_fullscreen_events(&mut self) -> Vec<bool> {
        std::mem::take(&mut self.state.fullscreen_events)
    }
}
