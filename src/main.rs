mod audio;
mod components;
mod config;
mod recorder;

use audio::AudioDevice;
use components::button::{self as ui_button, Variant as ButtonVariant};
use components::dropdown::{self, Entry as DropdownEntry, OptionItem};
use components::overlay::event_blocker;
use components::{dialog, styles as component_styles, textbox};
use config::{Settings, Workspace};
use display_info::DisplayInfo;
use iced::widget::{
    button, column, container, image, mouse_area, row, stack, svg, text, text_input, Space,
};
use iced::{
    alignment, application, border, time, window, Color, Element, Length, Padding, Shadow,
    Subscription, Task, Theme,
};
use notify_rust::Notification;
use recorder::Recorder;
use std::fs;
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
const WORKSPACE_ACTIONS_MENU_LEFT: f32 = 154.0;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceNameAction {
    Create,
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceTheme {
    Blue,
    Green,
    Red,
    Purple,
    Amber,
}

impl WorkspaceTheme {
    const ALL: [Self; 5] = [
        Self::Blue,
        Self::Green,
        Self::Red,
        Self::Purple,
        Self::Amber,
    ];

    fn from_key(key: &str) -> Self {
        match key {
            "Green" => Self::Green,
            "Red" => Self::Red,
            "Purple" => Self::Purple,
            "Amber" => Self::Amber,
            _ => Self::Blue,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Purple => "Purple",
            Self::Amber => "Amber",
        }
    }

    fn label(self) -> &'static str {
        self.key()
    }

    fn accent(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(47, 94, 158),
            Self::Green => Color::from_rgb8(52, 132, 87),
            Self::Red => Color::from_rgb8(154, 58, 63),
            Self::Purple => Color::from_rgb8(118, 82, 168),
            Self::Amber => Color::from_rgb8(154, 106, 35),
        }
    }

    fn accent_hover(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(37, 79, 138),
            Self::Green => Color::from_rgb8(39, 108, 69),
            Self::Red => Color::from_rgb8(128, 47, 52),
            Self::Purple => Color::from_rgb8(98, 66, 144),
            Self::Amber => Color::from_rgb8(128, 87, 25),
        }
    }

    fn selected_surface(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(58, 63, 68),
            Self::Green => Color::from_rgb8(56, 72, 62),
            Self::Red => Color::from_rgb8(77, 56, 58),
            Self::Purple => Color::from_rgb8(68, 59, 82),
            Self::Amber => Color::from_rgb8(77, 67, 51),
        }
    }

    fn app_background(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(39, 39, 37),
            Self::Green => Color::from_rgb8(37, 41, 37),
            Self::Red => Color::from_rgb8(41, 37, 37),
            Self::Purple => Color::from_rgb8(39, 37, 41),
            Self::Amber => Color::from_rgb8(41, 39, 35),
        }
    }

    fn sidebar_background(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(36, 36, 34),
            Self::Green => Color::from_rgb8(34, 38, 34),
            Self::Red => Color::from_rgb8(38, 34, 34),
            Self::Purple => Color::from_rgb8(36, 34, 38),
            Self::Amber => Color::from_rgb8(38, 36, 32),
        }
    }

    fn titlebar_background(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(35, 35, 33),
            Self::Green => Color::from_rgb8(33, 37, 33),
            Self::Red => Color::from_rgb8(37, 33, 33),
            Self::Purple => Color::from_rgb8(35, 33, 37),
            Self::Amber => Color::from_rgb8(37, 35, 31),
        }
    }

    fn surface(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(50, 50, 47),
            Self::Green => Color::from_rgb8(48, 53, 48),
            Self::Red => Color::from_rgb8(53, 48, 48),
            Self::Purple => Color::from_rgb8(51, 48, 55),
            Self::Amber => Color::from_rgb8(53, 51, 45),
        }
    }

    fn surface_hover(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(57, 57, 53),
            Self::Green => Color::from_rgb8(55, 61, 55),
            Self::Red => Color::from_rgb8(62, 55, 55),
            Self::Purple => Color::from_rgb8(59, 55, 64),
            Self::Amber => Color::from_rgb8(62, 59, 52),
        }
    }

    fn component_palette(self) -> component_styles::Palette {
        component_styles::Palette {
            panel: self.dialog_surface(),
            field: self.surface(),
            field_hover: self.surface_hover(),
            field_disabled: self.disabled_surface(),
            menu: self.surface(),
            option_hover: self.surface_hover(),
            option_selected: self.selected_surface(),
            option_selected_hover: self.selected_surface_hover(),
            separator: self.separator(),
            text: Color::from_rgb8(214, 212, 205),
            text_strong: Color::from_rgb8(246, 244, 238),
            text_muted: Color::from_rgb8(176, 174, 166),
            text_disabled: Color::from_rgb8(126, 124, 118),
            selection: self.accent(),
        }
    }

    fn dialog_surface(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(42, 42, 39),
            Self::Green => Color::from_rgb8(39, 46, 40),
            Self::Red => Color::from_rgb8(46, 39, 39),
            Self::Purple => Color::from_rgb8(42, 39, 47),
            Self::Amber => Color::from_rgb8(47, 43, 36),
        }
    }

    fn disabled_surface(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(45, 45, 42),
            Self::Green => Color::from_rgb8(42, 47, 42),
            Self::Red => Color::from_rgb8(47, 42, 42),
            Self::Purple => Color::from_rgb8(45, 42, 49),
            Self::Amber => Color::from_rgb8(48, 45, 39),
        }
    }

    fn selected_surface_hover(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(67, 74, 82),
            Self::Green => Color::from_rgb8(65, 84, 70),
            Self::Red => Color::from_rgb8(88, 65, 67),
            Self::Purple => Color::from_rgb8(78, 68, 94),
            Self::Amber => Color::from_rgb8(88, 76, 58),
        }
    }

    fn separator(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(74, 74, 70),
            Self::Green => Color::from_rgb8(68, 82, 70),
            Self::Red => Color::from_rgb8(86, 68, 68),
            Self::Purple => Color::from_rgb8(76, 68, 88),
            Self::Amber => Color::from_rgb8(84, 76, 61),
        }
    }

    fn connector_hex(self) -> &'static str {
        match self {
            Self::Blue => "#8fbcff",
            Self::Green => "#8fe0ac",
            Self::Red => "#ff9b9f",
            Self::Purple => "#c3a2ff",
            Self::Amber => "#ffd188",
        }
    }
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
    OpenAboutRoton,
    CloseAboutRoton,
    ToggleMinimalWindow,
    ToggleMinimizeToTray,
    ConfirmClose,
    CancelClose,
    OpenNode(NodeKind),
    HoverNode(Option<NodeKind>),
    HoverModalClose(bool),
    HoverRecord(bool),
    HoverPause(bool),
    HoverScreenMode(Option<ScreenMode>),
    HoverMicToggle(bool),
    HoverShowCursor(bool),
    HoverScreenSound(bool),
    PauseBlinkTick,
    RecordingTick,
    Frame,
    CloseModal,
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
    ToggleWorkspaceActions,
    SelectWorkspace(usize),
    SelectWorkspaceTheme(WorkspaceTheme),
    OpenCreateWorkspace,
    OpenRenameWorkspace,
    DeleteWorkspace,
    WorkspaceNameChanged(String),
    ConfirmWorkspaceName,
    CancelWorkspaceName,
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
    workspace_theme: WorkspaceTheme,
    is_workspace_actions_open: bool,
    workspace_name_action: Option<WorkspaceNameAction>,
    workspace_name: String,
    workspace_name_error: Option<String>,
    audio_devices: Vec<AudioDevice>,
    formats: Vec<String>,
    selected_format: String,
    monitors: Vec<String>,
    selected_monitor: Option<String>,
    mics: Vec<String>,
    selected_mic: Option<String>,
    mic_mode: AudioMode,
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
    is_window_maximized: bool,
    is_minimal_window: bool,
    show_about_dialog: bool,
    show_close_confirmation: bool,
    is_modal_close_hovered: bool,
    is_record_hovered: bool,
    is_pause_hovered: bool,
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
    has_scrop: bool,
    has_ffmpeg: bool,
    has_pactl: bool,
}

impl Roton {
    fn new() -> Self {
        let mut app = Self {
            recorder: Arc::new(Mutex::new(Recorder::new())),
            settings: Settings::load(),
            workspace_theme: WorkspaceTheme::Blue,
            is_workspace_actions_open: false,
            workspace_name_action: None,
            workspace_name: String::new(),
            workspace_name_error: None,
            audio_devices: Vec::new(),
            formats: vec!["MP4".to_string(), "MKV".to_string(), "WEBM".to_string()],
            selected_format: "MP4".to_string(),
            monitors: display_monitors(),
            selected_monitor: None,
            mics: Vec::new(),
            selected_mic: None,
            mic_mode: AudioMode::Mute,
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
            is_window_maximized: false,
            is_minimal_window: false,
            show_about_dialog: false,
            show_close_confirmation: false,
            is_modal_close_hovered: false,
            is_record_hovered: false,
            is_pause_hovered: false,
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
            has_scrop: Recorder::is_installed("scrop"),
            has_ffmpeg: Recorder::is_installed("ffmpeg"),
            has_pactl: Recorder::is_installed("pactl"),
        };
        app.refresh_audio_devices();
        app.selected_monitor = app.monitors.first().cloned();
        app.apply_active_workspace();
        app
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleRecord => {
                self.close_workspace_menus();
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
                self.is_window_maximized = maximized;
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
                self.close_workspace_menus();
                self.is_titlebar_menu_open = !self.is_titlebar_menu_open;
                self.hovered_titlebar_menu_item = None;
            }
            Message::HoverTitlebarMenuItem(item) => {
                self.hovered_titlebar_menu_item = item;
            }
            Message::HoverTitlebarAction(action) => {
                self.hovered_titlebar_action = action;
            }
            Message::OpenAboutRoton => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.show_about_dialog = true;
                self.is_modal_close_hovered = false;
            }
            Message::CloseAboutRoton => {
                self.show_about_dialog = false;
                self.is_modal_close_hovered = false;
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
                self.close_workspace_menus();
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
                self.modal_progress = 0.0;
            }
            Message::SelectFormat(format) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.selected_format = format;
                self.hovered_node = None;
                self.persist_workspace_state();
            }
            Message::SelectMonitor(monitor) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.selected_monitor = Some(monitor);
                self.hovered_node = None;
                self.persist_workspace_state();
            }
            Message::SelectMic(mic) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.selected_mic = Some(mic);
                self.persist_workspace_state();
            }
            Message::SelectMicMode(mode) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.mic_mode = mode;
                self.persist_workspace_state();
            }
            Message::SelectScreenMode(mode) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.screen_mode = mode;
                self.persist_workspace_state();
            }
            Message::SelectArea => {
                if self.is_config_locked() {
                    return Task::none();
                }
                if !self.has_scrop {
                    eprintln!("scrop is not installed");
                    return Task::none();
                }
                self.screen_mode = ScreenMode::SelectArea;
                self.persist_workspace_state();
                return Task::perform(select_area(), Message::AreaSelected);
            }
            Message::AreaSelected(area) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                if let Some(area) = area {
                    self.selected_area = Some(area);
                    self.persist_workspace_state();
                }
            }
            Message::ToggleShowCursor => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.show_cursor = !self.show_cursor;
                self.persist_workspace_state();
            }
            Message::ToggleScreenSound => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.record_screen_sound = !self.record_screen_sound;
                self.persist_workspace_state();
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
                    self.settings.active_workspace_mut().save_path = path;
                    self.persist_workspace_state();
                }
            }
            Message::ToggleWorkspaceActions => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.is_workspace_actions_open = !self.is_workspace_actions_open;
            }
            Message::SelectWorkspace(index) => {
                if self.is_config_locked() || index >= self.settings.workspaces.len() {
                    return Task::none();
                }
                self.persist_workspace_state();
                self.settings.active_workspace = index;
                self.apply_active_workspace();
                self.close_workspace_menus();
                self.save_settings();
            }
            Message::SelectWorkspaceTheme(theme) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.workspace_theme = theme;
                self.persist_workspace_state();
            }
            Message::OpenCreateWorkspace => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.close_workspace_menus();
                self.workspace_name_action = Some(WorkspaceNameAction::Create);
                self.workspace_name.clear();
                self.workspace_name_error = None;
                return text_input::focus("workspace-name");
            }
            Message::OpenRenameWorkspace => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.close_workspace_menus();
                self.workspace_name_action = Some(WorkspaceNameAction::Rename);
                self.workspace_name = self.settings.active_workspace().name.clone();
                self.workspace_name_error = None;
                return Task::batch([
                    text_input::focus("workspace-name"),
                    text_input::select_all("workspace-name"),
                ]);
            }
            Message::DeleteWorkspace => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.delete_active_workspace();
            }
            Message::WorkspaceNameChanged(name) => {
                self.workspace_name = name;
                self.workspace_name_error = None;
            }
            Message::ConfirmWorkspaceName => {
                self.confirm_workspace_name();
            }
            Message::CancelWorkspaceName => {
                self.workspace_name_action = None;
                self.workspace_name.clear();
                self.workspace_name_error = None;
            }
            Message::Noop => {}
        }

        Task::none()
    }

    fn close_workspace_menus(&mut self) {
        self.is_workspace_actions_open = false;
    }

    fn apply_active_workspace(&mut self) {
        let workspace = self.settings.active_workspace().clone();
        self.selected_format = workspace.selected_format;
        self.selected_monitor = workspace
            .selected_monitor
            .filter(|monitor| self.monitors.contains(monitor))
            .or_else(|| self.monitors.first().cloned());
        self.selected_mic = workspace
            .selected_mic
            .filter(|mic| self.mics.contains(mic))
            .or_else(|| self.mics.first().cloned());
        self.mic_mode = if workspace.mic_mode == "Mic" {
            AudioMode::Mic
        } else {
            AudioMode::Mute
        };
        self.screen_mode = if workspace.screen_mode == "SelectArea" {
            ScreenMode::SelectArea
        } else {
            ScreenMode::Fullscreen
        };
        self.selected_area = workspace.selected_area;
        self.show_cursor = workspace.show_cursor;
        self.record_screen_sound = workspace.record_screen_sound;
        self.workspace_theme = WorkspaceTheme::from_key(&workspace.theme);
    }

    fn persist_workspace_state(&mut self) {
        let workspace = self.settings.active_workspace_mut();
        workspace.selected_format = self.selected_format.clone();
        workspace.selected_monitor = self.selected_monitor.clone();
        workspace.selected_mic = self.selected_mic.clone();
        workspace.mic_mode = match self.mic_mode {
            AudioMode::Mute => "Mute",
            AudioMode::Mic => "Mic",
        }
        .to_string();
        workspace.screen_mode = match self.screen_mode {
            ScreenMode::Fullscreen => "Fullscreen",
            ScreenMode::SelectArea => "SelectArea",
        }
        .to_string();
        workspace.selected_area = self.selected_area.clone();
        workspace.show_cursor = self.show_cursor;
        workspace.record_screen_sound = self.record_screen_sound;
        workspace.theme = self.workspace_theme.key().to_string();
        self.save_settings();
    }

    fn save_settings(&self) {
        if let Err(error) = self.settings.save() {
            eprintln!("Failed to save settings: {error}");
        }
    }

    fn confirm_workspace_name(&mut self) {
        let name = self.workspace_name.trim().to_string();
        let Some(action) = self.workspace_name_action else {
            return;
        };

        if let Some(error) = self.validate_workspace_name(&name, action) {
            self.workspace_name_error = Some(error);
            return;
        }

        match action {
            WorkspaceNameAction::Create => {
                let workspace = Workspace::named(&name);
                if let Err(error) = fs::create_dir_all(&workspace.save_path) {
                    self.workspace_name_error =
                        Some(format!("Could not create workspace folder: {error}"));
                    return;
                }
                self.persist_workspace_state();
                self.settings.workspaces.push(workspace);
                self.settings.active_workspace = self.settings.workspaces.len() - 1;
                self.apply_active_workspace();
            }
            WorkspaceNameAction::Rename => {
                self.settings.active_workspace_mut().name = name;
            }
        }

        self.save_settings();
        self.workspace_name_action = None;
        self.workspace_name.clear();
        self.workspace_name_error = None;
    }

    fn validate_workspace_name(&self, name: &str, action: WorkspaceNameAction) -> Option<String> {
        if name.is_empty() {
            return Some("Workspace name cannot be empty.".to_string());
        }

        if name.chars().count() > 48 {
            return Some("Workspace name must be 48 characters or fewer.".to_string());
        }

        if name == "."
            || name == ".."
            || name
                .chars()
                .any(|character| character == '/' || character == '\\' || character.is_control())
        {
            return Some("Workspace name cannot contain path separators.".to_string());
        }

        let duplicate = self
            .settings
            .workspaces
            .iter()
            .enumerate()
            .any(|(index, workspace)| {
                let is_active_rename = action == WorkspaceNameAction::Rename
                    && index == self.settings.active_workspace;
                !is_active_rename && workspace.name.eq_ignore_ascii_case(name)
            });

        duplicate.then(|| "A workspace with that name already exists.".to_string())
    }

    fn can_delete_active_workspace(&self) -> bool {
        self.settings.workspaces.len() > 1 && self.settings.active_workspace != 0
    }

    fn delete_active_workspace(&mut self) {
        if !self.can_delete_active_workspace() {
            self.close_workspace_menus();
            return;
        }

        self.settings
            .workspaces
            .remove(self.settings.active_workspace);
        self.settings.active_workspace = self
            .settings
            .active_workspace
            .min(self.settings.workspaces.len() - 1);
        self.apply_active_workspace();
        self.close_workspace_menus();
        self.save_settings();
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
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create output folder: {error}"))?;
        }
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
        self.elapsed = 0;
        notify_recording_completed(
            self.settings.active_workspace().save_path.clone(),
            video_path,
        );
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
        Path::new(&self.settings.active_workspace().save_path).join(filename)
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
        subscriptions
            .push(time::every(std::time::Duration::from_millis(350)).map(|_| Message::TrayTick));

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
            .style({
                let theme = self.workspace_theme;
                move |_| panel(theme)
            });

        let app: Element<_> = if self.is_titlebar_menu_open {
            stack![
                app,
                event_blocker(
                    mouse_area(
                        container(Space::with_width(Length::Fill).height(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fill)
                    )
                    .on_press(Message::ToggleTitlebarMenu)
                ),
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

        let app: Element<_> = if self.is_workspace_actions_open {
            stack![
                app,
                event_blocker(
                    mouse_area(
                        container(Space::with_width(Length::Fill).height(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fill)
                    )
                    .on_press(Message::ToggleWorkspaceActions)
                ),
                container(container(self.workspace_actions_menu()).width(172))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(Padding::default().top(98).left(WORKSPACE_ACTIONS_MENU_LEFT))
                    .align_x(alignment::Horizontal::Left)
                    .align_y(alignment::Vertical::Top)
            ]
            .into()
        } else {
            app
        };

        if self.workspace_name_action.is_some() {
            stack![
                app,
                dialog::backdrop(1.0, Message::CancelWorkspaceName),
                container(mouse_area(self.workspace_name_modal()).on_press(Message::Noop))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if self.show_close_confirmation {
            stack![
                app,
                dialog::backdrop(1.0, Message::Noop),
                container(self.close_confirmation())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if self.show_about_dialog {
            stack![
                app,
                dialog::backdrop(1.0, Message::CloseAboutRoton),
                container(mouse_area(self.about_dialog()).on_press(Message::Noop))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if let Some(kind) = self.selected_node {
            let progress = ease_out(self.modal_progress);
            stack![
                app,
                dialog::backdrop(progress, Message::CloseModal),
                container(mouse_area(self.modal(kind, progress)).on_press(Message::Noop))
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
        let palette = self.workspace_theme.component_palette();

        dialog::panel(
            column![
                text("Stop Recording?").size(18),
                text("Roton is still recording. Closing now will stop and save the recording.")
                    .size(13),
                row![
                    ui_button::themed_text_button(
                        "Cancel",
                        ButtonVariant::Secondary,
                        Some(Message::CancelClose),
                        palette,
                    ),
                    Space::with_width(Length::Fill),
                    ui_button::themed_text_button(
                        "Stop and Close",
                        ButtonVariant::Danger,
                        Some(Message::ConfirmClose),
                        palette,
                    ),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(16),
            420.0,
            1.0,
            palette,
        )
    }

    fn about_dialog(&self) -> Element<Message> {
        let theme = self.workspace_theme;
        let palette = theme.component_palette();
        let close_style = {
            let is_hovered = self.is_modal_close_hovered;
            move |iced_theme: &Theme| {
                if is_hovered {
                    ghost_card_hovered(theme)
                } else {
                    ghost_card(iced_theme, theme)
                }
            }
        };
        let detail_row = |label: &'static str, value: &'static str| {
            row![
                text(label).size(12).color(palette.text_muted),
                Space::with_width(Length::Fill),
                text(value).size(12).color(palette.text),
            ]
            .align_y(alignment::Vertical::Center)
        };

        dialog::panel(
            column![
                row![
                    image("assets/rotonicon.png").width(44).height(44),
                    column![
                        text("Roton").size(21),
                        text("Screen recorder").size(12).color(palette.text_muted),
                    ]
                    .spacing(4),
                    Space::with_width(Length::Fill),
                    mouse_area(
                        container(svg("assets/icons/x.svg").width(16).height(16))
                            .padding(8)
                            .style(close_style)
                    )
                    .on_enter(Message::HoverModalClose(true))
                    .on_exit(Message::HoverModalClose(false))
                    .on_press(Message::CloseAboutRoton),
                ]
                .spacing(12)
                .align_y(alignment::Vertical::Center),
                container(
                    container(Space::with_height(1))
                        .width(Length::Fill)
                        .style(move |_| component_styles::separator_with_palette(palette))
                )
                .padding([2, 0])
                .width(Length::Fill),
                column![
                    detail_row("Version", APP_VERSION),
                    detail_row("Made by", "Ferdinan Iydheko"),
                ]
                .spacing(9),
                text("Built for quick workspace-based screen recording with monitor, area, cursor, screen-audio, and microphone controls.")
                    .size(12)
                    .color(palette.text_muted),
                row![
                    Space::with_width(Length::Fill),
                    ui_button::themed_text_button(
                        "Close",
                        ButtonVariant::Secondary,
                        Some(Message::CloseAboutRoton),
                        palette,
                    ),
                ]
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(14),
            430.0,
            1.0,
            palette,
        )
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
        let record_theme = self.workspace_theme;
        let is_recording = self.is_recording;
        let is_record_hovered = self.is_record_hovered;
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
            .style(move |iced_theme| {
                if is_recording {
                    if is_record_hovered {
                        stop_button_hovered(iced_theme)
                    } else {
                        stop_button(iced_theme)
                    }
                } else if is_record_hovered {
                    record_button_hovered(record_theme)
                } else {
                    record_button(record_theme)
                }
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

        let status_row = row![
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
        .align_y(alignment::Vertical::Center);

        let sidebar = container(
            column![
                self.workspace_selector(),
                controls,
                status_row,
                Space::with_height(Length::Fill),
                column![
                    text(format!("Roton v{APP_VERSION}")).size(12),
                    text("By Ferdinan Iydheko").size(11),
                ]
                .spacing(4),
            ]
            .spacing(16),
        )
        .width(210)
        .height(Length::Fill)
        .padding(18)
        .style({
            let theme = self.workspace_theme;
            move |_| sidebar(theme)
        });

        sidebar.into()
    }

    fn workspace_selector(&self) -> Element<Message> {
        let locked = self.is_config_locked();

        row![
            dropdown::select(
                self.settings.active_workspace().name.clone(),
                self.settings
                    .workspaces
                    .iter()
                    .enumerate()
                    .map(|(index, workspace)| {
                        DropdownEntry::from(OptionItem::new(
                            &workspace.name,
                            Message::SelectWorkspace(index),
                        ))
                    })
                    .chain([
                        DropdownEntry::Separator,
                        DropdownEntry::from(OptionItem::new(
                            "+ New Workspace",
                            Message::OpenCreateWorkspace,
                        )),
                    ]),
                16,
                locked,
                self.workspace_theme.component_palette(),
            ),
            ui_button::themed_compact_icon_button(
                "assets/icons/ellipsis-vertical.svg",
                ButtonVariant::Side,
                (!locked).then_some(Message::ToggleWorkspaceActions),
                self.workspace_theme.component_palette(),
            ),
        ]
        .spacing(8)
        .align_y(alignment::Vertical::Center)
        .into()
    }

    fn workspace_actions_menu(&self) -> Element<Message> {
        let palette = self.workspace_theme.component_palette();
        let menu_item = |label: &'static str, on_press: Option<Message>| {
            button(
                text(label)
                    .size(13)
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Left),
            )
            .padding([9, 10])
            .width(Length::Fill)
            .style(move |_, status| {
                component_styles::context_menu_option_with_palette(status, palette)
            })
            .on_press_maybe(on_press)
        };

        let theme_swatches = WorkspaceTheme::ALL
            .into_iter()
            .fold(row![].spacing(7), |row, theme| {
                row.push(self.theme_swatch(theme))
            });

        let items = column![
            container(
                row![
                    text("Color Theme").size(12).color(palette.text_muted),
                    Space::with_width(Length::Fill),
                    text(self.workspace_theme.label())
                        .size(12)
                        .color(palette.text),
                ]
                .width(Length::Fill)
                .align_y(alignment::Vertical::Center),
            )
            .padding(Padding::default().top(6).right(10).bottom(7).left(10))
            .width(Length::Fill),
            container(theme_swatches)
                .padding(Padding::default().right(10).bottom(8).left(10))
                .width(Length::Fill),
            container(
                container(Space::with_height(1))
                    .width(Length::Fill)
                    .style(move |_| component_styles::separator_with_palette(palette))
            )
            .padding([3, 0])
            .width(Length::Fill),
            menu_item("Rename", Some(Message::OpenRenameWorkspace)),
        ]
        .spacing(0);

        let items = if self.can_delete_active_workspace() {
            items.push(menu_item("Delete", Some(Message::DeleteWorkspace)))
        } else {
            items
        };

        event_blocker(
            container(items)
                .padding([4, 0])
                .width(Length::Fill)
                .style(move |_| component_styles::context_menu_with_palette(palette)),
        )
    }

    fn theme_swatch(&self, theme: WorkspaceTheme) -> Element<'_, Message> {
        let is_selected = self.workspace_theme == theme;
        let marker: Element<_> = if is_selected {
            svg("assets/icons/check.svg").width(13).height(13).into()
        } else {
            Space::with_width(13).height(13).into()
        };

        mouse_area(
            container(marker)
                .width(24)
                .height(24)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .style(move |_| theme_swatch_style(theme, is_selected)),
        )
        .on_press(Message::SelectWorkspaceTheme(theme))
        .into()
    }

    fn workspace_name_modal(&self) -> Element<Message> {
        let is_create = self.workspace_name_action == Some(WorkspaceNameAction::Create);
        let title = if is_create {
            "New Workspace"
        } else {
            "Rename Workspace"
        };
        let description = if is_create {
            "Recordings start in a dedicated folder under your Videos directory."
        } else {
            "Renaming keeps the existing recordings folder unchanged."
        };
        let confirm_label = if is_create { "Create" } else { "Rename" };
        let error: Element<_> = self
            .workspace_name_error
            .as_deref()
            .map(|error| {
                text(error)
                    .size(12)
                    .color(Color::from_rgb8(232, 126, 126))
                    .into()
            })
            .unwrap_or_else(|| Space::with_height(0).into());

        dialog::panel(
            column![
                text(title).size(18),
                text(description).size(13),
                textbox::field(
                    "Workspace name",
                    &self.workspace_name,
                    "workspace-name",
                    Message::WorkspaceNameChanged,
                    Message::ConfirmWorkspaceName,
                    self.workspace_theme.component_palette(),
                ),
                error,
                row![
                    Space::with_width(Length::Fill),
                    ui_button::themed_text_button(
                        "Cancel",
                        ButtonVariant::Secondary,
                        Some(Message::CancelWorkspaceName),
                        self.workspace_theme.component_palette(),
                    ),
                    ui_button::accent_text_button(
                        confirm_label,
                        Some(Message::ConfirmWorkspaceName),
                        self.workspace_theme.accent(),
                        self.workspace_theme.accent_hover(),
                    ),
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(12),
            420.0,
            1.0,
            self.workspace_theme.component_palette(),
        )
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
        .style({
            let theme = self.workspace_theme;
            move |_| titlebar(theme)
        })
        .into()
    }

    fn titlebar_menu(&self) -> Element<Message> {
        let theme = self.workspace_theme;
        let palette = theme.component_palette();
        let item = |index, label: &'static str, message| {
            mouse_area(
                container(
                    row![Space::with_width(25), text(label).size(13)]
                        .align_y(alignment::Vertical::Center),
                )
                .padding([9, 10])
                .width(Length::Fill)
                .style({
                    let is_hovered = self.hovered_titlebar_menu_item == Some(index);
                    move |_| {
                        if is_hovered {
                            titlebar_menu_item_hovered(theme)
                        } else {
                            titlebar_menu_item(theme)
                        }
                    }
                }),
            )
            .on_enter(Message::HoverTitlebarMenuItem(Some(index)))
            .on_exit(Message::HoverTitlebarMenuItem(None))
            .on_press(message)
        };

        let separator = || {
            container(
                container(Space::with_height(1))
                    .width(Length::Fill)
                    .style(move |_| component_styles::separator_with_palette(palette)),
            )
            .padding([3, 0])
            .width(Length::Fill)
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
            .style({
                let is_hovered = self.hovered_titlebar_menu_item == Some(3);
                move |_| {
                    if is_hovered {
                        titlebar_menu_item_hovered(theme)
                    } else {
                        titlebar_menu_item(theme)
                    }
                }
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
            .style({
                let is_hovered = self.hovered_titlebar_menu_item == Some(4);
                move |_| {
                    if is_hovered {
                        titlebar_menu_item_hovered(theme)
                    } else {
                        titlebar_menu_item(theme)
                    }
                }
            }),
        )
        .on_enter(Message::HoverTitlebarMenuItem(Some(4)))
        .on_exit(Message::HoverTitlebarMenuItem(None))
        .on_press(Message::ToggleMinimizeToTray);

        let maximize_label = if self.is_window_maximized {
            "Restore"
        } else {
            "Maximize"
        };

        mouse_area(
            container(
                column![
                    item(0, "Close", Message::CloseWindow),
                    item(1, maximize_label, Message::MaximizeWindow),
                    item(2, "Minimize", Message::MinimizeWindow),
                    separator(),
                    minimal_item,
                    tray_item,
                    separator(),
                    item(5, "About Roton", Message::OpenAboutRoton),
                ]
                .spacing(0),
            )
            .padding(4)
            .width(190)
            .style(move |_| titlebar_menu_surface(theme)),
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
            svg(connector_svg_handle(self.workspace_theme))
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
        let icon = if kind == NodeKind::Mic && self.mic_mode == AudioMode::Mute {
            "assets/icons/mic-off.svg"
        } else {
            kind.icon()
        };
        let theme = self.workspace_theme;
        let locked = self.is_config_locked();
        let is_hovered = self.selected_node.is_none() && self.hovered_node == Some(kind);
        mouse_area(
            container(
                container(
                    column![
                        svg(icon).width(26).height(26),
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
            .style(move |iced_theme| {
                if locked {
                    node_card_disabled(iced_theme)
                } else if is_hovered {
                    node_card_hovered(theme)
                } else {
                    node_card(theme)
                }
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
                self.dropdown(&self.selected_format, &self.formats, Message::SelectFormat),
                text("Save Folder").size(13),
                row![
                    textbox::readonly(
                        truncate_text(&self.settings.active_workspace().save_path, 38),
                        self.workspace_theme.component_palette(),
                    ),
                    ui_button::themed_text_button(
                        "Choose",
                        ButtonVariant::Secondary,
                        (!self.is_config_locked()).then_some(Message::ChooseOutputFolder),
                        self.workspace_theme.component_palette(),
                    ),
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
                    &self.monitors,
                    Message::SelectMonitor,
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
                    Element::<Message>::from(ui_button::accent_fill_text_button(
                        "Select Area",
                        (!self.is_config_locked()).then_some(Message::SelectArea),
                        self.workspace_theme.accent(),
                        self.workspace_theme.accent_hover(),
                    ))
                } else {
                    Element::<Message>::from(
                        mouse_area(container(Space::with_height(0))).on_press(Message::Noop),
                    )
                },
            ]
            .spacing(12)
            .into(),
            NodeKind::Mic => column![
                text("Microphone").size(13),
                self.dropdown(
                    self.selected_mic.as_deref().unwrap_or("Select microphone"),
                    &self.mics,
                    Message::SelectMic,
                ),
                self.mic_toggle_button(),
            ]
            .spacing(12)
            .into(),
        };

        dialog::panel(
            column![
                row![
                    text(format!("{} Config", kind.title())).size(18),
                    Space::with_width(Length::Fill),
                    mouse_area(
                        container(svg("assets/icons/x.svg").width(16).height(16))
                            .padding(8)
                            .style({
                                let theme = self.workspace_theme;
                                let is_hovered = self.is_modal_close_hovered;
                                move |iced_theme| {
                                    if is_hovered {
                                        ghost_card_hovered(theme)
                                    } else {
                                        ghost_card(iced_theme, theme)
                                    }
                                }
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
            460.0,
            progress,
            self.workspace_theme.component_palette(),
        )
    }

    fn screen_mode_button(&self, mode: ScreenMode) -> Element<Message> {
        let icon = match mode {
            ScreenMode::Fullscreen => "assets/icons/fullscreen.svg",
            ScreenMode::SelectArea => "assets/icons/square-dashed-mouse-pointer.svg",
        };
        let theme = self.workspace_theme;
        let locked = self.is_config_locked();
        let is_selected = self.screen_mode == mode;
        let is_hovered = self.hovered_screen_mode == Some(mode);

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
            .style(move |iced_theme| {
                if locked {
                    mode_card_disabled(iced_theme)
                } else if is_selected {
                    selected_mode_card(theme)
                } else if is_hovered {
                    node_card_hovered(theme)
                } else {
                    node_card(theme)
                }
            }),
        )
        .on_enter(Message::HoverScreenMode(Some(mode)))
        .on_exit(Message::HoverScreenMode(None))
        .on_press(if locked {
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
        let icon = if self.mic_mode == AudioMode::Mic {
            "assets/icons/mic.svg"
        } else {
            "assets/icons/mic-off.svg"
        };
        let subtitle = if self.mic_mode == AudioMode::Mic {
            "Microphone Will Be Recorded"
        } else {
            "Microphone Is Muted"
        };
        let theme = self.workspace_theme;
        let locked = self.is_config_locked();
        let is_active = self.mic_mode == AudioMode::Mic;
        let is_hovered = self.is_mic_toggle_hovered;

        mouse_area(
            container(
                column![
                    svg(icon).width(54).height(54),
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
            .style(move |iced_theme| {
                if locked {
                    mode_card_disabled(iced_theme)
                } else if is_active {
                    selected_mode_card(theme)
                } else if is_hovered {
                    node_card_hovered(theme)
                } else {
                    node_card(theme)
                }
            }),
        )
        .on_enter(Message::HoverMicToggle(true))
        .on_exit(Message::HoverMicToggle(false))
        .on_press(if locked {
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
        let theme = self.workspace_theme;
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
                    .style(move |iced_theme| {
                        if locked {
                            switch_track_disabled(iced_theme)
                        } else if is_active {
                            if is_hovered {
                                switch_track_active_hovered(theme)
                            } else {
                                switch_track_active(theme)
                            }
                        } else if is_hovered {
                            switch_track_hovered(iced_theme)
                        } else {
                            switch_track(iced_theme)
                        }
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

    fn dropdown(
        &self,
        selected: &str,
        options: &[String],
        on_select: fn(String) -> Message,
    ) -> Element<Message> {
        dropdown::select(
            selected,
            options
                .iter()
                .map(|option| OptionItem::new(option, on_select(option.clone())).into()),
            36,
            self.is_config_locked(),
            self.workspace_theme.component_palette(),
        )
    }
}

fn connector_svg_handle(theme: WorkspaceTheme) -> svg::Handle {
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
    stroke="{stroke}"
    stroke-opacity="0.62"
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
        stroke = theme.connector_hex(),
        left_c1_y = split_y + 18.0,
        left_c2_y = lower_y - 18.0,
        right_c1_y = split_y + 18.0,
        right_c2_y = lower_y - 18.0,
    );

    svg::Handle::from_memory(markup.into_bytes())
}

fn panel(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.app_background().into()),
        text_color: Some(Color::from_rgb8(224, 222, 216)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

fn sidebar(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.sidebar_background().into()),
        text_color: Some(Color::from_rgb8(224, 222, 216)),
        border: border::width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.titlebar_background().into()),
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

fn titlebar_menu_surface(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface().into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(9).width(0),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
    }
}

fn titlebar_menu_item(_: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(6).width(0),
        shadow: Shadow::default(),
    }
}

fn titlebar_menu_item_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface_hover().into()),
        text_color: Some(Color::from_rgb8(246, 244, 238)),
        border: border::rounded(6).width(0),
        shadow: Shadow::default(),
    }
}

fn theme_swatch_style(theme: WorkspaceTheme, is_selected: bool) -> container::Style {
    container::Style {
        background: Some(theme.accent().into()),
        text_color: Some(Color::from_rgb8(255, 255, 255)),
        border: border::rounded(12)
            .color(if is_selected {
                Color::from_rgb8(246, 244, 238)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.18)
            })
            .width(if is_selected { 2 } else { 1 }),
        shadow: Shadow::default(),
    }
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

fn record_button(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent().into()),
        text_color: Some(Color::from_rgb8(232, 242, 255)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn record_button_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent_hover().into()),
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

fn switch_track_active(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent().into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
    }
}

fn switch_track_active_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent_hover().into()),
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

fn node_card(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface().into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
    }
}

fn selected_mode_card(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.selected_surface().into()),
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

fn node_card_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface_hover().into()),
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

fn ghost_card(_: &Theme, theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.disabled_surface().into()),
        text_color: Some(Color::from_rgb8(170, 168, 160)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn ghost_card_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface_hover().into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(8).width(0),
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
    Command::new("scrop")
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
