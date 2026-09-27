use std::path::{Path, PathBuf};

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::widget::{column, container, row, scrollable, text, Space};
use cosmic::iced::{Alignment, Length, Size};
use cosmic::widget::{button, text_input};
use cosmic::{Application, Element};

use fluffy::cache::CacheManager;
use fluffy::ipc::{default_socket_path, IpcClient, OutputStatus};

#[derive(Debug, Clone)]
pub enum Message {
    RefreshStatus,
    SelectOutput(Option<String>), // None = All outputs
    PickFile,
    PathInputChanged(String),
    ApplyWallpaper,
    Pause,
    Resume,
    Stop,
    DismissMessage,
}

const FLUFFY_ICON_SVG: &[u8] = include_bytes!("../../images/fluffy-icon.svg");

fn app_icon(size: u16) -> cosmic::widget::icon::Icon {
    cosmic::widget::icon::from_svg_bytes(FLUFFY_ICON_SVG).icon().size(size)
}

struct FluffySettingsApp {
    core: Core,
    socket_path: PathBuf,
    daemon_online: bool,
    outputs: Vec<OutputStatus>,
    selected_output: Option<String>,
    chosen_file: Option<PathBuf>,
    path_input: String,
    status_message: Option<(String, bool)>, // (Message, is_error)
}

impl FluffySettingsApp {
    fn fetch_status(&mut self) {
        let client = IpcClient::new(&self.socket_path);
        match client.status() {
            Ok(status) => {
                self.daemon_online = true;
                self.outputs = status.outputs;
            }
            Err(err) => {
                self.daemon_online = false;
                self.outputs.clear();
                self.status_message = Some((
                    format!("Daemon offline or unreachable: {err}"),
                    true,
                ));
            }
        }
    }
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

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let socket_path = default_socket_path();
        let mut app = Self {
            core,
            socket_path,
            daemon_online: false,
            outputs: Vec::new(),
            selected_output: None,
            chosen_file: None,
            path_input: String::new(),
            status_message: None,
        };

        app.core.set_header_title("Fluffy Wallpaper Settings".to_string());
        app.fetch_status();
        (app, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::RefreshStatus => {
                self.fetch_status();
                if self.daemon_online {
                    self.status_message = Some(("Daemon status updated".to_string(), false));
                }
                Task::none()
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
                    self.status_message = Some(("Please select a valid video file first".to_string(), true));
                    return Task::none();
                };

                let file_path = file.clone();
                let output = self.selected_output.clone();
                let socket = self.socket_path.clone();

                // Validate & import locally via CacheManager
                match CacheManager::new(CacheManager::default_cache_dir()) {
                    Ok(cache) => match cache.import_video(&file_path) {
                        Ok(cached_path) => {
                            // Send set-video over IPC
                            let client = IpcClient::new(&socket);
                            match client.set_video(&cached_path, output.as_deref(), None) {
                                Ok(_) => {
                                    self.status_message = Some(("Wallpaper successfully applied!".to_string(), false));
                                    self.fetch_status();
                                }
                                Err(e) => {
                                    self.status_message = Some((format!("Failed to apply wallpaper: {e}"), true));
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = Some((format!("Video validation failed: {e}"), true));
                        }
                    },
                    Err(e) => {
                        self.status_message = Some((format!("Cache error: {e}"), true));
                    }
                }
                Task::none()
            }

            Message::Pause => {
                let client = IpcClient::new(&self.socket_path);
                let _ = client.pause(self.selected_output.as_deref());
                self.fetch_status();
                Task::none()
            }

            Message::Resume => {
                let client = IpcClient::new(&self.socket_path);
                let _ = client.resume(self.selected_output.as_deref());
                self.fetch_status();
                Task::none()
            }

            Message::Stop => {
                let client = IpcClient::new(&self.socket_path);
                let _ = client.stop(self.selected_output.as_deref());
                self.fetch_status();
                Task::none()
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
        } else {
            text("🔴 Daemon Offline").size(14)
        };

        let header = row![
            app_icon(36),
            text("Fluffy Video Wallpaper").size(24),
            Space::new().width(Length::Fill),
            daemon_status_text,
            button::standard("Refresh").on_press(Message::RefreshStatus)
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

            content = content.push(
                container(banner)
                    .padding(10)
                    .width(Length::Fill)
            );
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
            let label = format!("{}: {}", out.name, out.state);
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

                cards = cards.push(
                    container(card_row)
                        .padding(8)
                        .width(Length::Fill)
                );
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

        // Apply Button
        let apply_btn = if self.chosen_file.is_some() {
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

        scrollable(content).into()
    }
}

fn main() -> cosmic::iced::Result {
    let settings = Settings::default().size(Size::new(760.0, 560.0));
    cosmic::app::run::<FluffySettingsApp>(settings, ())
}
