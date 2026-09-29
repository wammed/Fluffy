pub mod cache;
pub mod daemon;
pub mod error;
pub mod ipc;
pub mod playback;
pub mod wayland;

use std::{
    env,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    cache::CacheManager,
    daemon::WallpaperDaemon,
    error::{FluffyError, Result},
    ipc::{IpcClient, default_socket_path},
};

fn print_help() {
    println!(
        r#"Fluffy Video Wallpaper Manager - Phase 4 (Cache & Import)

USAGE:
    fluffy [COMMAND] [OPTIONS]

COMMANDS:
    daemon, run          Run the wallpaper daemon (default if no command given)
    status               Query daemon and output status via IPC
    set-video <PATH>     Change video wallpaper via IPC (validates & normalizes to cache)
    import <PATH>        Validate and import video into cache without playing
    pause                Pause video playback
    resume               Resume video playback
    stop                 Stop video playback
    reload               Reload current wallpaper video
    help, --help         Print this help message

OPTIONS for 'daemon' / 'run':
    --socket <PATH>      Unix socket path (default: $XDG_RUNTIME_DIR/fluffy.sock)
    --output <NAME>      Bind to specific Wayland output (e.g. DP-1)
    --video <PATH>       Start playback immediately with specified video
    --loop               Continuous test loop
    --switch-loop        Continuous alternating video test loop

OPTIONS for IPC client commands:
    --socket <PATH>      Target daemon Unix socket path
    --output <NAME>      Target specific output (default: all outputs)
    --generation <NUM>   Generation number for video switch
"#
    );
}

fn parse_positional_path(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "--socket"
            || args[i] == "--output"
            || args[i] == "--generation"
            || args[i] == "--video")
            && i + 1 < args.len()
        {
            i += 2;
            continue;
        }
        if args[i].starts_with("--") {
            i += 1;
            continue;
        }
        if args[i] == "set-video" || args[i] == "set_video" || args[i] == "import" {
            i += 1;
            continue;
        }
        return Some(args[i].clone());
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let raw_args = &args[1..];

    // Find the command token, skipping any flag parameters
    let mut command = "daemon";
    let mut i = 0;
    while i < raw_args.len() {
        let arg = &raw_args[i];
        if (arg == "--socket" || arg == "--output" || arg == "--video" || arg == "--generation")
            && i + 1 < raw_args.len()
        {
            i += 2;
            continue;
        }
        if arg.starts_with("--") {
            if arg == "--help" || arg == "-h" {
                command = "help";
                break;
            }
            i += 1;
            continue;
        }
        command = arg.as_str();
        break;
    }

    match command {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "status" => cmd_status(raw_args),
        "set-video" | "set_video" => cmd_set_video(raw_args),
        "import" => cmd_import(raw_args),
        "pause" => cmd_pause(raw_args),
        "resume" => cmd_resume(raw_args),
        "stop" => cmd_stop(raw_args),
        "reload" => cmd_reload(raw_args),
        "daemon" | "run" => cmd_daemon(raw_args),
        cmd => {
            // Check if it's a file path meant for daemon initial video, or invalid command
            if Path::new(cmd).exists() {
                cmd_daemon(raw_args)
            } else {
                eprintln!("Unknown command: '{cmd}'. Run 'fluffy help' for usage.");
                std::process::exit(1);
            }
        }
    }
}

fn parse_socket_arg(args: &[String]) -> PathBuf {
    for i in 0..args.len() {
        if args[i] == "--socket" && i + 1 < args.len() {
            return PathBuf::from(&args[i + 1]);
        }
    }
    default_socket_path()
}

fn parse_output_arg(args: &[String]) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == "--output" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

fn parse_generation_arg(args: &[String]) -> Option<u64> {
    for i in 0..args.len() {
        if args[i] == "--generation" && i + 1 < args.len() {
            return args[i + 1].parse().ok();
        }
    }
    None
}

fn init_logging() {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("fluffy=info,gstreamer=warn"));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

fn cmd_daemon(args: &[String]) -> Result<()> {
    init_logging();
    tracing::info!("=== Fluffy Video Wallpaper Manager Daemon (Phase 7 Hardened) ===");

    let socket_path = parse_socket_arg(args);
    let requested_output = parse_output_arg(args);

    let initial_video: Option<PathBuf> = {
        let mut vid = None;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "--video" && i + 1 < args.len() {
                vid = Some(PathBuf::from(&args[i + 1]));
                break;
            } else if (args[i] == "--socket" || args[i] == "--output") && i + 1 < args.len() {
                i += 2;
                continue;
            } else if !args[i].starts_with("--") && Path::new(&args[i]).exists() {
                vid = Some(PathBuf::from(&args[i]));
                break;
            }
            i += 1;
        }
        vid
    };

    let exit_flag = Arc::new(AtomicBool::new(false));
    {
        let exit_flag = exit_flag.clone();
        ctrlc::set_handler(move || {
            tracing::info!("[Main] Shutdown signal (Ctrl+C / SIGTERM) received...");
            exit_flag.store(true, Ordering::SeqCst);
        })
        .ok();
    }

    let mut daemon =
        WallpaperDaemon::new(&socket_path, requested_output.as_deref(), exit_flag.clone())?;

    // If an initial video was specified, start playing it
    if let Some(video) = initial_video {
        tracing::info!("[Main] Starting initial playback: {:?}", video);
        let req =
            crate::ipc::RequestEnvelope::new(0, crate::ipc::CommandType::SetVideo).with_path(video);
        let resp = daemon.handle_request(&req);
        if !resp.success {
            tracing::error!("[Main] Initial video playback failed: {:?}", resp.error);
        }
    }

    // Run the main daemon loop
    daemon.run()
}

fn cmd_status(args: &[String]) -> Result<()> {
    let socket = parse_socket_arg(args);
    let client = IpcClient::new(&socket);
    let status = client.status()?;

    println!("Fluffy Daemon Status (v{})", status.daemon_version);
    println!("Connected Socket: {:?}", socket);
    if status.is_converting {
        if !status.active_jobs.is_empty() {
            println!(
                "Background Tasks: ⚙️  {} active conversion job(s):",
                status.active_jobs.len()
            );
            for job in &status.active_jobs {
                println!(
                    "  - [Job #{}] (gen: {}, state: {}): {}",
                    job.job_id, job.generation, job.state, job.source
                );
            }
        } else {
            println!(
                "Background Task:  ⚙️  Optimizing / Transcoding video: {}",
                status.converting_file.as_deref().unwrap_or("active")
            );
        }
    } else {
        println!("Background Task:  Idle (No active conversion)");
    }
    println!("Outputs ({} total):", status.outputs.len());
    for out in status.outputs {
        println!("  - [{}] State: {}", out.name, out.state);
        println!(
            "      Current Video: {}",
            out.current_video.unwrap_or_else(|| "None".to_string())
        );
        println!("      Generation:    {}", out.generation);
        println!("      Loop Count:    {}", out.loop_count);
    }

    Ok(())
}

fn cmd_set_video(args: &[String]) -> Result<()> {
    let socket = parse_socket_arg(args);
    let output = parse_output_arg(args);
    let generation = parse_generation_arg(args);

    let path_str = parse_positional_path(args)
        .ok_or_else(|| FluffyError::Ipc("Missing video path for 'set-video'".to_string()))?;

    let path = PathBuf::from(path_str);
    let abs_path = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };

    if !abs_path.exists() {
        return Err(FluffyError::Ipc(format!(
            "Video file does not exist: {:?}",
            abs_path
        )));
    }

    println!("[Fluffy] Requesting wallpaper change to: {:?}", abs_path);
    println!(
        "         (If normalization is required, transcoding runs asynchronously in background without interrupting current playback)"
    );
    let client = IpcClient::new(&socket);
    client.set_video(&abs_path, output.as_deref(), generation)?;

    println!("[Fluffy] Wallpaper successfully applied!");
    Ok(())
}

fn cmd_pause(args: &[String]) -> Result<()> {
    let socket = parse_socket_arg(args);
    let output = parse_output_arg(args);
    let client = IpcClient::new(&socket);
    client.pause(output.as_deref())?;
    println!("Playback paused.");
    Ok(())
}

fn cmd_resume(args: &[String]) -> Result<()> {
    let socket = parse_socket_arg(args);
    let output = parse_output_arg(args);
    let client = IpcClient::new(&socket);
    client.resume(output.as_deref())?;
    println!("Playback resumed.");
    Ok(())
}

fn cmd_stop(args: &[String]) -> Result<()> {
    let socket = parse_socket_arg(args);
    let output = parse_output_arg(args);
    let client = IpcClient::new(&socket);
    client.stop(output.as_deref())?;
    println!("Playback stopped.");
    Ok(())
}

fn cmd_import(args: &[String]) -> Result<()> {
    let path_str = parse_positional_path(args)
        .ok_or_else(|| FluffyError::Ipc("Missing video path for 'import'".to_string()))?;

    let path = PathBuf::from(path_str);
    let abs_path = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };

    if !abs_path.exists() {
        return Err(FluffyError::Cache(format!(
            "Video file does not exist: {:?}",
            abs_path
        )));
    }

    println!(
        "[Import] Validating and importing to persistent storage: {:?}",
        abs_path
    );
    let cache = CacheManager::new(CacheManager::default_storage_dir())?;
    let cached_path = cache.import_video(&abs_path)?;
    println!(
        "[Import] Successfully stored in persistent storage: {:?}",
        cached_path
    );

    Ok(())
}

fn cmd_reload(args: &[String]) -> Result<()> {
    let socket = parse_socket_arg(args);
    let client = IpcClient::new(&socket);
    client.reload()?;
    println!("Reloaded wallpaper.");
    Ok(())
}
