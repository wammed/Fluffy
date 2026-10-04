use clap::{Parser, Subcommand};
use fluffy::{
    cache::CacheManager,
    config::{DaemonState, FluffyConfig},
    daemon::WallpaperDaemon,
    error::{FluffyError, Result},
    ipc::{IpcClient, default_socket_path},
};

use std::{
    env,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Parser, Debug)]
#[command(
    name = "fluffy",
    author,
    version,
    about = "Fluffy Video Wallpaper Manager for COSMIC Desktop / Wayland",
    long_about = None
)]
pub struct Cli {
    /// Unix socket path (default: $XDG_RUNTIME_DIR/fluffy.sock)
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to a video file to play immediately (shortcut for daemon with video)
    #[arg(value_name = "VIDEO_PATH")]
    pub direct_video: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the wallpaper daemon (default command)
    #[command(alias = "run")]
    Daemon {
        /// Bind to a specific Wayland output (e.g. DP-1)
        #[arg(long)]
        output: Option<String>,

        /// Start playback immediately with specified video
        #[arg(long)]
        video: Option<PathBuf>,
    },

    /// Query daemon and output status via IPC
    Status,

    /// Change video wallpaper via IPC (validates & normalizes to storage)
    #[command(alias = "set_video")]
    SetVideo {
        /// Path to the video file
        path: PathBuf,

        /// Target specific output (default: all outputs)
        #[arg(long)]
        output: Option<String>,

        /// Generation number for video switch
        #[arg(long)]
        generation: Option<u64>,

        /// IPC response timeout in seconds (default: 60)
        #[arg(long, default_value = "60")]
        timeout: u64,
    },

    /// Validate and import video into storage without playing
    Import {
        /// Path to the video file
        path: PathBuf,

        /// Optional duration in milliseconds to crossfade head and tail for seamless loop
        #[arg(long)]
        crossfade_ms: Option<u32>,
    },

    /// Pause video playback
    Pause {
        /// Target specific output (default: all outputs)
        #[arg(long)]
        output: Option<String>,
    },

    /// Resume video playback
    Resume {
        /// Target specific output (default: all outputs)
        #[arg(long)]
        output: Option<String>,
    },

    /// Stop video playback
    Stop {
        /// Target specific output (default: all outputs)
        #[arg(long)]
        output: Option<String>,
    },

    /// Reload current wallpaper video
    Reload {
        /// Target specific output (default: all outputs)
        #[arg(long)]
        output: Option<String>,
    },

    /// Record a benchmark workload marker event via IPC
    Mark {
        /// Label for benchmark marker
        label: String,
    },

    /// Benchmark subcommands
    Bench {
        #[command(subcommand)]
        subcommand: BenchCommands,
    },

    /// Show or update startup and wallpaper settings
    Config {
        /// Restore last wallpaper on daemon startup (true/false)
        #[arg(long)]
        restore_on_startup: Option<bool>,

        /// Enable/disable daemon autostart on login via systemd (true/false)
        #[arg(long)]
        autostart: Option<bool>,

        /// Configure pause on fullscreen windows (true/false)
        #[arg(long)]
        pause_fullscreen: Option<bool>,

        /// Configure loop crossfade duration in milliseconds (0 to disable)
        #[arg(long)]
        loop_crossfade_ms: Option<u32>,
    },
}

#[derive(Subcommand, Debug)]
pub enum BenchCommands {
    /// Record a benchmark mark
    Mark { label: String },
}

fn init_logging() {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("fluffy=info,gstreamer=warn"));

    let fmt_layer =
        tracing_subscriber::fmt::layer().with_timer(fluffy::benchmark::Iso8601LocalTime);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let socket = cli.socket.unwrap_or_else(default_socket_path);

    match cli.command {
        Some(Commands::Daemon { output, video }) => {
            cmd_daemon(&socket, output.as_deref(), video.as_deref())
        }
        Some(Commands::Status) => cmd_status(&socket),
        Some(Commands::SetVideo {
            path,
            output,
            generation,
            timeout,
        }) => cmd_set_video(
            &socket,
            &path,
            output.as_deref(),
            generation,
            Duration::from_secs(timeout),
        ),
        Some(Commands::Import { path, crossfade_ms }) => cmd_import(&path, crossfade_ms),
        Some(Commands::Pause { output }) => cmd_pause(&socket, output.as_deref()),
        Some(Commands::Resume { output }) => cmd_resume(&socket, output.as_deref()),
        Some(Commands::Stop { output }) => cmd_stop(&socket, output.as_deref()),
        Some(Commands::Reload { output }) => cmd_reload(&socket, output.as_deref()),
        Some(Commands::Mark { label }) => cmd_mark(&socket, &label),
        Some(Commands::Bench { subcommand }) => match subcommand {
            BenchCommands::Mark { label } => cmd_mark(&socket, &label),
        },
        Some(Commands::Config {
            restore_on_startup,
            autostart,
            pause_fullscreen,
            loop_crossfade_ms,
        }) => cmd_config(
            restore_on_startup,
            autostart,
            pause_fullscreen,
            loop_crossfade_ms,
        ),
        None => {
            // Default: run daemon (with direct_video if given)
            cmd_daemon(&socket, None, cli.direct_video.as_deref())
        }
    }
}

fn cmd_daemon(
    socket_path: &Path,
    requested_output: Option<&str>,
    initial_video: Option<&Path>,
) -> Result<()> {
    init_logging();
    tracing::info!("=== Fluffy Video Wallpaper Manager Daemon ===");

    let exit_flag = Arc::new(AtomicBool::new(false));
    {
        let exit_flag = exit_flag.clone();
        ctrlc::set_handler(move || {
            tracing::info!("[Main] Shutdown signal (Ctrl+C / SIGTERM) received...");
            exit_flag.store(true, Ordering::SeqCst);
        })
        .ok();
    }

    let mut daemon = WallpaperDaemon::new(socket_path, requested_output, exit_flag)?;

    // If an initial video was specified, start playing it cleanly without going through IPC envelopes
    if let Some(video) = initial_video {
        tracing::info!("[Main] Starting initial playback: {:?}", video);
        if let Err(e) = daemon.set_initial_video(video) {
            tracing::error!("[Main] Initial video playback failed: {:?}", e);
        }
    }

    // Run the main daemon loop
    daemon.run()
}

fn cmd_status(socket: &Path) -> Result<()> {
    let client = IpcClient::new(socket);
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

fn cmd_set_video(
    socket: &Path,
    path: &Path,
    output: Option<&str>,
    generation: Option<u64>,
    timeout: Duration,
) -> Result<()> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
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
        "         (If normalization is required, daemon converts in background without interrupting current playback; waiting for completion...)"
    );
    let client = IpcClient::with_timeout(socket, timeout);
    client.set_video(&abs_path, output, generation)?;

    println!("[Fluffy] Wallpaper successfully applied!");
    Ok(())
}

fn cmd_pause(socket: &Path, output: Option<&str>) -> Result<()> {
    let client = IpcClient::new(socket);
    client.pause(output)?;
    println!("Playback paused.");
    Ok(())
}

fn cmd_resume(socket: &Path, output: Option<&str>) -> Result<()> {
    let client = IpcClient::new(socket);
    client.resume(output)?;
    println!("Playback resumed.");
    Ok(())
}

fn cmd_stop(socket: &Path, output: Option<&str>) -> Result<()> {
    let client = IpcClient::new(socket);
    client.stop(output)?;
    println!("Playback stopped.");
    Ok(())
}

fn cmd_import(path: &Path, crossfade_ms: Option<u32>) -> Result<()> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };

    if !abs_path.exists() {
        return Err(FluffyError::Cache(format!(
            "Video file does not exist: {:?}",
            abs_path
        )));
    }

    let config = FluffyConfig::load();
    let effective_crossfade = crossfade_ms.or((config.startup_and_wallpaper.loop_crossfade_ms > 0)
        .then_some(config.startup_and_wallpaper.loop_crossfade_ms));

    println!(
        "[Import] Validating and importing to persistent storage (crossfade: {:?}): {:?}",
        effective_crossfade.map(|ms| format!("{ms}ms")),
        abs_path
    );
    let cache = CacheManager::new(CacheManager::default_storage_dir())?;
    let cached_path = match effective_crossfade {
        Some(ms) => cache.import_video_with_crossfade(&abs_path, ms)?,
        None => cache.import_video(&abs_path)?,
    };
    println!(
        "[Import] Successfully stored in persistent storage: {:?}",
        cached_path
    );

    Ok(())
}

fn cmd_reload(socket: &Path, output: Option<&str>) -> Result<()> {
    let client = IpcClient::new(socket);
    client.reload(output)?;
    if let Some(target) = output {
        println!("Reloaded wallpaper on output '{target}'.");
    } else {
        println!("Reloaded wallpaper on all outputs.");
    }
    Ok(())
}

fn cmd_mark(socket: &Path, label: &str) -> Result<()> {
    let client = IpcClient::new(socket);
    client.mark(label)?;
    println!("[Benchmark] Mark recorded: {}", label);
    Ok(())
}

fn cmd_config(
    restore_on_startup: Option<bool>,
    autostart: Option<bool>,
    pause_fullscreen: Option<bool>,
    loop_crossfade_ms: Option<u32>,
) -> Result<()> {
    let mut config = FluffyConfig::load();
    let mut modified = false;

    if let Some(val) = restore_on_startup {
        config.startup_and_wallpaper.restore_on_startup = val;
        modified = true;
    }

    if let Some(val) = autostart {
        config.startup_and_wallpaper.autostart_daemon = val;
        let arg = if val { "enable" } else { "disable" };
        match std::process::Command::new("systemctl")
            .args(["--user", arg, "fluffy.service"])
            .output()
        {
            Ok(out) if out.status.success() => {
                println!("[Fluffy] systemd fluffy.service {arg}d successfully.");
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                eprintln!("[Fluffy] Warning: Failed to {arg} fluffy.service via systemctl: {err}");
            }
            Err(e) => {
                eprintln!("[Fluffy] Warning: Failed to execute systemctl: {e}");
            }
        }
        modified = true;
    }

    if let Some(val) = pause_fullscreen {
        config.startup_and_wallpaper.pause_on_fullscreen = val;
        modified = true;
    }

    if let Some(val) = loop_crossfade_ms {
        config.startup_and_wallpaper.loop_crossfade_ms = val;
        modified = true;
    }

    if modified {
        config.save()?;
        println!("[Fluffy] Configuration updated successfully.");
    }

    let path = FluffyConfig::default_config_path();
    println!("Fluffy Configuration (Path: {:?}):", path);
    println!("  [startup_and_wallpaper]");
    println!(
        "    restore_on_startup:  {} (Restore wallpaper video on daemon launch/login)",
        config.startup_and_wallpaper.restore_on_startup
    );
    println!(
        "    autostart_daemon:    {} (Autostart fluffy.service on user login)",
        config.startup_and_wallpaper.autostart_daemon
    );
    println!(
        "    pause_on_fullscreen: {} (Pause video when window is fullscreen)",
        config.startup_and_wallpaper.pause_on_fullscreen
    );
    println!(
        "    loop_crossfade_ms:   {} ms (Crossfade head/tail for seamless looping, 0 to disable)",
        config.startup_and_wallpaper.loop_crossfade_ms
    );

    let state = DaemonState::load();
    println!(
        "\nSaved Wallpaper State (Path: {:?}):",
        DaemonState::default_state_path()
    );
    if state.outputs.is_empty() {
        println!("  (No saved wallpaper state)");
    } else {
        for (out, vid) in &state.outputs {
            println!("  - {}: {}", out, vid);
        }
    }

    Ok(())
}
