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

    let switch_loop = args.iter().any(|a| a == "--switch-loop");

    if continuous_loop {
        // Continuous loop mode with single video
        println!("[Main] Starting continuous loop mode with {:?}...", video1);
        player.play(&video1)?;

        while !exit_flag.load(Ordering::SeqCst) {
            wayland_ctx.dispatch_pending()?;
            if !player.poll_events()? {
                eprintln!("[Main] Playback failed");
                break;
            }
        }
    } else if switch_loop {
        // Continuous alternating switch mode (every 3 seconds) for manual visual inspection
        println!("[Main] Starting continuous switch-loop mode (alternating test.mp4 <-> test2.mp4 every 3s)...");
        println!("[Main] Press Ctrl+C anytime to stop and cleanly teardown.");
        player.play(&video1)?;
        let mut current_video = 1;
        let mut last_switch = Instant::now();

        while !exit_flag.load(Ordering::SeqCst) {
            wayland_ctx.dispatch_pending()?;
            if !player.poll_events()? {
                break;
            }

            if last_switch.elapsed().as_secs() >= 3 {
                if current_video == 1 {
                    println!("\n[SwitchLoop] Switching A (test.mp4) -> B (test2.mp4)...");
                    player.play(&video2)?;
                    current_video = 2;
                } else {
                    println!("\n[SwitchLoop] Switching B (test2.mp4) -> A (test.mp4)...");
                    player.play(&video1)?;
                    current_video = 1;
                }
                last_switch = Instant::now();
            }
        }
    } else {
        // Comprehensive Video Switching Verification Scenario:
        // 1. Initial play video A (test.mp4, 3s)
        // 2. Switch A -> B (test2.mp4, 3s) [Normal switch, different content]
        // 3. Switch B -> A (test.mp4, 3s) [Reverse switch]
        // 4. Pause (1.5s) & Resume (2s) verification
        // 5. Rapid continuous switching: A -> B (1.5s) -> A (1.5s) -> B (1.5s) -> A (1.5s)
        // 6. Loop playback verification (3s) -> Clean teardown
        println!("[Main] Starting Comprehensive Video Switching Verification Scenario:");
        println!("       Step 1: Play 'test.mp4' (3s)");
        println!("       Step 2: Normal switch -> 'test2.mp4' (3s)");
        println!("       Step 3: Reverse switch -> 'test.mp4' (3s)");
        println!("       Step 4: Pause (1.5s) & Resume (2s)");
        println!("       Step 5: Rapid continuous switches (A -> B -> A -> B -> A)");
        println!("       Step 6: Completion & Clean teardown");

        // Step 1: Play video A
        player.play(&video1)?;
        let mut step = 1;
        let mut step_start = Instant::now();

        while !exit_flag.load(Ordering::SeqCst) {
            wayland_ctx.dispatch_pending()?;
            if !player.poll_events()? {
                break;
            }

            let elapsed_ms = step_start.elapsed().as_millis();

            match step {
                1 if elapsed_ms >= 3000 => {
                    println!("\n[Scenario] Step 2: Normal Switch A (test.mp4) -> B (test2.mp4)...");
                    player.play(&video2)?;
                    step = 2;
                    step_start = Instant::now();
                }
                2 if elapsed_ms >= 3000 => {
                    println!("\n[Scenario] Step 3: Reverse Switch B (test2.mp4) -> A (test.mp4)...");
                    player.play(&video1)?;
                    step = 3;
                    step_start = Instant::now();
                }
                3 if elapsed_ms >= 3000 => {
                    println!("\n[Scenario] Step 4a: Testing PAUSE...");
                    player.pause()?;
                    step = 4;
                    step_start = Instant::now();
                }
                4 if elapsed_ms >= 1500 => {
                    println!("\n[Scenario] Step 4b: Testing RESUME...");
                    player.resume()?;
                    step = 5;
                    step_start = Instant::now();
                }
                5 if elapsed_ms >= 2000 => {
                    println!("\n[Scenario] Step 5a: Rapid Switch -> B (test2.mp4)...");
                    player.play(&video2)?;
                    step = 6;
                    step_start = Instant::now();
                }
                6 if elapsed_ms >= 1500 => {
                    println!("\n[Scenario] Step 5b: Rapid Switch -> A (test.mp4)...");
                    player.play(&video1)?;
                    step = 7;
                    step_start = Instant::now();
                }
                7 if elapsed_ms >= 1500 => {
                    println!("\n[Scenario] Step 5c: Rapid Switch -> B (test2.mp4)...");
                    player.play(&video2)?;
                    step = 8;
                    step_start = Instant::now();
                }
                8 if elapsed_ms >= 1500 => {
                    println!("\n[Scenario] Step 5d: Rapid Switch -> A (test.mp4)...");
                    player.play(&video1)?;
                    step = 9;
                    step_start = Instant::now();
                }
                9 if elapsed_ms >= 3000 => {
                    println!("\n[Scenario] All verification steps completed successfully!");
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
