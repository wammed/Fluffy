use std::{
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use gstreamer::glib::translate::FromGlibPtrFull;
use gstreamer::prelude::*;
use gstreamer_video::prelude::*;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, Proxy, QueueHandle,
};

unsafe extern "C" {
    fn gst_wl_display_handle_context_new(
        display: *mut std::ffi::c_void,
    ) -> *mut gstreamer::ffi::GstContext;
}

struct WallpaperApp {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    layer_surface: Option<LayerSurface>,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
    exit: Arc<AtomicBool>,
}

impl ProvidesRegistryState for WallpaperApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl OutputHandler for WallpaperApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl CompositorHandler for WallpaperApp {
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

impl ShmHandler for WallpaperApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl LayerShellHandler for WallpaperApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        println!("[PoC] Layer surface closed by compositor");
        self.exit.store(true, Ordering::SeqCst);
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) = configure.new_size;
        println!("[PoC] Compositor configured layer surface size: {width}x{height}");
        self.width = if width == 0 { 2560 } else { width };
        self.height = if height == 0 { 1440 } else { height };
        self.configured = true;
    }
}

impl WallpaperApp {
    fn attach_initial_frame(&mut self) {
        let Some(layer_surface) = self.layer_surface.as_ref() else {
            return;
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
            .expect("Failed to create initial shm buffer");

        // Transparent background so the video subsurface is directly visible
        for chunk in canvas.chunks_exact_mut(4) {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
            chunk[3] = 0;
        }

        layer_surface
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        buffer
            .attach_to(layer_surface.wl_surface())
            .expect("Buffer attach failed");
        layer_surface.commit();

        println!("[PoC] Initial base frame mapped on layer surface ({width}x{height})");
    }
}

delegate_registry!(WallpaperApp);
smithay_client_toolkit::delegate_dispatch2!(WallpaperApp);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Step 3: GStreamer + Wayland layer-shell PoC ===");

    let requested_output_name = env::args().nth(1);
    let requested_layer = if env::args().any(|a| a == "--bottom") {
        println!("[PoC] Using Layer::Bottom (above cosmic-bg wallpaper)");
        Layer::Bottom
    } else {
        println!("[PoC] Using Layer::Background");
        Layer::Background
    };

    // 1. Initialize GStreamer
    gstreamer::init()?;
    println!("[PoC] GStreamer initialized");

    // 2. Initialize Wayland connection
    let conn = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let output_state = OutputState::new(&globals, &qh);
    let compositor = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let pool = SlotPool::new(2560 * 1440 * 4, &shm)?;

    let exit_flag = Arc::new(AtomicBool::new(false));

    // Handle Ctrl+C gracefully
    {
        let exit_flag = exit_flag.clone();
        ctrlc::set_handler(move || {
            println!("\n[PoC] Received Ctrl+C, exiting gracefully...");
            exit_flag.store(true, Ordering::SeqCst);
        })
        .ok();
    }

    let mut app = WallpaperApp {
        registry_state,
        output_state,
        shm,
        layer_surface: None,
        pool,
        width: 2560,
        height: 1440,
        configured: false,
        exit: exit_flag.clone(),
    };

    // Populate outputs
    event_queue.roundtrip(&mut app)?;

    let outputs: Vec<_> = app.output_state.outputs().collect();
    for (i, output) in outputs.iter().enumerate() {
        let name = app
            .output_state
            .info(output)
            .and_then(|info| info.name)
            .unwrap_or_else(|| "unknown".into());
        println!("  Output #{i}: {name}");
    }

    // Select target output based on CLI arg or default to first
    let target_output = if let Some(req_name) = &requested_output_name {
        outputs.iter().find(|o| {
            app.output_state
                .info(o)
                .and_then(|i| i.name)
                .as_deref()
                == Some(req_name.as_str())
        }).cloned()
    } else {
        outputs.first().cloned()
    };

    let target_name = target_output
        .as_ref()
        .and_then(|o| app.output_state.info(o)?.name.clone());
    println!("[PoC] Target output for video wallpaper: {:?}", target_name);

    let surface = compositor.create_surface(&qh);
    let raw_surface_ptr = surface.id().as_ptr() as usize;
    println!("[PoC] wl_surface raw pointer: 0x{raw_surface_ptr:x}");

    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        requested_layer,
        Some("fluffy_wallpaper_gst_poc"),
        target_output.as_ref(),
    );

    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_exclusive_zone(-1);
    layer.commit();

    app.layer_surface = Some(layer);

    // Initial roundtrip for layer surface configure
    event_queue.roundtrip(&mut app)?;

    // Map parent surface so Compositor will render child subsurface
    app.attach_initial_frame();
    event_queue.roundtrip(&mut app)?;

    // 3. Prepare GStreamer Wayland Context BEFORE any pipeline creation
    let display_ptr = conn.backend().display_ptr() as *mut std::ffi::c_void;
    println!("[PoC] wl_display raw pointer: {display_ptr:?}");
    let gst_wl_context_raw = unsafe { gst_wl_display_handle_context_new(display_ptr) };
    let gst_wl_context: gstreamer::Context =
        unsafe { FromGlibPtrFull::from_glib_full(gst_wl_context_raw) };

    // 4. Create waylandsink and set context immediately
    let sink = gstreamer::ElementFactory::make("waylandsink")
        .name("sink")
        .build()?;
    sink.set_context(&gst_wl_context);

    let overlay = sink
        .clone()
        .dynamic_cast::<gstreamer_video::VideoOverlay>()
        .expect("waylandsink does not implement VideoOverlay");

    // Pre-configure overlay handle & size
    unsafe {
        overlay.set_window_handle(raw_surface_ptr);
    }
    let (w, h) = (app.width as i32, app.height as i32);
    let _ = overlay.set_render_rectangle(0, 0, w, h);
    println!("[PoC] overlay set_window_handle & set_render_rectangle(0, 0, {w}, {h}) configured");

    // 5. Create playbin with the pre-configured sink
    let video_path = PathBuf::from("test.mp4").canonicalize()?;
    println!("[PoC] Target video file: {:?}", video_path);

    let playbin = gstreamer::ElementFactory::make("playbin")
        .property("uri", format!("file://{}", video_path.display()))
        .property("video-sink", &sink)
        .build()?;
    let pipeline = playbin.dynamic_cast::<gstreamer::Pipeline>().unwrap();
    pipeline.set_context(&gst_wl_context);

    let bus = pipeline.bus().expect("Failed to get bus");

    // 6. Start playback
    pipeline.set_state(gstreamer::State::Playing)?;
    println!("[PoC] GStreamer pipeline state -> PLAYING");

    let loop_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

    println!("[PoC] Video playback running in loop. Press Ctrl+C to stop (runs up to 15s)...");
    let start_time = std::time::Instant::now();

    while !exit_flag.load(Ordering::SeqCst) && start_time.elapsed().as_secs() < 15 {
        // Dispatch Wayland events
        let _ = event_queue.dispatch_pending(&mut app);
        let _ = conn.flush();

        // Process GStreamer bus messages with 50ms timeout
        while let Some(msg) = bus.timed_pop(gstreamer::ClockTime::from_mseconds(50)) {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    let count = loop_count.fetch_add(1, Ordering::SeqCst) + 1;
                    println!("[PoC] EOS reached! Looping (count: {count}). Seeking to 0...");
                    let _ = pipeline.seek_simple(
                        gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                        gstreamer::ClockTime::ZERO,
                    );
                }
                MessageView::Error(err) => {
                    eprintln!(
                        "[PoC] GStreamer Error: {} ({:?})",
                        err.error(),
                        err.debug()
                    );
                    exit_flag.store(true, Ordering::SeqCst);
                    break;
                }
                MessageView::NeedContext(msg) => {
                    let ctx_type = msg.context_type();
                    if ctx_type == "GstWaylandDisplayHandleContextType"
                        || ctx_type == "GstWlDisplayHandleContextType"
                    {
                        if let Some(src) = msg.src() {
                            if let Ok(elem) = src.clone().downcast::<gstreamer::Element>() {
                                elem.set_context(&gst_wl_context);
                            }
                        }
                    }
                }
                MessageView::Element(msg) => {
                    if gstreamer_video::is_video_overlay_prepare_window_handle_message(msg) {
                        println!("[PoC] prepare-window-handle: refreshing handle & rect");
                        unsafe {
                            overlay.set_window_handle(raw_surface_ptr);
                        }
                        let _ = overlay.set_render_rectangle(0, 0, w, h);
                    }
                }
                _ => {}
            }
        }
    }

    // 7. Graceful teardown
    println!("[PoC] Stopping pipeline...");
    let _ = pipeline.set_state(gstreamer::State::Null);

    println!("[PoC] Cleaning up Wayland layer surface...");
    if let Some(layer) = app.layer_surface.take() {
        layer.wl_surface().attach(None, 0, 0);
        layer.commit();
        drop(layer);
    }
    let _ = event_queue.roundtrip(&mut app);
    let _ = conn.flush();

    println!("[PoC] Completed gracefully.");
    Ok(())
}
