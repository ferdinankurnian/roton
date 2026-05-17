mod audio;
mod config;
mod recorder;

use audio::AudioDevice;
use config::Settings;
use display_info::DisplayInfo;
use iced::widget::{column, container, image, mouse_area, row, stack, svg, text, Space};
use iced::{
    alignment, application, border, time, window, Color, Element, Length, Padding, Shadow,
    Subscription, Task, Theme,
};
use notify_rust::Notification;
use recorder::Recorder;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const NODE_CARD_WIDTH: f32 = 148.0;
const NODE_CARD_HEIGHT: f32 = 134.0;
const NODE_ROW_SPACING: f32 = 24.0;
const NODE_COLUMN_SPACING: f32 = 52.0;
const GRAPH_WIDTH: f32 = (NODE_CARD_WIDTH * 2.0) + NODE_ROW_SPACING;
const GRAPH_HEIGHT: f32 = (NODE_CARD_HEIGHT * 2.0) + NODE_COLUMN_SPACING;

fn main() -> iced::Result {
    application("roton", Roton::update, Roton::view)
        .theme(|_| Theme::Dark)
        .subscription(Roton::subscription)
        .window_size((1150.0, 580.0))
        .decorations(false)
        .run_with(|| (Roton::new(), Task::none()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    Output,
    Screen,
    Mic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenMode {
    Fullscreen,
    SelectArea,
}

impl ScreenMode {
    fn label(self) -> &'static str {
        match self {
            Self::Fullscreen => "Fullscreen",
            Self::SelectArea => "Select Area",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioMode {
    Mute,
    Mic,
}

impl AudioMode {
    fn label(self) -> &'static str {
        match self {
            Self::Mute => "Muted",
            Self::Mic => "Active",
        }
    }
}

impl NodeKind {
    fn title(self) -> &'static str {
        match self {
            Self::Output => "Output",
            Self::Screen => "Screen",
            Self::Mic => "Mic",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Output => "assets/icons/file-video-camera.svg",
            Self::Screen => "assets/icons/monitor.svg",
            Self::Mic => "assets/icons/mic.svg",
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    ToggleRecord,
    TogglePause,
    DragWindow,
    CloseWindow,
    MaximizeWindow,
    SetWindowMaximized(bool),
    MinimizeWindow,
    RestoreWindow,
    TrayTick,
    ToggleTitlebarMenu,
    HoverTitlebarMenuItem(Option<usize>),
    HoverTitlebarAction(Option<TitlebarAction>),
    ToggleMinimalWindow,
    ToggleMinimizeToTray,
    ConfirmClose,
    CancelClose,
    OpenNode(NodeKind),
    HoverNode(Option<NodeKind>),
    HoverModalClose(bool),
    HoverRecord(bool),
    HoverPause(bool),
    HoverChoose(bool),
    HoverScreenMode(Option<ScreenMode>),
    HoverMicToggle(bool),
    HoverShowCursor(bool),
    HoverScreenSound(bool),
    PauseBlinkTick,
    RecordingTick,
    Frame,
    CloseModal,
    ToggleFormatDropdown,
    ToggleMonitorDropdown,
    ToggleMicDropdown,
    HoverDropdownOption(Option<usize>),
    DismissDropdown,
    SelectMic(String),
    SelectMicMode(AudioMode),
    SelectFormat(String),
    SelectMonitor(String),
    SelectScreenMode(ScreenMode),
    SelectArea,
    AreaSelected(Option<String>),
    ToggleShowCursor,
    ToggleScreenSound,
    ChooseOutputFolder,
    OutputFolderChosen(Option<String>),
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitlebarAction {
    Minimize,
    Maximize,
    Close,
}

struct Roton {
    recorder: Arc<Mutex<Recorder>>,
    settings: Settings,
    audio_devices: Vec<AudioDevice>,
    formats: Vec<String>,
    selected_format: String,
    is_format_dropdown_open: bool,
    monitors: Vec<String>,
    selected_monitor: Option<String>,
    is_monitor_dropdown_open: bool,
    mics: Vec<String>,
    selected_mic: Option<String>,
    is_mic_dropdown_open: bool,
    mic_mode: AudioMode,
    hovered_dropdown_option: Option<usize>,
    screen_mode: ScreenMode,
    selected_area: Option<String>,
    show_cursor: bool,
    record_screen_sound: bool,
    tray_icon: Option<TrayIcon>,
    selected_node: Option<NodeKind>,
    hovered_node: Option<NodeKind>,
    is_titlebar_menu_open: bool,
    hovered_titlebar_menu_item: Option<usize>,
    hovered_titlebar_action: Option<TitlebarAction>,
    is_minimal_window: bool,
    show_close_confirmation: bool,
    is_modal_close_hovered: bool,
    is_record_hovered: bool,
    is_pause_hovered: bool,
    is_choose_hovered: bool,
    hovered_screen_mode: Option<ScreenMode>,
    is_mic_toggle_hovered: bool,
    is_show_cursor_hovered: bool,
    is_screen_sound_hovered: bool,
    modal_progress: f32,
    is_recording: bool,
    is_paused: bool,
    pause_blink_on: bool,
    elapsed: u64,
    current_recording_path: Option<PathBuf>,
    has_wl_screenrec: bool,
    has_slurp: bool,
    has_ffmpeg: bool,
    has_pactl: bool,
}

impl Roton {
    fn new() -> Self {
        let mut app = Self {
            recorder: Arc::new(Mutex::new(Recorder::new())),
            settings: Settings::load(),
            audio_devices: Vec::new(),
            formats: vec!["MP4".to_string(), "MKV".to_string(), "WEBM".to_string()],
            selected_format: "MP4".to_string(),
            is_format_dropdown_open: false,
            monitors: display_monitors(),
            selected_monitor: None,
            is_monitor_dropdown_open: false,
            mics: Vec::new(),
            selected_mic: None,
            is_mic_dropdown_open: false,
            mic_mode: AudioMode::Mute,
            hovered_dropdown_option: None,
            screen_mode: ScreenMode::Fullscreen,
            selected_area: None,
            show_cursor: true,
            record_screen_sound: false,
            tray_icon: create_tray_icon(),
            selected_node: None,
            hovered_node: None,
            is_titlebar_menu_open: false,
            hovered_titlebar_menu_item: None,
            hovered_titlebar_action: None,
            is_minimal_window: false,
            show_close_confirmation: false,
            is_modal_close_hovered: false,
            is_record_hovered: false,
            is_pause_hovered: false,
            is_choose_hovered: false,
            hovered_screen_mode: None,
            is_mic_toggle_hovered: false,
            is_show_cursor_hovered: false,
            is_screen_sound_hovered: false,
            modal_progress: 0.0,
            is_recording: false,
            is_paused: false,
            pause_blink_on: true,
            elapsed: 0,
            current_recording_path: None,
            has_wl_screenrec: Recorder::is_installed("wl-screenrec"),
            has_slurp: Recorder::is_installed("slurp"),
            has_ffmpeg: Recorder::is_installed("ffmpeg"),
            has_pactl: Recorder::is_installed("pactl"),
        };
        app.refresh_audio_devices();
        app.selected_monitor = app.monitors.first().cloned();
        app
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleRecord => {
                if self.is_recording {
                    if let Err(error) = self.finish_recording() {
                        eprintln!("Failed to stop recording: {error}");
                    }
                } else {
                    if let Err(error) = self.start_recording() {
                        eprintln!("Failed to start recording: {error}");
                    }
                }
            }
            Message::TogglePause => {
                if self.is_recording {
                    let result = if self.is_paused {
                        self.recorder
                            .lock()
                            .map_err(|_| "Recorder lock poisoned".to_string())
                            .and_then(|mut recorder| recorder.resume_session())
                    } else {
                        self.recorder
                            .lock()
                            .map_err(|_| "Recorder lock poisoned".to_string())
                            .and_then(|mut recorder| recorder.pause_session())
                    };

                    if let Err(error) = result {
                        eprintln!("Failed to toggle pause: {error}");
                    } else {
                        self.is_paused = !self.is_paused;
                        if !self.is_paused {
                            self.pause_blink_on = true;
                        }
                    }
                }
            }
            Message::DragWindow => {
                if self.is_titlebar_menu_open {
                    return Task::none();
                }
                return window::get_latest().and_then(window::drag);
            }
            Message::CloseWindow => {
                if self.settings.minimize_to_tray {
                    return self.minimize_to_tray();
                }
                if self.is_recording {
                    self.show_close_confirmation = true;
                    self.is_titlebar_menu_open = false;
                    self.hovered_titlebar_menu_item = None;
                    return Task::none();
                }
                return window::get_latest().and_then(window::close);
            }
            Message::MaximizeWindow => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                return window::get_latest().and_then(|id| {
                    window::get_maximized(id)
                        .map(move |is_maximized| Message::SetWindowMaximized(!is_maximized))
                });
            }
            Message::SetWindowMaximized(maximized) => {
                return window::get_latest().and_then(move |id| window::maximize(id, maximized));
            }
            Message::MinimizeWindow => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                return window::get_latest().and_then(|id| window::minimize(id, true));
            }
            Message::RestoreWindow => {
                return window::get_latest().and_then(|id| window::minimize(id, false));
            }
            Message::TrayTick => {
                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                        | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        } => return Task::done(Message::RestoreWindow),
                        _ => {}
                    }
                }
            }
            Message::ToggleTitlebarMenu => {
                self.is_titlebar_menu_open = !self.is_titlebar_menu_open;
                self.hovered_titlebar_menu_item = None;
            }
            Message::HoverTitlebarMenuItem(item) => {
                self.hovered_titlebar_menu_item = item;
            }
            Message::HoverTitlebarAction(action) => {
                self.hovered_titlebar_action = action;
            }
            Message::ToggleMinimalWindow => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.is_minimal_window = !self.is_minimal_window;
            }
            Message::ToggleMinimizeToTray => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.settings.minimize_to_tray = !self.settings.minimize_to_tray;
                let _ = self.settings.save();
            }
            Message::ConfirmClose => {
                if let Err(error) = self.finish_recording() {
                    eprintln!("Failed to stop recording before close: {error}");
                }
                self.show_close_confirmation = false;
                return window::get_latest().and_then(window::close);
            }
            Message::CancelClose => {
                self.show_close_confirmation = false;
            }
            Message::OpenNode(kind) => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.selected_node = Some(kind);
                self.hovered_node = None;
                self.modal_progress = 0.0;
            }
            Message::HoverNode(kind) => {
                if self.selected_node.is_none() {
                    self.hovered_node = kind;
                }
            }
            Message::HoverModalClose(is_hovered) => {
                self.is_modal_close_hovered = is_hovered;
            }
            Message::HoverRecord(is_hovered) => {
                self.is_record_hovered = is_hovered;
            }
            Message::HoverPause(is_hovered) => {
                self.is_pause_hovered = is_hovered;
            }
            Message::HoverChoose(is_hovered) => {
                self.is_choose_hovered = is_hovered;
            }
            Message::HoverScreenMode(mode) => {
                self.hovered_screen_mode = mode;
            }
            Message::HoverMicToggle(is_hovered) => {
                self.is_mic_toggle_hovered = is_hovered;
            }
            Message::HoverShowCursor(is_hovered) => {
                self.is_show_cursor_hovered = is_hovered;
            }
            Message::HoverScreenSound(is_hovered) => {
                self.is_screen_sound_hovered = is_hovered;
            }
            Message::PauseBlinkTick => {
                if self.is_paused {
                    self.pause_blink_on = !self.pause_blink_on;
                } else {
                    self.pause_blink_on = true;
                }
            }
            Message::RecordingTick => {
                if self.is_recording && !self.is_paused {
                    self.elapsed += 1;
                }
            }
            Message::Frame => {
                if self.selected_node.is_some() {
                    self.modal_progress = (self.modal_progress + 0.16).min(1.0);
                }
            }
            Message::CloseModal => {
                self.selected_node = None;
                self.hovered_node = None;
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.is_modal_close_hovered = false;
                self.is_format_dropdown_open = false;
                self.is_monitor_dropdown_open = false;
                self.is_mic_dropdown_open = false;
                self.hovered_dropdown_option = None;
                self.modal_progress = 0.0;
            }
            Message::ToggleFormatDropdown => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.is_format_dropdown_open = !self.is_format_dropdown_open;
                self.is_monitor_dropdown_open = false;
                self.is_mic_dropdown_open = false;
                self.hovered_dropdown_option = None;
            }
            Message::ToggleMonitorDropdown => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.is_monitor_dropdown_open = !self.is_monitor_dropdown_open;
                self.is_format_dropdown_open = false;
                self.is_mic_dropdown_open = false;
                self.hovered_dropdown_option = None;
            }
            Message::ToggleMicDropdown => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.is_mic_dropdown_open = !self.is_mic_dropdown_open;
                self.is_format_dropdown_open = false;
                self.is_monitor_dropdown_open = false;
                self.hovered_dropdown_option = None;
            }
            Message::HoverDropdownOption(option) => {
                self.hovered_dropdown_option = option;
            }
            Message::DismissDropdown => {
                self.is_format_dropdown_open = false;
                self.is_monitor_dropdown_open = false;
                self.is_mic_dropdown_open = false;
                self.hovered_dropdown_option = None;
            }
            Message::SelectFormat(format) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.selected_format = format;
                self.is_format_dropdown_open = false;
                self.hovered_dropdown_option = None;
                self.hovered_node = None;
            }
            Message::SelectMonitor(monitor) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.selected_monitor = Some(monitor);
                self.is_monitor_dropdown_open = false;
                self.hovered_dropdown_option = None;
                self.hovered_node = None;
            }
            Message::SelectMic(mic) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.selected_mic = Some(mic);
                self.is_mic_dropdown_open = false;
                self.hovered_dropdown_option = None;
            }
            Message::SelectMicMode(mode) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.mic_mode = mode;
                self.settings.audio_mode = match mode {
                    AudioMode::Mute => "Mute".to_string(),
                    AudioMode::Mic => "Mic".to_string(),
                };
                let _ = self.settings.save();
            }
            Message::SelectScreenMode(mode) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.screen_mode = mode;
            }
            Message::SelectArea => {
                if self.is_config_locked() {
                    return Task::none();
                }
                if !self.has_slurp {
                    eprintln!("slurp is not installed");
                    return Task::none();
                }
                self.screen_mode = ScreenMode::SelectArea;
                return Task::perform(select_area(), Message::AreaSelected);
            }
            Message::AreaSelected(area) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                if let Some(area) = area {
                    self.selected_area = Some(area);
                }
            }
            Message::ToggleShowCursor => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.show_cursor = !self.show_cursor;
            }
            Message::ToggleScreenSound => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.record_screen_sound = !self.record_screen_sound;
            }
            Message::ChooseOutputFolder => {
                if self.is_config_locked() {
                    return Task::none();
                }
                return Task::perform(pick_output_folder(), Message::OutputFolderChosen);
            }
            Message::OutputFolderChosen(path) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                if let Some(path) = path {
                    self.settings.save_path = path;
                    let _ = self.settings.save();
                }
            }
            Message::Noop => {}
        }

        Task::none()
    }

    fn is_config_locked(&self) -> bool {
        self.is_recording
    }

    fn start_recording(&mut self) -> Result<(), String> {
        if !self.has_wl_screenrec {
            return Err("wl-screenrec is not installed".to_string());
        }
        if self.is_paused && !self.has_ffmpeg {
            return Err("ffmpeg is required to finish paused recordings".to_string());
        }

        let output_path = self.recording_output_path();
        let audio_mode = self.recording_audio_mode();
        let mic = self.selected_audio_device(false);
        let monitor = self.selected_audio_device(true);
        let output = if self.screen_mode == ScreenMode::Fullscreen {
            self.selected_monitor_output()
        } else {
            None
        };
        let geometry = if self.screen_mode == ScreenMode::SelectArea {
            self.selected_area.as_deref()
        } else {
            None
        };

        self.recorder
            .lock()
            .map_err(|_| "Recorder lock poisoned".to_string())?
            .start_session(
                output_path.to_string_lossy().as_ref(),
                geometry,
                &audio_mode,
                mic.as_deref(),
                monitor.as_deref(),
                output.as_deref(),
                self.show_cursor,
            )?;

        self.is_recording = true;
        self.is_paused = false;
        self.pause_blink_on = true;
        self.elapsed = 0;
        self.current_recording_path = Some(output_path);
        self.is_format_dropdown_open = false;
        self.is_monitor_dropdown_open = false;
        self.is_mic_dropdown_open = false;
        self.hovered_dropdown_option = None;
        Ok(())
    }

    fn finish_recording(&mut self) -> Result<(), String> {
        if !self.has_ffmpeg {
            return Err("ffmpeg is required to finish recordings".to_string());
        }
        self.recorder
            .lock()
            .map_err(|_| "Recorder lock poisoned".to_string())?
            .finish_session()?;
        let video_path = self.current_recording_path.take();
        self.is_recording = false;
        self.is_paused = false;
        notify_recording_completed(self.settings.save_path.clone(), video_path);
        Ok(())
    }

    fn minimize_to_tray(&self) -> Task<Message> {
        if self.tray_icon.is_none() {
            return window::get_latest().and_then(window::close);
        }
        window::get_latest().and_then(|id| window::minimize(id, true))
    }

    fn recording_output_path(&self) -> PathBuf {
        let extension = self.selected_format.to_lowercase();
        let filename = format!(
            "recording_{}.{}",
            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"),
            extension
        );
        Path::new(&self.settings.save_path).join(filename)
    }

    fn recording_audio_mode(&self) -> String {
        match (self.mic_mode, self.record_screen_sound) {
            (AudioMode::Mic, true) => "Both",
            (AudioMode::Mic, false) => "Mic",
            (AudioMode::Mute, true) => "Screen",
            (AudioMode::Mute, false) => "Mute",
        }
        .to_string()
    }

    fn selected_audio_device(&self, monitor: bool) -> Option<String> {
        self.audio_devices
            .iter()
            .find(|device| {
                device.is_monitor == monitor
                    && if monitor {
                        true
                    } else {
                        Some(device.description.as_str()) == self.selected_mic.as_deref()
                    }
            })
            .map(|device| device.name.clone())
    }

    fn selected_monitor_output(&self) -> Option<String> {
        self.selected_monitor
            .as_deref()
            .and_then(|monitor| monitor.split('·').next())
            .map(str::trim)
            .filter(|monitor| !monitor.is_empty())
            .map(ToOwned::to_owned)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        subscriptions.push(window::close_requests().map(|_| Message::CloseWindow));
        subscriptions.push(time::every(std::time::Duration::from_millis(350)).map(|_| {
            Message::TrayTick
        }));

        if self.selected_node.is_some() && self.modal_progress < 1.0 {
            subscriptions.push(window::frames().map(|_| Message::Frame));
        }

        if self.is_paused {
            subscriptions.push(
                time::every(std::time::Duration::from_millis(420)).map(|_| Message::PauseBlinkTick),
            );
        }

        if self.is_recording {
            subscriptions.push(
                time::every(std::time::Duration::from_secs(1)).map(|_| Message::RecordingTick),
            );
        }

        Subscription::batch(subscriptions)
    }

    fn refresh_audio_devices(&mut self) {
        self.audio_devices = if self.has_pactl {
            audio::get_audio_devices()
        } else {
            Vec::new()
        };
        self.mics = self
            .audio_devices
            .iter()
            .filter(|device| !device.is_monitor)
            .map(|device| device.description.clone())
            .collect();
        if self.selected_mic.is_none() {
            self.selected_mic = self.mics.first().cloned();
        }
    }

    fn view(&self) -> Element<Message> {
        let content = row![
            self.sidebar(),
            container(self.canvas())
                .width(Length::Fill)
                .height(Length::Fill)
        ]
        .height(Length::Fill);

        let app = container(column![self.titlebar(), content].height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(panel);

        let app: Element<_> = if self.is_titlebar_menu_open {
            stack![
                app,
                mouse_area(
                    container(Space::with_width(Length::Fill).height(Length::Fill))
                        .width(Length::Fill)
                        .height(Length::Fill)
                )
                .on_press(Message::ToggleTitlebarMenu),
                container(self.titlebar_menu())
                    .padding(Padding::default().top(34).left(8))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Left)
                    .align_y(alignment::Vertical::Top)
            ]
            .into()
        } else {
            app.into()
        };

        if self.show_close_confirmation {
            stack![
                app,
                mouse_area(
                    container(Space::with_width(Length::Fill).height(Length::Fill))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|_| scrim(1.0))
                )
                .on_press(Message::Noop),
                container(self.close_confirmation())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if let Some(kind) = self.selected_node {
            let progress = ease_out(self.modal_progress);
            let has_dropdown_open = self.is_format_dropdown_open
                || self.is_monitor_dropdown_open
                || self.is_mic_dropdown_open;
            let backdrop_press = if has_dropdown_open {
                Message::DismissDropdown
            } else {
                Message::CloseModal
            };
            let modal_press = if has_dropdown_open {
                Message::DismissDropdown
            } else {
                Message::Noop
            };
            stack![
                app,
                mouse_area(
                    container(Space::with_width(Length::Fill).height(Length::Fill))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(move |_| scrim(progress))
                )
                .on_press(backdrop_press),
                container(mouse_area(self.modal(kind, progress)).on_press(modal_press))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else {
            app
        }
    }

    fn close_confirmation(&self) -> Element<Message> {
        container(
            column![
                text("Stop Recording?").size(18),
                text("Roton is still recording. Closing now will stop and save the recording.")
                    .size(13),
                row![
                    mouse_area(
                        container(text("Cancel").size(13))
                            .padding([10, 14])
                            .style(pill)
                    )
                    .on_press(Message::CancelClose),
                    Space::with_width(Length::Fill),
                    mouse_area(
                        container(text("Stop and Close").size(13))
                            .padding([10, 14])
                            .style(stop_button)
                    )
                    .on_press(Message::ConfirmClose),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(16),
        )
        .width(420)
        .padding(18)
        .style(|_| modal_panel(1.0))
        .into()
    }

    fn sidebar(&self) -> Element<Message> {
        let status = if self.is_paused {
            "Paused"
        } else if self.is_recording {
            "Recording"
        } else {
            "Idle"
        };
        let record_label = if self.is_recording { "Stop" } else { "Record" };
        let record_icon = if self.is_recording {
            "assets/icons/square.svg"
        } else {
            "assets/icons/record.svg"
        };
        let record_button = mouse_area(
            container(
                row![
                    svg(record_icon).width(16).height(16),
                    text(record_label).size(14),
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
            )
            .padding([10, 28])
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .style(if self.is_recording {
                if self.is_record_hovered {
                    stop_button_hovered
                } else {
                    stop_button
                }
            } else if self.is_record_hovered {
                record_button_hovered
            } else {
                record_button
            }),
        )
        .on_enter(Message::HoverRecord(true))
        .on_exit(Message::HoverRecord(false))
        .on_press(Message::ToggleRecord);
        let controls: Element<_> = if self.is_recording {
            row![
                mouse_area(
                    container(
                        svg(if self.is_paused {
                            "assets/icons/play.svg"
                        } else {
                            "assets/icons/pause.svg"
                        })
                        .width(16)
                        .height(16)
                    )
                    .padding(10)
                    .width(42)
                    .align_x(alignment::Horizontal::Center)
                    .style(if self.is_paused {
                        if self.pause_blink_on {
                            if self.is_pause_hovered {
                                pause_button_active_hovered
                            } else {
                                pause_button_active
                            }
                        } else {
                            pause_button_blink_off
                        }
                    } else if self.is_pause_hovered {
                        side_button_hovered
                    } else {
                        side_button
                    })
                )
                .on_enter(Message::HoverPause(true))
                .on_exit(Message::HoverPause(false))
                .on_press(Message::TogglePause),
                record_button,
            ]
            .spacing(8)
            .width(Length::Fill)
            .into()
        } else {
            record_button.into()
        };

        container(
            column![
                controls,
                row![
                    text(status).size(12),
                    Space::with_width(Length::Fill),
                    text(format!(
                        "{:02}.{:02}:00",
                        self.elapsed / 60,
                        self.elapsed % 60
                    ))
                    .size(12),
                ]
                .width(Length::Fill)
                .align_y(alignment::Vertical::Center),
                Space::with_height(Length::Fill),
                column![
                    text("Roton v1.0.0").size(12),
                    text("By Ferdinan Iydheko").size(11),
                ]
                .spacing(4),
            ]
            .spacing(16),
        )
        .width(210)
        .height(Length::Fill)
        .padding(18)
        .style(sidebar)
        .into()
    }

    fn titlebar(&self) -> Element<Message> {
        let titlebar_action = |action, icon: &'static str, icon_size: u16, message| {
            mouse_area(
                container(image(icon).width(icon_size).height(icon_size))
                    .width(34)
                    .height(34)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
                    .style(if self.hovered_titlebar_action == Some(action) {
                        titlebar_close_hovered
                    } else {
                        titlebar_close
                    }),
            )
            .on_enter(Message::HoverTitlebarAction(Some(action)))
            .on_exit(Message::HoverTitlebarAction(None))
            .on_press(message)
        };

        let right_controls: Element<_> = if self.is_minimal_window {
            row![titlebar_action(
                TitlebarAction::Close,
                "assets/icons/close.png",
                15,
                Message::CloseWindow
            )]
            .align_y(alignment::Vertical::Center)
            .into()
        } else {
            row![
                titlebar_action(
                    TitlebarAction::Minimize,
                    "assets/icons/minimize.png",
                    14,
                    Message::MinimizeWindow
                ),
                titlebar_action(
                    TitlebarAction::Maximize,
                    "assets/icons/maximize.png",
                    13,
                    Message::MaximizeWindow
                ),
                titlebar_action(
                    TitlebarAction::Close,
                    "assets/icons/close.png",
                    15,
                    Message::CloseWindow
                ),
            ]
            .align_y(alignment::Vertical::Center)
            .into()
        };

        container(
            mouse_area(
                row![
                    mouse_area(
                        container(image("assets/rotonicon.png").width(16).height(16))
                            .padding([7, 12])
                            .style(titlebar_icon_button)
                    )
                    .on_press(Message::ToggleTitlebarMenu),
                    Space::with_width(Length::Fill),
                    text("Roton").size(15),
                    Space::with_width(Length::Fill),
                    right_controls,
                ]
                .align_y(alignment::Vertical::Center)
                .height(34),
            )
            .on_press(Message::DragWindow),
        )
        .width(Length::Fill)
        .padding([0, 0])
        .style(titlebar)
        .into()
    }

    fn titlebar_menu(&self) -> Element<Message> {
        let item = |index, label: &'static str, icon: &'static str, message| {
            mouse_area(
                container(
                    row![image(icon).width(15).height(15), text(label).size(13),]
                        .spacing(10)
                        .align_y(alignment::Vertical::Center),
                )
                .padding([9, 10])
                .width(Length::Fill)
                .style(if self.hovered_titlebar_menu_item == Some(index) {
                    titlebar_menu_item_hovered
                } else {
                    titlebar_menu_item
                }),
            )
            .on_enter(Message::HoverTitlebarMenuItem(Some(index)))
            .on_exit(Message::HoverTitlebarMenuItem(None))
            .on_press(message)
        };

        let minimal_marker: Element<_> = if self.is_minimal_window {
            svg("assets/icons/check.svg").width(15).height(15).into()
        } else {
            Space::with_width(15).height(15).into()
        };

        let minimal_item = mouse_area(
            container(
                row![minimal_marker, text("Minimal Window").size(13),]
                    .spacing(10)
                    .align_y(alignment::Vertical::Center),
            )
            .padding([9, 10])
            .width(Length::Fill)
            .style(if self.hovered_titlebar_menu_item == Some(3) {
                titlebar_menu_item_hovered
            } else {
                titlebar_menu_item
            }),
        )
        .on_enter(Message::HoverTitlebarMenuItem(Some(3)))
        .on_exit(Message::HoverTitlebarMenuItem(None))
        .on_press(Message::ToggleMinimalWindow);

        let tray_marker: Element<_> = if self.settings.minimize_to_tray {
            svg("assets/icons/check.svg").width(15).height(15).into()
        } else {
            Space::with_width(15).height(15).into()
        };

        let tray_item = mouse_area(
            container(
                row![tray_marker, text("Minimize to Tray").size(13),]
                    .spacing(10)
                    .align_y(alignment::Vertical::Center),
            )
            .padding([9, 10])
            .width(Length::Fill)
            .style(if self.hovered_titlebar_menu_item == Some(4) {
                titlebar_menu_item_hovered
            } else {
                titlebar_menu_item
            }),
        )
        .on_enter(Message::HoverTitlebarMenuItem(Some(4)))
        .on_exit(Message::HoverTitlebarMenuItem(None))
        .on_press(Message::ToggleMinimizeToTray);

        mouse_area(
            container(
                column![
                    item(0, "Close", "assets/icons/close.png", Message::CloseWindow),
                    item(
                        1,
                        "Maximize",
                        "assets/icons/maximize.png",
                        Message::MaximizeWindow
                    ),
                    item(
                        2,
                        "Minimize",
                        "assets/icons/minimize.png",
                        Message::MinimizeWindow
                    ),
                    container(
                        container(Space::with_height(1))
                            .width(Length::Fill)
                            .style(titlebar_menu_separator)
                    )
                    .padding([3, 0])
                    .width(Length::Fill),
                    minimal_item,
                    tray_item,
                ]
                .spacing(0),
            )
            .padding(4)
            .width(190)
            .style(titlebar_menu_surface),
        )
        .on_press(Message::Noop)
        .into()
    }

    fn canvas(&self) -> Element<Message> {
        let nodes = container(
            column![
                container(self.node(NodeKind::Output))
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Center),
                row![self.node(NodeKind::Screen), self.node(NodeKind::Mic),]
                    .spacing(NODE_ROW_SPACING)
                    .align_y(alignment::Vertical::Center),
            ]
            .spacing(NODE_COLUMN_SPACING)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        let graph = stack![
            svg(connector_svg_handle())
                .width(Length::Fill)
                .height(Length::Fill),
            nodes,
        ]
        .width(GRAPH_WIDTH)
        .height(GRAPH_HEIGHT);

        container(graph)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(28)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .into()
    }

    fn node(&self, kind: NodeKind) -> Element<Message> {
        let detail = self.node_detail(kind);
        mouse_area(
            container(
                container(
                    column![
                        svg(kind.icon()).width(26).height(26),
                        text(kind.title())
                            .size(14)
                            .width(Length::Fill)
                            .align_x(alignment::Horizontal::Center),
                        text(truncate_text(&detail, 24))
                            .size(11)
                            .width(Length::Fill)
                            .align_x(alignment::Horizontal::Center),
                    ]
                    .spacing(7)
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Center),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center),
            )
            .padding(14)
            .width(NODE_CARD_WIDTH)
            .height(NODE_CARD_HEIGHT)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(if self.is_config_locked() {
                node_card_disabled
            } else if self.selected_node.is_none() && self.hovered_node == Some(kind) {
                node_card_hovered
            } else {
                node_card
            }),
        )
        .on_enter(Message::HoverNode(Some(kind)))
        .on_exit(Message::HoverNode(None))
        .on_press(Message::OpenNode(kind))
        .into()
    }

    fn node_detail(&self, kind: NodeKind) -> String {
        match kind {
            NodeKind::Output => format!("Format {}", self.selected_format),
            NodeKind::Screen => {
                if self.screen_mode == ScreenMode::SelectArea && self.selected_area.is_some() {
                    "Selected Area".to_string()
                } else {
                    self.screen_mode.label().to_string()
                }
            }
            NodeKind::Mic => {
                if self.mic_mode == AudioMode::Mute {
                    "Muted".to_string()
                } else {
                    self.selected_mic
                        .clone()
                        .unwrap_or_else(|| "No Microphone".to_string())
                }
            }
        }
    }

    fn modal(&self, kind: NodeKind, progress: f32) -> Element<Message> {
        let body: Element<_> = match kind {
            NodeKind::Output => column![
                text("Format").size(13),
                self.dropdown(&self.selected_format, Message::ToggleFormatDropdown,),
                text("Save Folder").size(13),
                row![
                    container(text(truncate_text(&self.settings.save_path, 38)).size(13))
                        .padding(10)
                        .width(Length::Fill)
                        .style(input_surface),
                    mouse_area(container(text("Choose").size(13)).padding([10, 14]).style(
                        if self.is_config_locked() {
                            control_disabled
                        } else if self.is_choose_hovered {
                            pill_hovered
                        } else {
                            pill
                        }
                    ))
                    .on_enter(Message::HoverChoose(true))
                    .on_exit(Message::HoverChoose(false))
                    .on_press(if self.is_config_locked() {
                        Message::Noop
                    } else {
                        Message::ChooseOutputFolder
                    }),
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(10)
            .into(),
            NodeKind::Screen => column![
                text("Monitor").size(13),
                self.dropdown(
                    self.selected_monitor.as_deref().unwrap_or("Select monitor"),
                    Message::ToggleMonitorDropdown,
                ),
                self.switch_row(
                    "Show Cursor",
                    self.show_cursor,
                    self.is_show_cursor_hovered,
                    Message::ToggleShowCursor,
                    Message::HoverShowCursor(true),
                    Message::HoverShowCursor(false),
                ),
                self.switch_row(
                    "Record Screen Sound",
                    self.record_screen_sound,
                    self.is_screen_sound_hovered,
                    Message::ToggleScreenSound,
                    Message::HoverScreenSound(true),
                    Message::HoverScreenSound(false),
                ),
                row![
                    self.screen_mode_button(ScreenMode::Fullscreen),
                    self.screen_mode_button(ScreenMode::SelectArea),
                ]
                .spacing(10),
                if self.screen_mode == ScreenMode::SelectArea {
                    mouse_area(
                        container(text("Select Area").size(13))
                            .padding([10, 14])
                            .width(Length::Fill)
                            .align_x(alignment::Horizontal::Center)
                            .style(if self.is_config_locked() {
                                control_disabled
                            } else {
                                record_button
                            }),
                    )
                    .on_press(if self.is_config_locked() {
                        Message::Noop
                    } else {
                        Message::SelectArea
                    })
                } else {
                    mouse_area(container(Space::with_height(0))).on_press(Message::Noop)
                },
            ]
            .spacing(12)
            .into(),
            NodeKind::Mic => column![
                text("Microphone").size(13),
                self.dropdown(
                    self.selected_mic.as_deref().unwrap_or("Select microphone"),
                    Message::ToggleMicDropdown,
                ),
                self.mic_toggle_button(),
            ]
            .spacing(12)
            .into(),
        };

        let panel = container(
            column![
                row![
                    text(format!("{} Config", kind.title())).size(18),
                    Space::with_width(Length::Fill),
                    mouse_area(
                        container(svg("assets/icons/x.svg").width(16).height(16))
                            .padding(8)
                            .style(if self.is_modal_close_hovered {
                                ghost_card_hovered
                            } else {
                                ghost_card
                            })
                    )
                    .on_enter(Message::HoverModalClose(true))
                    .on_exit(Message::HoverModalClose(false))
                    .on_press(Message::CloseModal),
                ]
                .align_y(alignment::Vertical::Center),
                body,
            ]
            .spacing(18),
        )
        .width(460)
        .padding(18)
        .style(move |_| modal_panel(progress));

        if let Some(menu) = self.dropdown_overlay(kind) {
            stack![
                panel,
                container(column![Space::with_height(88), menu,].width(Length::Fill))
                    .width(460)
                    .padding([0, 18])
            ]
            .into()
        } else {
            panel.into()
        }
    }

    fn screen_mode_button(&self, mode: ScreenMode) -> Element<Message> {
        let icon = match mode {
            ScreenMode::Fullscreen => "assets/icons/fullscreen.svg",
            ScreenMode::SelectArea => "assets/icons/square-dashed-mouse-pointer.svg",
        };

        mouse_area(
            container(
                column![
                    svg(icon).width(54).height(54),
                    text(mode.label())
                        .size(12)
                        .width(Length::Fill)
                        .align_x(alignment::Horizontal::Center),
                ]
                .spacing(12)
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .padding(16)
            .width(Length::Fill)
            .height(120)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(if self.is_config_locked() {
                mode_card_disabled
            } else if self.screen_mode == mode {
                selected_mode_card
            } else if self.hovered_screen_mode == Some(mode) {
                node_card_hovered
            } else {
                node_card
            }),
        )
        .on_enter(Message::HoverScreenMode(Some(mode)))
        .on_exit(Message::HoverScreenMode(None))
        .on_press(if self.is_config_locked() {
            Message::Noop
        } else {
            Message::SelectScreenMode(mode)
        })
        .into()
    }

    fn mic_toggle_button(&self) -> Element<Message> {
        let next = if self.mic_mode == AudioMode::Mic {
            AudioMode::Mute
        } else {
            AudioMode::Mic
        };
        let subtitle = if self.mic_mode == AudioMode::Mic {
            "Microphone Will Be Recorded"
        } else {
            "Microphone Is Muted"
        };

        mouse_area(
            container(
                column![
                    svg("assets/icons/mic.svg").width(54).height(54),
                    text(self.mic_mode.label())
                        .size(12)
                        .width(Length::Fill)
                        .align_x(alignment::Horizontal::Center),
                    text(subtitle)
                        .size(11)
                        .width(Length::Fill)
                        .align_x(alignment::Horizontal::Center),
                ]
                .spacing(10)
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .padding(16)
            .width(Length::Fill)
            .height(140)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(if self.is_config_locked() {
                mode_card_disabled
            } else if self.mic_mode == AudioMode::Mic {
                selected_mode_card
            } else if self.is_mic_toggle_hovered {
                node_card_hovered
            } else {
                node_card
            }),
        )
        .on_enter(Message::HoverMicToggle(true))
        .on_exit(Message::HoverMicToggle(false))
        .on_press(if self.is_config_locked() {
            Message::Noop
        } else {
            Message::SelectMicMode(next)
        })
        .into()
    }

    fn switch_row(
        &self,
        label: &'static str,
        is_active: bool,
        is_hovered: bool,
        on_press: Message,
        on_enter: Message,
        on_exit: Message,
    ) -> Element<Message> {
        let locked = self.is_config_locked();
        container(
            row![
                text(label).size(13),
                Space::with_width(Length::Fill),
                mouse_area(
                    container(
                        row![
                            if is_active {
                                Space::with_width(Length::Fill)
                            } else {
                                Space::with_width(0)
                            },
                            container(Space::with_width(16).height(16)).style(if locked {
                                switch_knob_disabled
                            } else if is_active {
                                switch_knob_active
                            } else {
                                switch_knob
                            }),
                            if is_active {
                                Space::with_width(0)
                            } else {
                                Space::with_width(Length::Fill)
                            },
                        ]
                        .align_y(alignment::Vertical::Center)
                    )
                    .padding(3)
                    .width(42)
                    .height(22)
                    .style(if locked {
                        switch_track_disabled
                    } else if is_active {
                        if is_hovered {
                            switch_track_active_hovered
                        } else {
                            switch_track_active
                        }
                    } else if is_hovered {
                        switch_track_hovered
                    } else {
                        switch_track
                    })
                )
                .on_enter(on_enter)
                .on_exit(on_exit)
                .on_press(if locked { Message::Noop } else { on_press }),
            ]
            .align_y(alignment::Vertical::Center),
        )
        .padding([8, 0])
        .width(Length::Fill)
        .style(switch_row)
        .into()
    }

    fn dropdown<'a>(&'a self, selected: &'a str, toggle: Message) -> Element<'a, Message> {
        let field = mouse_area(
            container(
                row![
                    text(truncate_text(selected, 36)).size(13),
                    Space::with_width(Length::Fill),
                    svg("assets/icons/chevrons-up-down.svg")
                        .width(15)
                        .height(15),
                ]
                .align_y(alignment::Vertical::Center),
            )
            .padding(10)
            .width(Length::Fill)
            .style(if self.is_config_locked() {
                input_surface_disabled
            } else {
                input_surface
            }),
        )
        .on_press(if self.is_config_locked() {
            Message::Noop
        } else {
            toggle
        });

        field.into()
    }

    fn dropdown_overlay(&self, kind: NodeKind) -> Option<Element<Message>> {
        match kind {
            NodeKind::Output if self.is_format_dropdown_open => {
                Some(self.dropdown_menu(&self.formats, Message::SelectFormat))
            }
            NodeKind::Screen if self.is_monitor_dropdown_open => {
                Some(self.dropdown_menu(&self.monitors, Message::SelectMonitor))
            }
            NodeKind::Mic if self.is_mic_dropdown_open => {
                Some(self.dropdown_menu(&self.mics, Message::SelectMic))
            }
            _ => None,
        }
    }

    fn dropdown_menu<'a>(
        &'a self,
        options: &'a [String],
        on_select: fn(String) -> Message,
    ) -> Element<'a, Message> {
        let options =
            options
                .iter()
                .enumerate()
                .fold(column![].spacing(0), |column, (index, option)| {
                    column.push(
                        mouse_area(
                            container(text(option).size(13))
                                .padding([9, 10])
                                .width(Length::Fill)
                                .style(if self.hovered_dropdown_option == Some(index) {
                                    dropdown_option_hovered
                                } else {
                                    dropdown_option
                                }),
                        )
                        .on_enter(Message::HoverDropdownOption(Some(index)))
                        .on_exit(Message::HoverDropdownOption(None))
                        .on_press(on_select(option.clone())),
                    )
                });

        container(options)
            .padding([4, 0])
            .width(Length::Fill)
            .style(dropdown_menu_surface)
            .into()
    }
}

fn connector_svg_handle() -> svg::Handle {
    let center_x = GRAPH_WIDTH / 2.0;
    let output_y = NODE_CARD_HEIGHT;
    let lower_y = NODE_CARD_HEIGHT + NODE_COLUMN_SPACING;
    let split_y = output_y + (NODE_COLUMN_SPACING * 0.45);
    let screen_x = NODE_CARD_WIDTH / 2.0;
    let mic_x = NODE_CARD_WIDTH + NODE_ROW_SPACING + (NODE_CARD_WIDTH / 2.0);

    let markup = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <path
    d="M {cx} {output_y}
       L {cx} {split_y}
       M {cx} {split_y}
       C {cx} {left_c1_y}, {screen_x} {left_c2_y}, {screen_x} {lower_y}
       M {cx} {split_y}
       C {cx} {right_c1_y}, {mic_x} {right_c2_y}, {mic_x} {lower_y}"
    fill="none"
    stroke="#f4f2eb"
    stroke-opacity="0.48"
    stroke-width="2.25"
    stroke-linecap="round"
    stroke-linejoin="round"
    shape-rendering="geometricPrecision"
  />
</svg>"##,
        width = GRAPH_WIDTH,
        height = GRAPH_HEIGHT,
        cx = center_x,
        output_y = output_y,
        split_y = split_y,
        screen_x = screen_x,
        mic_x = mic_x,
        lower_y = lower_y,
        left_c1_y = split_y + 18.0,
        left_c2_y = lower_y - 18.0,
        right_c1_y = split_y + 18.0,
        right_c2_y = lower_y - 18.0,
    );

    svg::Handle::from_memory(markup.into_bytes())
}

fn panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(34, 34, 32).into()),
        text_color: Some(Color::from_rgb8(224, 222, 216)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

fn sidebar(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(31, 31, 29).into()),
        text_color: Some(Color::from_rgb8(224, 222, 216)),
        border: border::width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(30, 30, 28).into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_close(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(214, 212, 205)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_close_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(50, 50, 47).into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(0).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_icon_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_menu_surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(47, 47, 44).into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(9).width(0),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
    }
}

fn titlebar_menu_item(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(6).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_menu_item_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(62, 62, 58).into()),
        text_color: Some(Color::from_rgb8(246, 244, 238)),
        border: border::rounded(6).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_menu_separator(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(74, 74, 70).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(1).width(0),
        shadow: Shadow::default(),
    }
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

fn scrim(progress: f32) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.42 * progress).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn modal_panel(progress: f32) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(42, 42, 39).into()),
        text_color: Some(Color::from_rgb8(235, 233, 226)),
        border: border::rounded(8).width(0),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35 * progress),
            offset: iced::Vector::new(0.0, 10.0 * progress),
            blur_radius: 24.0 * progress,
        },
    }
}

fn record_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(47, 94, 158).into()),
        text_color: Some(Color::from_rgb8(232, 242, 255)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn record_button_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(37, 79, 138).into()),
        text_color: Some(Color::from_rgb8(232, 242, 255)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn stop_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(170, 43, 48).into()),
        text_color: Some(Color::from_rgb8(255, 235, 235)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn stop_button_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(142, 34, 40).into()),
        text_color: Some(Color::from_rgb8(255, 235, 235)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn side_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(47, 47, 44).into()),
        text_color: Some(Color::from_rgb8(230, 228, 220)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn side_button_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(58, 58, 54).into()),
        text_color: Some(Color::from_rgb8(240, 238, 230)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn pause_button_active(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(158, 122, 38).into()),
        text_color: Some(Color::from_rgb8(255, 245, 210)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn pause_button_active_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(136, 101, 28).into()),
        text_color: Some(Color::from_rgb8(255, 245, 210)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn pause_button_blink_off(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(112, 86, 24).into()),
        text_color: Some(Color::from_rgb8(255, 245, 210)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn input_surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(52, 52, 49).into()),
        text_color: Some(Color::from_rgb8(214, 212, 205)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

fn input_surface_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(45, 45, 42).into()),
        text_color: Some(Color::from_rgb8(138, 136, 130)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

fn control_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(43, 43, 40).into()),
        text_color: Some(Color::from_rgb8(126, 124, 118)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_row(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_track(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(68, 68, 63).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_track_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(82, 82, 76).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_track_active(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(47, 94, 158).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_track_active_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(37, 79, 138).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_track_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(50, 50, 46).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_knob(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(218, 216, 208).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_knob_active(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(238, 246, 255).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_knob_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(120, 118, 112).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn dropdown_menu_surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(52, 52, 49).into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(7).width(0),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
    }
}

fn dropdown_option(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(5).width(0),
        shadow: Shadow::default(),
    }
}

fn dropdown_option_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(66, 66, 62).into()),
        text_color: Some(Color::from_rgb8(246, 244, 238)),
        border: border::rounded(5).width(0),
        shadow: Shadow::default(),
    }
}

fn node_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(45, 45, 42).into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
    }
}

fn selected_mode_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(54, 59, 64).into()),
        text_color: Some(Color::from_rgb8(235, 242, 250)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
    }
}

fn mode_card_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(43, 43, 40).into()),
        text_color: Some(Color::from_rgb8(132, 130, 124)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
    }
}

fn node_card_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(50, 50, 47).into()),
        text_color: Some(Color::from_rgb8(244, 242, 235)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
    }
}

fn node_card_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(41, 41, 38).into()),
        text_color: Some(Color::from_rgb8(150, 148, 140)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
    }
}

fn ghost_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(38, 38, 35).into()),
        text_color: Some(Color::from_rgb8(145, 143, 136)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn ghost_card_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(46, 46, 42).into()),
        text_color: Some(Color::from_rgb8(190, 188, 179)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn pill(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(43, 43, 40).into()),
        text_color: Some(Color::from_rgb8(210, 208, 200)),
        border: border::rounded(7)
            .color(Color::from_rgb8(76, 75, 70))
            .width(1),
        shadow: Shadow::default(),
    }
}

fn pill_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(55, 55, 51).into()),
        text_color: Some(Color::from_rgb8(226, 224, 216)),
        border: border::rounded(7)
            .color(Color::from_rgb8(76, 75, 70))
            .width(1),
        shadow: Shadow::default(),
    }
}

fn notify_recording_completed(save_path: String, video_path: Option<PathBuf>) {
    thread::spawn(move || {
        let thumbnail_path = video_path
            .as_deref()
            .and_then(create_video_thumbnail)
            .map(|path| path.to_string_lossy().to_string());

        let mut notification = Notification::new();
        notification
            .summary("Recording completed!")
            .body("Click to open in file manager")
            .icon("video-x-generic")
            .action("open", "Open");

        if let Some(path) = thumbnail_path.as_deref() {
            notification.image_path(path);
        }

        let notification = notification.show();

        if let Ok(handle) = notification {
            handle.wait_for_action(|action| {
                if action == "default" || action == "open" {
                    let _ = Command::new("xdg-open").arg(&save_path).spawn();
                }
            });
        }
    });
}

fn create_video_thumbnail(video_path: &Path) -> Option<PathBuf> {
    let thumbnail_path = std::env::temp_dir().join(format!(
        "roton_thumbnail_{}.png",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%f")
    ));

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg("00:00:01")
        .arg("-i")
        .arg(video_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg("scale=320:-1")
        .arg(&thumbnail_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;

    status.success().then_some(thumbnail_path)
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut text = String::new();

    for _ in 0..max_chars {
        if let Some(ch) = chars.next() {
            text.push(ch);
        } else {
            return text;
        }
    }

    if chars.next().is_some() {
        text.push_str("...");
    }

    text
}

async fn pick_output_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Choose save folder")
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string())
}

async fn select_area() -> Option<String> {
    Command::new("slurp")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let area = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!area.is_empty()).then_some(area)
        })
}

fn create_tray_icon() -> Option<TrayIcon> {
    if !has_appindicator_runtime() {
        eprintln!("Tray disabled: libayatana-appindicator3/libappindicator3 is not installed");
        return None;
    }

    let icon = load_tray_icon("assets/rotonicon.png").ok()?;
    TrayIconBuilder::new()
        .with_tooltip("Roton")
        .with_icon(icon)
        .build()
        .ok()
}

fn has_appindicator_runtime() -> bool {
    [
        "/usr/lib/libayatana-appindicator3.so",
        "/usr/lib/libayatana-appindicator3.so.1",
        "/usr/lib/libappindicator3.so",
        "/usr/lib/libappindicator3.so.1",
        "/usr/local/lib/libayatana-appindicator3.so",
        "/usr/local/lib/libappindicator3.so",
    ]
    .iter()
    .any(|path| Path::new(path).exists())
}

fn load_tray_icon(path: &str) -> Result<Icon, String> {
    let image = ::image::open(path)
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).map_err(|error| error.to_string())
}

fn display_monitors() -> Vec<String> {
    DisplayInfo::all()
        .map(|displays| {
            displays
                .into_iter()
                .map(|display| {
                    let name = if display.friendly_name.is_empty() {
                        display.name
                    } else {
                        display.friendly_name
                    };
                    let primary = if display.is_primary {
                        "primary"
                    } else {
                        "display"
                    };
                    format!(
                        "{} · {}x{} · {}",
                        name, display.width, display.height, primary
                    )
                })
                .collect()
        })
        .unwrap_or_else(|_| vec!["Primary display".to_string()])
}
