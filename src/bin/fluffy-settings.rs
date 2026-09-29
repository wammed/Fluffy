use std::path::{Path, PathBuf};

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::widget::{Space, column, container, row, scrollable, text};
use cosmic::iced::{Alignment, Length, Size};
use cosmic::widget::{button, text_input};
use cosmic::{Application, Element};

use fluffy::ipc::{DaemonStatus, IpcClient, OutputStatus, default_socket_path};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Message {
    RefreshStatus,
    StatusFetched(Result<DaemonStatus, String>),
    StartDaemon,
    DaemonStarted(Result<(), String>),
    SelectOutput(Option<String>), // None = All outputs
    PickFile,
    PathInputChanged(String),
    ApplyWallpaper,
    WallpaperApplied(Result<(), String>),
    TickSpinner,
    TickPoll,
    Pause,
    Resume,
    Stop,
    PlaybackControlCompleted(Result<String, String>),
    DismissMessage,
}

#[derive(Debug, Clone, Copy)]
enum PlaybackControlAction {
    Pause,
    Resume,
    Stop,
}

const ARROW_FRAMES: &[&str] = &["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];

const FLUFFY_ICON_SVG: &[u8] = include_bytes!("../../images/fluffy-icon.svg");

fn app_icon(size: u16) -> cosmic::widget::icon::Icon {
    cosmic::widget::icon::from_svg_bytes(FLUFFY_ICON_SVG)
        .icon()
        .size(size)
}

fn find_fluffy_executable() -> Result<PathBuf, String> {
    // 1. Check ~/.local/bin/fluffy
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".local/bin/fluffy");
        if p.is_file() {
            return Ok(p);
        }
    }

    // 2. Check sibling in same directory as current executable
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        let sibling = parent.join("fluffy");
        if sibling.is_file() {
            return Ok(sibling);
        }
    }

    // 3. Check PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("fluffy");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err("Could not find 'fluffy' binary in ~/.local/bin or PATH".to_string())
}

fn wait_for_daemon_ready(socket_path: &Path, max_attempts: usize) -> bool {
    let client = IpcClient::with_timeout(socket_path, Duration::from_millis(500));
    for _ in 0..max_attempts {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if client.status().is_ok() {
            return true;
        }
    }
    false
}

fn launch_daemon(socket_path: &Path) -> Result<(), String> {
    // 1. Try systemctl --user start fluffy.service first (standard COSMIC integration)
    if let Ok(output) = std::process::Command::new("systemctl")
        .args(["--user", "start", "fluffy.service"])
        .output()
        && output.status.success()
        && wait_for_daemon_ready(socket_path, 25)
    {
        return Ok(());
    }

    // 2. Fallback: direct process spawn (detached from GUI process group)
    let binary = find_fluffy_executable()?;
    let mut cmd = std::process::Command::new(&binary);
    cmd.arg("daemon");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn daemon process ({}): {e}", binary.display()))?;

    if wait_for_daemon_ready(socket_path, 25) {
        Ok(())
    } else {
        Err("Daemon started, but IPC socket did not become ready in time".to_string())
    }
}

/// Classifies raw error strings into structured, user-friendly failure categories:
/// - connection unavailable
/// - timeout
/// - daemon stopped
/// - invalid response
fn classify_daemon_error(err: &str) -> &'static str {
    let lower = err.to_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        "timeout"
    } else if lower.contains("no such file")
        || lower.contains("connection refused")
        || lower.contains("broken pipe")
    {
        "daemon stopped"
    } else if lower.contains("invalid response")
        || lower.contains("malformed")
        || lower.contains("failed to parse")
    {
        "invalid response"
    } else {
        "connection unavailable"
    }
}

fn async_fetch_status(socket_path: PathBuf) -> Task<Message> {
    cosmic::app::Task::future(async move {
        let res = tokio::task::spawn_blocking(move || {
            let client = IpcClient::with_timeout(&socket_path, Duration::from_millis(1500));
            client.status().map_err(|e| e.to_string())
        })
        .await
        .unwrap_or_else(|e| Err(format!("Task execution failed: {e}")));

        cosmic::Action::App(Message::StatusFetched(res))
    })
}

fn async_playback_control(
    socket_path: PathBuf,
    action: PlaybackControlAction,
    output: Option<String>,
) -> Task<Message> {
    cosmic::app::Task::future(async move {
        let label = match action {
            PlaybackControlAction::Pause => "paused",
            PlaybackControlAction::Resume => "resumed",
            PlaybackControlAction::Stop => "stopped",
        };
        let res = tokio::task::spawn_blocking(move || {
            let client = IpcClient::with_timeout(&socket_path, Duration::from_secs(3));
            let r = match action {
                PlaybackControlAction::Pause => client.pause(output.as_deref()),
                PlaybackControlAction::Resume => client.resume(output.as_deref()),
                PlaybackControlAction::Stop => client.stop(output.as_deref()),
            };
            r.map(|_| label.to_string()).map_err(|e| e.to_string())
        })
        .await
        .unwrap_or_else(|e| Err(format!("Task execution failed: {e}")));

        cosmic::Action::App(Message::PlaybackControlCompleted(res))
    })
}

struct FluffySettingsApp {
    core: Core,
    socket_path: PathBuf,
    daemon_online: bool,
    starting_daemon: bool,
    outputs: Vec<OutputStatus>,
    selected_output: Option<String>,
    chosen_file: Option<PathBuf>,
    path_input: String,
    status_message: Option<(String, bool)>, // (Message, is_error)
    is_converting: bool,
    converting_file: Option<String>,
    spinner_index: usize,
    is_fetching_status: bool,
}

impl Application for FluffySettingsApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.github.wammed.fluffy.settings";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        vec![app_icon(24).into()]
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        let mut subs = Vec::new();

        if self.is_converting {
            subs.push(
                cosmic::iced::time::every(std::time::Duration::from_millis(150))
                    .map(|_| Message::TickSpinner),
            );
        }

        // Periodic background status polling (every 1s if converting, every 3s if idle)
        let poll_interval = if self.is_converting {
            std::time::Duration::from_millis(1000)
        } else {
            std::time::Duration::from_secs(3)
        };
        subs.push(cosmic::iced::time::every(poll_interval).map(|_| Message::TickPoll));

        cosmic::iced::Subscription::batch(subs)
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let socket_path = default_socket_path();
        let mut app = Self {
            core,
            socket_path: socket_path.clone(),
            daemon_online: false,
            starting_daemon: false,
            outputs: Vec::new(),
            selected_output: None,
            chosen_file: None,
            path_input: String::new(),
            status_message: None,
            is_converting: false,
            converting_file: None,
            spinner_index: 0,
            is_fetching_status: true,
        };

        app.core
            .set_header_title("Fluffy Wallpaper Settings".to_string());
        let task = async_fetch_status(socket_path);
        (app, task)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::RefreshStatus => {
                if !self.is_fetching_status {
                    self.is_fetching_status = true;
                    async_fetch_status(self.socket_path.clone())
                } else {
                    Task::none()
                }
            }

            Message::TickPoll => {
                if !self.is_fetching_status {
                    self.is_fetching_status = true;
                    async_fetch_status(self.socket_path.clone())
                } else {
                    Task::none()
                }
            }

            Message::StatusFetched(result) => {
                self.is_fetching_status = false;
                match result {
                    Ok(status) => {
                        self.daemon_online = true;
                        self.outputs = status.outputs;
                        if status.is_converting {
                            self.is_converting = true;
                            if self.converting_file.is_none() {
                                self.converting_file = status.converting_file;
                            }
                        } else if self.converting_file.is_none() {
                            self.is_converting = false;
                        }
                    }
                    Err(err) => {
                        self.daemon_online = false;
                        self.outputs.clear();
                        self.is_converting = false;
                        let category = classify_daemon_error(&err);
                        self.status_message =
                            Some((format!("Daemon status error ({category}): {err}"), true));
                    }
                }
                Task::none()
            }

            Message::StartDaemon => {
                if self.starting_daemon {
                    return Task::none();
                }
                self.starting_daemon = true;
                self.status_message = Some(("Starting wallpaper daemon...".to_string(), false));

                let socket = self.socket_path.clone();
                cosmic::app::Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || launch_daemon(&socket))
                        .await
                        .unwrap_or_else(|e| Err(format!("Task execution failed: {e}")));
                    cosmic::Action::App(Message::DaemonStarted(res))
                })
            }

            Message::DaemonStarted(result) => {
                self.starting_daemon = false;
                match result {
                    Ok(_) => {
                        self.status_message =
                            Some(("Daemon successfully started and ready!".to_string(), false));
                        self.is_fetching_status = true;
                        async_fetch_status(self.socket_path.clone())
                    }
                    Err(err) => {
                        self.status_message =
                            Some((format!("Failed to start daemon: {err}"), true));
                        Task::none()
                    }
                }
            }

            Message::SelectOutput(out) => {
                self.selected_output = out;
                Task::none()
            }

            Message::PathInputChanged(path) => {
                self.path_input = path.clone();
                let p = PathBuf::from(path);
                if p.exists() && p.is_file() {
                    self.chosen_file = Some(p);
                } else {
                    self.chosen_file = None;
                }
                Task::none()
            }

            Message::PickFile => {
                if let Some(file) = rfd::FileDialog::new()
                    .add_filter("Videos", &["mp4", "mkv", "webm", "mov", "avi"])
                    .set_title("Select Wallpaper Video")
                    .pick_file()
                {
                    self.path_input = file.to_string_lossy().to_string();
                    self.chosen_file = Some(file);
                }
                Task::none()
            }

            Message::ApplyWallpaper => {
                let Some(ref file) = self.chosen_file else {
                    self.status_message =
                        Some(("Please select a valid video file first".to_string(), true));
                    return Task::none();
                };

                let file_path = file.clone();
                let output = self.selected_output.clone();
                let socket = self.socket_path.clone();

                self.is_converting = true;
                let file_name = file.file_name().map(|n| n.to_string_lossy().to_string());
                self.converting_file = file_name;
                self.status_message = None;

                // Non-blocking asynchronous task: sends set-video over IPC to daemon
                cosmic::app::Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || {
                        let client = IpcClient::with_timeout(&socket, Duration::from_secs(60));
                        client
                            .set_video(&file_path, output.as_deref(), None)
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Task execution failed: {e}")));

                    cosmic::Action::App(Message::WallpaperApplied(res))
                })
            }

            Message::WallpaperApplied(result) => {
                self.is_converting = false;
                self.converting_file = None;
                match result {
                    Ok(_) => {
                        self.status_message =
                            Some(("Wallpaper successfully applied!".to_string(), false));
                        self.is_fetching_status = true;
                        async_fetch_status(self.socket_path.clone())
                    }
                    Err(e) => {
                        let category = classify_daemon_error(&e);
                        self.status_message =
                            Some((format!("Failed to apply wallpaper ({category}): {e}"), true));
                        Task::none()
                    }
                }
            }

            Message::TickSpinner => {
                self.spinner_index = self.spinner_index.wrapping_add(1);
                Task::none()
            }

            Message::Pause => async_playback_control(
                self.socket_path.clone(),
                PlaybackControlAction::Pause,
                self.selected_output.clone(),
            ),

            Message::Resume => async_playback_control(
                self.socket_path.clone(),
                PlaybackControlAction::Resume,
                self.selected_output.clone(),
            ),

            Message::Stop => async_playback_control(
                self.socket_path.clone(),
                PlaybackControlAction::Stop,
                self.selected_output.clone(),
            ),

            Message::PlaybackControlCompleted(result) => {
                match result {
                    Ok(action) => {
                        self.status_message = Some((format!("Playback {action}"), false));
                    }
                    Err(e) => {
                        let category = classify_daemon_error(&e);
                        self.status_message =
                            Some((format!("Playback command failed ({category}): {e}"), true));
                    }
                }
                self.is_fetching_status = true;
                async_fetch_status(self.socket_path.clone())
            }

            Message::DismissMessage => {
                self.status_message = None;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let mut content = column![].spacing(16).padding(20);

        // Header: Title and Daemon Status
        let daemon_status_text = if self.daemon_online {
            text("🟢 Daemon Online (Ready)").size(14)
        } else if self.starting_daemon {
            text("🟡 Starting Daemon...").size(14)
        } else {
            text("🔴 Daemon Offline").size(14)
        };

        let mut header_actions = row![].spacing(8).align_y(Alignment::Center);

        if !self.daemon_online {
            let start_btn = if self.starting_daemon {
                button::standard("Starting...")
            } else {
                button::suggested("▶ Start Daemon").on_press(Message::StartDaemon)
            };
            header_actions = header_actions.push(start_btn);
        }

        header_actions =
            header_actions.push(button::standard("Refresh").on_press(Message::RefreshStatus));

        let header = row![
            app_icon(36),
            text("Fluffy Video Wallpaper").size(24),
            Space::new().width(Length::Fill),
            daemon_status_text,
            header_actions
        ]
        .align_y(Alignment::Center)
        .spacing(12);

        content = content.push(header);

        // Status / Error Banner
        if let Some((ref msg, _is_err)) = self.status_message {
            let banner = row![
                text(msg).size(13),
                Space::new().width(Length::Fill),
                button::text("✕").on_press(Message::DismissMessage)
            ]
            .align_y(Alignment::Center)
            .spacing(10);

            content = content.push(container(banner).padding(10).width(Length::Fill));
        }

        // Section: Output Selector
        content = content.push(text("Target Display:").size(16));

        let mut output_buttons = row![].spacing(8);

        let all_btn = if self.selected_output.is_none() {
            button::suggested("All Displays")
        } else {
            button::standard("All Displays")
        }
        .on_press(Message::SelectOutput(None));
        output_buttons = output_buttons.push(all_btn);

        for out in &self.outputs {
            let is_sel = self.selected_output.as_deref() == Some(&out.name);
            let geom_str = if out.width > 0 && out.height > 0 {
                format!(" ({}x{})", out.width, out.height)
            } else {
                String::new()
            };
            let label = format!("{}{}: {}", out.name, geom_str, out.state);
            let btn = if is_sel {
                button::suggested(label)
            } else {
                button::standard(label)
            }
            .on_press(Message::SelectOutput(Some(out.name.clone())));
            output_buttons = output_buttons.push(btn);
        }

        content = content.push(output_buttons);

        // Display current output status cards
        if !self.outputs.is_empty() {
            let mut cards = column![].spacing(8);
            for out in &self.outputs {
                let video_label = out
                    .current_video
                    .as_deref()
                    .and_then(|p| Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "None".to_string());

                let card_row = row![
                    text(format!("[{}]", out.name)).size(14),
                    text(format!("State: {}", out.state)).size(14),
                    text(format!("Playing: {video_label}")).size(14),
                    text(format!("Loops: {}", out.loop_count)).size(14)
                ]
                .spacing(16);

                cards = cards.push(container(card_row).padding(8).width(Length::Fill));
            }
            content = content.push(cards);
        }

        // Section: Video Picker
        content = content.push(text("Select Video:").size(16));

        let file_input_row = row![
            text_input("Enter video path...", &self.path_input)
                .on_input(Message::PathInputChanged)
                .width(Length::Fill),
            button::standard("Browse...").on_press(Message::PickFile)
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        content = content.push(file_input_row);

        // Converting Status Card / Spinner Animation
        if self.is_converting {
            let arrow_frame = ARROW_FRAMES[self.spinner_index % ARROW_FRAMES.len()];
            let file_desc = self.converting_file.as_deref().unwrap_or("動画ファイル");
            let converting_banner = column![
                row![
                    text(format!("⚙️ {arrow_frame} 動画を最適化・変換中...")).size(15),
                    Space::new().width(Length::Fill),
                    text(format!("対象: {file_desc}")).size(13),
                ]
                .align_y(Alignment::Center)
                .spacing(8),
                text("・初回のみ Fluffy 規格（H.264/30fps）への正規化が行われるため、CPU負荷と時間がかかります。").size(12),
                text("・変換中も現在の壁紙は途切れることなく継続再生されます（黒画面にはなりません）。").size(12),
                text("・一度変換された動画は永続ストレージに保存され、2回目以降は即座に再生されます。").size(12),
            ]
            .spacing(4);

            content = content.push(container(converting_banner).padding(12).width(Length::Fill));
        }

        // Apply Button
        let apply_btn = if self.is_converting {
            let arrow_frame = ARROW_FRAMES[self.spinner_index % ARROW_FRAMES.len()];
            button::standard(format!("⚙️ 最適化・変換中... {arrow_frame}"))
        } else if self.chosen_file.is_some() {
            button::suggested("Apply Video Wallpaper").on_press(Message::ApplyWallpaper)
        } else {
            button::standard("Apply Video Wallpaper")
        };

        content = content.push(apply_btn);

        // Section: Playback Controls
        content = content.push(text("Playback Controls:").size(16));
        let controls = row![
            button::standard("⏸ Pause").on_press(Message::Pause),
            button::standard("▶ Resume").on_press(Message::Resume),
            button::destructive("⏹ Stop").on_press(Message::Stop)
        ]
        .spacing(8);

        content = content.push(controls);

        // Section: Video Specification Guide
        let guide = column![
            text("💡 動画フォーマット規格ガイド:").size(14),
            text("【そのまま即座に再生できる適合規格】:").size(12),
            text("  ・コンテナ: MP4 / コーデック: H.264 (avc1) / 色空間: yuv420p").size(12),
            text("  ・フレームレート: 30fps以下 / 解像度: 偶数幅×偶数高さ (4K: 3840x2160 以下)").size(12),
            text("【その他の動画 (HEVC/AV1/60fps/MKV等)】: 初回適用時に自動でバックグラウンド変換され、永続ストレージ (~/.local/share/fluffy/storage) に保存されます。2回目以降はスムーズに即座に再生されます。").size(12),
        ]
        .spacing(3);

        content = content.push(container(guide).padding(12).width(Length::Fill));

        scrollable(content).into()
    }
}

fn main() -> cosmic::iced::Result {
    let settings = Settings::default().size(Size::new(760.0, 560.0));
    cosmic::app::run::<FluffySettingsApp>(settings, ())
}
