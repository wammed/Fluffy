pub mod error;
pub mod playback;
pub mod wayland;

use std::{
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use smithay_client_toolkit::shell::wlr_layer::Layer;

use crate::{
    error::Result,
    playback::{GstVideoPlayer, VideoPlayer},
    wayland::{WallpaperSurface, WaylandContext},
};

fn main() -> Result<()> {
    println!("=== Fluffy Video Wallpaper Manager (Phase 2 Playback Core) ===");

    let args: Vec<String> = env::args().collect();
    let requested_output_name = args.get(1).filter(|a| !a.starts_with("--"));
    let continuous_loop = args.iter().any(|a| a == "--loop");

    // 1. Initialize Wayland Context
    println!("[Main] Initializing Wayland connection...");
    let mut wayland_ctx = WaylandContext::init()?;

    let outputs = wayland_ctx.outputs();
    println!("[Main] Discovered Wayland outputs:");
    for (name, _) in &outputs {
        println!("  - {name}");
    }

    // Select target output
    let target_output = if let Some(req_name) = requested_output_name {
        wayland_ctx.find_output(req_name)
    } else {
        outputs.first().map(|(_, o)| o.clone())
    };

    let target_output_name = target_output.as_ref().and_then(|o| {
        wayland_ctx
            .state
            .output_state
            .info(o)
            .and_then(|i| i.name)
    });
    println!("[Main] Selected target output: {:?}", target_output_name);

    // 2. Create Wallpaper Surface on Layer::Bottom (non-destructive overlay)
    println!("[Main] Creating wallpaper surface on Layer::Bottom...");
    let mut surface = WallpaperSurface::new(&mut wayland_ctx, target_output.as_ref(), Layer::Bottom)?;
    println!(
        "[Main] Wallpaper surface initialized (raw ptr: 0x{:x}, geometry: {}x{})",
        surface.raw_surface_ptr, surface.width, surface.height
    );

    // 3. Initialize GStreamer Video Player
    println!("[Main] Initializing GStreamer playback core...");
    let mut player = GstVideoPlayer::new(
        wayland_ctx.raw_display_ptr(),
        surface.raw_surface_ptr,
        surface.width,
        surface.height,
    )?;

    // 4. Handle graceful termination (Ctrl+C)
    let exit_flag = Arc::new(AtomicBool::new(false));
    {
        let exit_flag = exit_flag.clone();
        ctrlc::set_handler(move || {
            println!("\n[Main] Received shutdown signal (Ctrl+C)...");
            exit_flag.store(true, Ordering::SeqCst);
        })
        .ok();
    }

    let video1 = PathBuf::from("test.mp4");
    let video2 = PathBuf::from("test2.mp4");

    if continuous_loop {
        // Continuous loop mode
        println!("[Main] Starting continuous loop mode with {:?}...", video1);
        player.play(&video1)?;

        while !exit_flag.load(Ordering::SeqCst) {
            wayland_ctx.dispatch_pending()?;
            if !player.poll_events()? {
                eprintln!("[Main] Playback failed");
                break;
            }
        }
    } else {
        // Phase 2 Lifecycle Verification Scenario:
        // Play video1 (4s) -> Pause (2s) -> Resume (3s) -> Switch to video2 (4s) -> Done
        println!("[Main] Starting Phase 2 Playback Lifecycle Test Scenario:");
        println!("       Step 1: Play 'test.mp4' (4 seconds)");
        println!("       Step 2: Pause playback (2 seconds)");
        println!("       Step 3: Resume playback (3 seconds)");
        println!("       Step 4: Switch video to 'test2.mp4' (4 seconds)");
        println!("       Step 5: Clean teardown");

        // Step 1: Play video1
        player.play(&video1)?;
        let mut phase = 1;
        let mut phase_start = Instant::now();

        while !exit_flag.load(Ordering::SeqCst) {
            wayland_ctx.dispatch_pending()?;
            if !player.poll_events()? {
                break;
            }

            match phase {
                1 if phase_start.elapsed().as_secs() >= 4 => {
                    println!("\n[Scenario] Step 2: Testing PAUSE...");
                    player.pause()?;
                    phase = 2;
                    phase_start = Instant::now();
                }
                2 if phase_start.elapsed().as_secs() >= 2 => {
                    println!("\n[Scenario] Step 3: Testing RESUME...");
                    player.resume()?;
                    phase = 3;
                    phase_start = Instant::now();
                }
                3 if phase_start.elapsed().as_secs() >= 3 => {
                    println!("\n[Scenario] Step 4: Testing VIDEO SWITCH to 'test2.mp4'...");
                    player.play(&video2)?;
                    phase = 4;
                    phase_start = Instant::now();
                }
                4 if phase_start.elapsed().as_secs() >= 4 => {
                    println!("\n[Scenario] Step 5: Scenario completed successfully!");
                    break;
                }
                _ => {}
            }
        }
    }

    // 5. Clean teardown
    println!("[Main] Stopping playback...");
    player.stop()?;

    println!("[Main] Destroying Wayland surface and restoring desktop wallpaper...");
    surface.destroy(&mut wayland_ctx);

    println!("[Main] Fluffy exited gracefully.");
    Ok(())
}
