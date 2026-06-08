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
    button, column, container, image, mouse_area, operation, row, scrollable, slider, stack, svg,
    text, Space,
};
use iced::{
    alignment, application, border, event, keyboard, mouse, time, window, Background, Color,
    ContentFit, Element, Length, Padding, Shadow, Subscription, Task, Theme,
};
use iced_video_player::{Video, VideoPlayer};
use notify_rust::Notification;
use recorder::Recorder;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use url::Url;

const NODE_CARD_WIDTH: f32 = 148.0;
const NODE_CARD_HEIGHT: f32 = 134.0;
const NODE_ROW_SPACING: f32 = 24.0;
const NODE_COLUMN_SPACING: f32 = 52.0;
const GRAPH_WIDTH: f32 = (NODE_CARD_WIDTH * 2.0) + NODE_ROW_SPACING;
const GRAPH_HEIGHT: f32 = (NODE_CARD_HEIGHT * 2.0) + NODE_COLUMN_SPACING;
const WORKSPACE_ACTIONS_MENU_LEFT: f32 = 154.0;
const SIDEBAR_WIDTH: f32 = 252.0;
const RECORDING_SORT_MENU_LEFT: f32 = 142.0;
const RECORDING_SORT_MENU_TOP: f32 = 194.0;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> iced::Result {
    application(Roton::boot, Roton::update, Roton::view)
        .title("roton")
        .theme(app_theme)
        .subscription(Roton::subscription)
        .window_size((1150.0, 580.0))
        .decorations(false)
        .run()
}

fn app_theme(_: &Roton) -> Theme {
    Theme::Dark
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
            Self::SelectArea => "Area",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioMode {
    Mute,
    Mic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordingSort {
    Newest,
    Oldest,
    AToZ,
    ZToA,
    Custom,
}

impl RecordingSort {
    const ALL: [Self; 5] = [
        Self::Newest,
        Self::Oldest,
        Self::AToZ,
        Self::ZToA,
        Self::Custom,
    ];

    fn from_key(key: &str) -> Self {
        match key {
            "Oldest" => Self::Oldest,
            "A-Z" => Self::AToZ,
            "Z-A" => Self::ZToA,
            "Custom" => Self::Custom,
            _ => Self::Newest,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Newest => "Newest",
            Self::Oldest => "Oldest",
            Self::AToZ => "A-Z",
            Self::ZToA => "Z-A",
            Self::Custom => "Custom",
        }
    }

    fn label(self) -> &'static str {
        self.key()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackShortcut {
    Toggle,
    Rewind,
    Forward,
}

#[derive(Debug, Clone)]
struct RecordingItem {
    path: PathBuf,
    file_name: String,
    title: String,
    extension: String,
    duration: Option<Duration>,
    modified: SystemTime,
    thumbnail_path: Option<PathBuf>,
}

struct VideoModal {
    path: PathBuf,
    title: String,
    extension: String,
    video: Option<Video>,
    video_width: u32,
    video_height: u32,
    duration: Duration,
    position: f64,
    is_dragging: bool,
    resume_after_seek: bool,
    load_error: Option<String>,
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
            Self::Blue => Color::from_rgb8(35, 35, 33),
            Self::Green => Color::from_rgb8(33, 37, 33),
            Self::Red => Color::from_rgb8(37, 33, 33),
            Self::Purple => Color::from_rgb8(35, 33, 37),
            Self::Amber => Color::from_rgb8(37, 35, 31),
        }
    }

    fn sidebar_background(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(39, 39, 37),
            Self::Green => Color::from_rgb8(37, 41, 37),
            Self::Red => Color::from_rgb8(41, 37, 37),
            Self::Purple => Color::from_rgb8(39, 37, 41),
            Self::Amber => Color::from_rgb8(41, 39, 35),
        }
    }

    fn titlebar_background(self) -> Color {
        match self {
            Self::Blue => Color::from_rgb8(39, 39, 37),
            Self::Green => Color::from_rgb8(37, 41, 37),
            Self::Red => Color::from_rgb8(41, 37, 37),
            Self::Purple => Color::from_rgb8(39, 37, 41),
            Self::Amber => Color::from_rgb8(41, 39, 35),
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
    RecordingsLoaded(usize, Vec<RecordingItem>),
    RecordingSearchChanged(String),
    ToggleRecordingSortMenu,
    SelectRecordingSort(RecordingSort),
    OpenRecording(PathBuf),
    ToggleVideoPlayback,
    PlaybackShortcut(PlaybackShortcut),
    SeekVideo(f64),
    SeekVideoReleased,
    VideoFrameRendered,
    VideoEnded,
    VideoError(String),
    StartRenameRecording,
    RecordingTitleChanged(String),
    CancelRenameRecording,
    SaveRecordingTitle,
    StartRecordingDrag(PathBuf),
    DragRecordingOver(PathBuf),
    FinishRecordingDrag,
    ScrollRecordings(mouse::ScrollDelta),
    HoverRecording(Option<PathBuf>),
    ToggleWorkspaceActions,
    SelectWorkspace(usize),
    SelectWorkspaceTheme(WorkspaceTheme),
    OpenCreateWorkspace,
    OpenRenameWorkspace,
    DeleteWorkspace,
    ConfirmDeleteWorkspace,
    CancelDeleteWorkspace,
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
    show_delete_workspace_confirmation: bool,
    recordings: Vec<RecordingItem>,
    recording_search: String,
    recording_sort: RecordingSort,
    is_recording_sort_open: bool,
    video_modal: Option<VideoModal>,
    is_recording_title_editing: bool,
    recording_title_edit: String,
    recording_title_error: Option<String>,
    dragging_recording: Option<PathBuf>,
    hovered_recording: Option<PathBuf>,
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
    is_modal_closing: bool,
    is_recording: bool,
    is_paused: bool,
    pause_blink_on: bool,
    elapsed: Duration,
    active_recording_started_at: Option<Instant>,
    current_recording_path: Option<PathBuf>,
    has_wl_screenrec: bool,
    has_scrop: bool,
    has_ffmpeg: bool,
    has_pactl: bool,
}

impl Roton {
    fn boot() -> (Self, Task<Message>) {
        let app = Self::new();
        let task = app.load_recordings_task();
        (app, task)
    }

    fn new() -> Self {
        let mut app = Self {
            recorder: Arc::new(Mutex::new(Recorder::new())),
            settings: Settings::load(),
            workspace_theme: WorkspaceTheme::Blue,
            is_workspace_actions_open: false,
            workspace_name_action: None,
            workspace_name: String::new(),
            workspace_name_error: None,
            show_delete_workspace_confirmation: false,
            recordings: Vec::new(),
            recording_search: String::new(),
            recording_sort: RecordingSort::Newest,
            is_recording_sort_open: false,
            video_modal: None,
            is_recording_title_editing: false,
            recording_title_edit: String::new(),
            recording_title_error: None,
            dragging_recording: None,
            hovered_recording: None,
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
            is_modal_closing: false,
            is_recording: false,
            is_paused: false,
            pause_blink_on: true,
            elapsed: Duration::ZERO,
            active_recording_started_at: None,
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
                    } else {
                        return self.load_recordings_task();
                    }
                } else {
                    if let Err(error) = self.start_recording() {
                        eprintln!("Failed to start recording: {error}");
                    }
                }
            }
            Message::TogglePause => {
                if self.is_recording {
                    let was_paused = self.is_paused;
                    let result = if was_paused {
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
                    } else if was_paused {
                        self.is_paused = false;
                        self.active_recording_started_at = Some(Instant::now());
                        self.pause_blink_on = true;
                    } else {
                        self.refresh_elapsed();
                        self.active_recording_started_at = None;
                        self.is_paused = true;
                    }
                }
            }
            Message::DragWindow => {
                if self.is_titlebar_menu_open {
                    return Task::none();
                }
                return with_latest_window(window::drag);
            }
            Message::CloseWindow => {
                if self.settings.minimize_to_tray {
                    return self.minimize_to_tray();
                }
                if self.is_recording {
                    self.show_close_confirmation = true;
                    self.open_dialog_animation();
                    self.is_titlebar_menu_open = false;
                    self.hovered_titlebar_menu_item = None;
                    return Task::none();
                }
                return with_latest_window(window::close);
            }
            Message::MaximizeWindow => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                return with_latest_window(|id| {
                    window::is_maximized(id)
                        .map(move |is_maximized| Message::SetWindowMaximized(!is_maximized))
                });
            }
            Message::SetWindowMaximized(maximized) => {
                self.is_window_maximized = maximized;
                return with_latest_window(move |id| window::maximize(id, maximized));
            }
            Message::MinimizeWindow => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                return with_latest_window(|id| window::minimize(id, true));
            }
            Message::RestoreWindow => {
                return with_latest_window(|id| window::minimize(id, false));
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
                self.open_dialog_animation();
                self.is_modal_close_hovered = false;
            }
            Message::CloseAboutRoton => {
                self.close_dialog_animation();
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
                return with_latest_window(window::close);
            }
            Message::CancelClose => {
                self.close_dialog_animation();
            }
            Message::OpenNode(kind) => {
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.close_workspace_menus();
                self.selected_node = Some(kind);
                self.hovered_node = None;
                self.open_dialog_animation();
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
                    self.refresh_elapsed();
                }
            }
            Message::Frame => {
                if self.is_modal_closing {
                    self.modal_progress = (self.modal_progress - 0.18).max(0.0);
                    if self.modal_progress == 0.0 {
                        self.close_active_dialogs();
                        self.is_modal_closing = false;
                    }
                } else if self.has_active_dialog() {
                    self.modal_progress = (self.modal_progress + 0.16).min(1.0);
                }
            }
            Message::CloseModal => {
                if let Some(modal) = self.video_modal.as_mut() {
                    if let Some(video) = modal.video.as_mut() {
                        video.set_paused(true);
                    }
                }

                self.close_dialog_animation();
                self.hovered_node = None;
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.is_modal_close_hovered = false;
                self.recording_title_error = None;
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
                if mode == ScreenMode::SelectArea && self.selected_area.is_none() {
                    return Task::done(Message::SelectArea);
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
                return Task::perform(select_area(), Message::AreaSelected);
            }
            Message::AreaSelected(area) => {
                if self.is_config_locked() {
                    return Task::none();
                }
                if let Some(area) = area {
                    self.screen_mode = ScreenMode::SelectArea;
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
                    return self.load_recordings_task();
                }
            }
            Message::RecordingsLoaded(workspace_index, recordings) => {
                if workspace_index == self.settings.active_workspace {
                    self.recordings = recordings;
                    self.normalize_recording_order();
                }
            }
            Message::RecordingSearchChanged(search) => {
                self.recording_search = search;
            }
            Message::ToggleRecordingSortMenu => {
                self.close_workspace_menus();
                self.is_recording_sort_open = !self.is_recording_sort_open;
            }
            Message::SelectRecordingSort(sort) => {
                self.recording_sort = sort;
                self.is_recording_sort_open = false;
                self.dragging_recording = None;
                self.persist_workspace_state();
                self.normalize_recording_order();
            }
            Message::OpenRecording(path) => {
                self.open_dialog_animation();
                self.is_modal_close_hovered = false;
                self.is_recording_sort_open = false;
                self.close_workspace_menus();
                return self.open_recording(path);
            }
            Message::ToggleVideoPlayback => {
                return self.toggle_video_playback();
            }
            Message::PlaybackShortcut(shortcut) => {
                if self.is_recording_title_editing {
                    return Task::none();
                }

                return match shortcut {
                    PlaybackShortcut::Toggle => self.toggle_video_playback(),
                    PlaybackShortcut::Rewind => self.seek_video_relative(-5.0),
                    PlaybackShortcut::Forward => self.seek_video_relative(5.0),
                };
            }
            Message::SeekVideo(position) => {
                if let Some(modal) = self.video_modal.as_mut() {
                    if !modal.is_dragging {
                        modal.resume_after_seek =
                            modal.video.as_ref().is_some_and(|video| !video.paused());
                    }

                    modal.position = position;
                    modal.is_dragging = true;

                    if let Some(video) = modal.video.as_mut() {
                        video.set_paused(true);
                    }
                }
            }
            Message::SeekVideoReleased => {
                if let Some(modal) = self.video_modal.as_mut() {
                    modal.is_dragging = false;

                    if let Some(video) = modal.video.as_mut() {
                        if let Err(error) =
                            video.seek(Duration::from_secs_f64(modal.position.max(0.0)), false)
                        {
                            modal.load_error = Some(format!("Could not seek video: {error}"));
                        } else {
                            video.set_paused(!modal.resume_after_seek);
                        }
                    }
                }
            }
            Message::VideoFrameRendered => {
                if let Some(modal) = self.video_modal.as_mut() {
                    if !modal.is_dragging {
                        if let Some(video) = modal.video.as_ref() {
                            modal.position = video.position().as_secs_f64();
                            let player_duration = video.duration();

                            if !player_duration.is_zero() {
                                modal.duration = player_duration;
                            }
                        }
                    }
                }
            }
            Message::VideoEnded => {
                if let Some(modal) = self.video_modal.as_mut() {
                    modal.position = modal.duration.as_secs_f64();

                    if let Some(video) = modal.video.as_mut() {
                        video.set_paused(true);
                    }
                }
            }
            Message::VideoError(error) => {
                if let Some(modal) = self.video_modal.as_mut() {
                    modal.load_error = Some(error);

                    if let Some(video) = modal.video.as_mut() {
                        video.set_paused(true);
                    }
                }
            }
            Message::StartRenameRecording => {
                if let Some(modal) = self.video_modal.as_ref() {
                    self.is_recording_title_editing = true;
                    self.recording_title_edit = modal.title.clone();
                    self.recording_title_error = None;
                    return Task::batch([
                        operation::focus("recording-title"),
                        operation::select_all("recording-title"),
                    ]);
                }
            }
            Message::RecordingTitleChanged(title) => {
                self.recording_title_edit = title;
                self.recording_title_error = None;
            }
            Message::CancelRenameRecording => {
                self.is_recording_title_editing = false;
                self.recording_title_edit.clear();
                self.recording_title_error = None;
            }
            Message::SaveRecordingTitle => {
                return self.save_recording_title();
            }
            Message::StartRecordingDrag(path) => {
                if self.recording_sort == RecordingSort::Custom {
                    self.dragging_recording = Some(path);
                }
            }
            Message::DragRecordingOver(path) => {
                self.reorder_recording_over(&path);
            }
            Message::FinishRecordingDrag => {
                self.dragging_recording = None;
            }
            Message::ScrollRecordings(delta) => {
                return operation::scroll_by("recording-list", scroll_delta_to_offset(delta));
            }
            Message::HoverRecording(path) => {
                self.hovered_recording = path;
            }
            Message::ToggleWorkspaceActions => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.is_titlebar_menu_open = false;
                self.hovered_titlebar_menu_item = None;
                self.is_recording_sort_open = false;
                self.is_workspace_actions_open = !self.is_workspace_actions_open;
            }
            Message::SelectWorkspace(index) => {
                if self.is_config_locked() || index >= self.settings.workspaces.len() {
                    return Task::none();
                }
                self.persist_workspace_state();
                self.settings.active_workspace = index;
                self.apply_active_workspace();
                self.recording_search.clear();
                self.recordings.clear();
                self.close_workspace_menus();
                self.save_settings();
                return self.load_recordings_task();
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
                self.open_dialog_animation();
                return operation::focus("workspace-name");
            }
            Message::OpenRenameWorkspace => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.close_workspace_menus();
                self.workspace_name_action = Some(WorkspaceNameAction::Rename);
                self.workspace_name = self.settings.active_workspace().name.clone();
                self.workspace_name_error = None;
                self.open_dialog_animation();
                return Task::batch([
                    operation::focus("workspace-name"),
                    operation::select_all("workspace-name"),
                ]);
            }
            Message::DeleteWorkspace => {
                if self.is_config_locked() {
                    return Task::none();
                }
                self.close_workspace_menus();
                if self.can_delete_active_workspace() {
                    self.show_delete_workspace_confirmation = true;
                    self.open_dialog_animation();
                }
            }
            Message::ConfirmDeleteWorkspace => {
                if self.is_config_locked() {
                    self.show_delete_workspace_confirmation = false;
                    return Task::none();
                }
                self.show_delete_workspace_confirmation = false;
                self.delete_active_workspace();
                return self.load_recordings_task();
            }
            Message::CancelDeleteWorkspace => {
                self.close_dialog_animation();
            }
            Message::WorkspaceNameChanged(name) => {
                self.workspace_name = name;
                self.workspace_name_error = None;
            }
            Message::ConfirmWorkspaceName => {
                self.confirm_workspace_name();
                return self.load_recordings_task();
            }
            Message::CancelWorkspaceName => {
                self.close_dialog_animation();
                self.workspace_name_error = None;
            }
            Message::Noop => {}
        }

        Task::none()
    }

    fn close_workspace_menus(&mut self) {
        self.is_workspace_actions_open = false;
        self.is_recording_sort_open = false;
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
        self.selected_area = workspace.selected_area;
        self.screen_mode = if workspace.screen_mode == "SelectArea" && self.selected_area.is_some()
        {
            ScreenMode::SelectArea
        } else {
            ScreenMode::Fullscreen
        };
        self.show_cursor = workspace.show_cursor;
        self.record_screen_sound = workspace.record_screen_sound;
        self.workspace_theme = WorkspaceTheme::from_key(&workspace.theme);
        self.recording_sort = RecordingSort::from_key(&workspace.recording_sort);
        self.is_recording_sort_open = false;
        self.dragging_recording = None;
        self.hovered_recording = None;
        self.video_modal = None;
        self.is_recording_title_editing = false;
        self.recording_title_edit.clear();
        self.is_recording_title_editing = false;
        self.recording_title_error = None;
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
        workspace.recording_sort = self.recording_sort.key().to_string();
        self.save_settings();
    }

    fn save_settings(&self) {
        if let Err(error) = self.settings.save() {
            eprintln!("Failed to save settings: {error}");
        }
    }

    fn refresh_elapsed(&mut self) {
        if let Some(started_at) = self.active_recording_started_at {
            let now = Instant::now();
            self.elapsed += now.duration_since(started_at);
            self.active_recording_started_at = Some(now);
        }
    }

    fn has_active_dialog(&self) -> bool {
        self.workspace_name_action.is_some()
            || self.show_delete_workspace_confirmation
            || self.show_close_confirmation
            || self.show_about_dialog
            || self.video_modal.is_some()
            || self.selected_node.is_some()
    }

    fn open_dialog_animation(&mut self) {
        self.modal_progress = 0.0;
        self.is_modal_closing = false;
    }

    fn close_dialog_animation(&mut self) {
        if self.has_active_dialog() {
            self.is_modal_closing = true;
        }
    }

    fn close_active_dialogs(&mut self) {
        self.workspace_name_action = None;
        self.workspace_name.clear();
        self.show_delete_workspace_confirmation = false;
        self.show_close_confirmation = false;
        self.show_about_dialog = false;
        self.video_modal = None;
        self.recording_title_edit.clear();
        self.is_recording_title_editing = false;
        self.recording_title_error = None;
        self.selected_node = None;
        self.hovered_node = None;
        self.is_modal_close_hovered = false;
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
        self.workspace_name_error = None;
        self.close_dialog_animation();
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
            self.show_delete_workspace_confirmation = false;
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
        self.show_delete_workspace_confirmation = false;
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
        let geometry = match self.screen_mode {
            ScreenMode::Fullscreen => None,
            ScreenMode::SelectArea => Some(
                self.selected_area
                    .as_deref()
                    .ok_or_else(|| "Choose an area before recording".to_string())?,
            ),
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
        self.elapsed = Duration::ZERO;
        self.active_recording_started_at = Some(Instant::now());
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
        self.elapsed = Duration::ZERO;
        self.active_recording_started_at = None;
        notify_recording_completed(
            self.settings.active_workspace().save_path.clone(),
            video_path,
        );
        Ok(())
    }

    fn minimize_to_tray(&self) -> Task<Message> {
        if self.tray_icon.is_none() {
            return with_latest_window(window::close);
        }
        with_latest_window(|id| window::minimize(id, true))
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
        subscriptions.push(time::every(Duration::from_millis(350)).map(|_| Message::TrayTick));

        if self.has_active_dialog()
            && (self.modal_progress < 1.0 || (self.is_modal_closing && self.modal_progress > 0.0))
        {
            subscriptions.push(window::frames().map(|_| Message::Frame));
        }

        if self.is_paused {
            subscriptions
                .push(time::every(Duration::from_millis(420)).map(|_| Message::PauseBlinkTick));
        }

        if self.is_recording && !self.is_paused {
            subscriptions
                .push(time::every(Duration::from_millis(10)).map(|_| Message::RecordingTick));
        }

        if self.video_modal.is_some() {
            subscriptions.push(event::listen_with(|event, _, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    repeat,
                    ..
                }) if !repeat && !modifiers.command() && !modifiers.alt() && !modifiers.logo() => {
                    match key.as_ref() {
                        keyboard::Key::Named(keyboard::key::Named::Space) => {
                            Some(Message::PlaybackShortcut(PlaybackShortcut::Toggle))
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            Some(Message::PlaybackShortcut(PlaybackShortcut::Rewind))
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            Some(Message::PlaybackShortcut(PlaybackShortcut::Forward))
                        }
                        _ => None,
                    }
                }
                _ => None,
            }));
        }

        if self.dragging_recording.is_some() {
            subscriptions.push(event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Message::FinishRecordingDrag)
                }
                _ => None,
            }));
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

    fn load_recordings_task(&self) -> Task<Message> {
        let workspace_index = self.settings.active_workspace;
        let folder = PathBuf::from(self.settings.active_workspace().save_path.clone());

        Task::perform(scan_recordings(folder), move |recordings| {
            Message::RecordingsLoaded(workspace_index, recordings)
        })
    }

    fn normalize_recording_order(&mut self) {
        let file_names = self
            .recordings
            .iter()
            .map(|recording| recording.file_name.clone())
            .collect::<Vec<_>>();
        let workspace = self.settings.active_workspace_mut();
        let before = workspace.recording_order.clone();

        workspace
            .recording_order
            .retain(|file_name| file_names.iter().any(|current| current == file_name));

        for file_name in file_names {
            if !workspace
                .recording_order
                .iter()
                .any(|existing| existing == &file_name)
            {
                workspace.recording_order.push(file_name);
            }
        }

        if workspace.recording_order != before {
            self.save_settings();
        }
    }

    fn visible_recordings(&self) -> Vec<&RecordingItem> {
        let query = self.recording_search.trim().to_ascii_lowercase();
        let mut recordings = self
            .recordings
            .iter()
            .filter(|recording| {
                query.is_empty()
                    || recording.title.to_ascii_lowercase().contains(&query)
                    || recording.extension.to_ascii_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();

        match self.recording_sort {
            RecordingSort::Newest => recordings.sort_by(|a, b| b.modified.cmp(&a.modified)),
            RecordingSort::Oldest => recordings.sort_by(|a, b| a.modified.cmp(&b.modified)),
            RecordingSort::AToZ => recordings.sort_by(|a, b| {
                a.title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
            }),
            RecordingSort::ZToA => recordings.sort_by(|a, b| {
                b.title
                    .to_ascii_lowercase()
                    .cmp(&a.title.to_ascii_lowercase())
            }),
            RecordingSort::Custom => {
                let order = &self.settings.active_workspace().recording_order;
                recordings.sort_by(|a, b| {
                    let a_index = order
                        .iter()
                        .position(|file_name| file_name == &a.file_name)
                        .unwrap_or(usize::MAX);
                    let b_index = order
                        .iter()
                        .position(|file_name| file_name == &b.file_name)
                        .unwrap_or(usize::MAX);

                    a_index
                        .cmp(&b_index)
                        .then_with(|| a.file_name.cmp(&b.file_name))
                });
            }
        }

        recordings
    }

    fn reorder_recording_over(&mut self, target_path: &Path) {
        if self.recording_sort != RecordingSort::Custom {
            return;
        }

        let Some(dragged_path) = self.dragging_recording.clone() else {
            return;
        };

        if dragged_path == target_path {
            return;
        }

        let Some(dragged_name) = file_name_string(&dragged_path) else {
            return;
        };
        let Some(target_name) = file_name_string(target_path) else {
            return;
        };

        self.normalize_recording_order();
        let order = &mut self.settings.active_workspace_mut().recording_order;
        let Some(from) = order
            .iter()
            .position(|file_name| file_name == &dragged_name)
        else {
            return;
        };
        let Some(to) = order.iter().position(|file_name| file_name == &target_name) else {
            return;
        };

        let moved = order.remove(from);
        let insert_at = if from < to { to.saturating_sub(1) } else { to };
        order.insert(insert_at, moved);
        self.save_settings();
    }

    fn open_recording(&mut self, path: PathBuf) -> Task<Message> {
        let title = self
            .recordings
            .iter()
            .find(|recording| recording.path == path)
            .map(|recording| recording.title.clone())
            .or_else(|| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "recording".to_string());
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("mp4")
            .to_ascii_lowercase();
        let metadata = probe_video_metadata(&path);
        let mut duration = self
            .recordings
            .iter()
            .find(|recording| recording.path == path)
            .and_then(|recording| recording.duration)
            .or(metadata.duration)
            .unwrap_or(Duration::ZERO);

        let (mut video_width, mut video_height) = metadata.dimensions.unwrap_or((640, 360));
        let video_url_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let (video, load_error) = match Url::from_file_path(&video_url_path) {
            Ok(url) => match Video::new(&url) {
                Ok(mut video) => {
                    let (width, height) = video.size();

                    if width > 0 && height > 0 {
                        video_width = width as u32;
                        video_height = height as u32;
                    }

                    let player_duration = video.duration();
                    if !player_duration.is_zero() {
                        duration = player_duration;
                    }

                    video.set_paused(true);

                    (Some(video), None)
                }
                Err(error) => (None, Some(format!("Could not load video: {error}"))),
            },
            Err(()) => (
                None,
                Some("Could not create a file URL for this video.".to_string()),
            ),
        };

        self.video_modal = Some(VideoModal {
            path: path.clone(),
            title,
            extension,
            video,
            video_width,
            video_height,
            duration,
            position: 0.0,
            is_dragging: false,
            resume_after_seek: false,
            load_error,
        });
        self.recording_title_edit.clear();
        self.is_recording_title_editing = false;
        self.recording_title_error = None;

        Task::none()
    }

    fn toggle_video_playback(&mut self) -> Task<Message> {
        if let Some(modal) = self.video_modal.as_mut() {
            if let Some(video) = modal.video.as_mut() {
                if video.eos() || modal.position >= modal.duration.as_secs_f64() {
                    if let Err(error) = video.seek(Duration::ZERO, false) {
                        modal.load_error = Some(format!("Could not restart video: {error}"));
                        return Task::none();
                    }

                    modal.position = 0.0;
                }

                video.set_paused(!video.paused());
            }
        }

        Task::none()
    }

    fn seek_video_relative(&mut self, seconds: f64) -> Task<Message> {
        if let Some(modal) = self.video_modal.as_mut() {
            let Some(video) = modal.video.as_mut() else {
                return Task::none();
            };

            let duration = modal.duration.as_secs_f64().max(0.0);
            modal.position = (modal.position + seconds).clamp(0.0, duration);
            modal.is_dragging = false;
            modal.resume_after_seek = !video.paused();

            if let Err(error) = video.seek(Duration::from_secs_f64(modal.position), false) {
                modal.load_error = Some(format!("Could not seek video: {error}"));
            }
        }

        Task::none()
    }

    fn save_recording_title(&mut self) -> Task<Message> {
        let Some(modal) = self.video_modal.as_ref() else {
            return Task::none();
        };

        let title = self.recording_title_edit.trim().to_string();
        let extension = modal.extension.clone();

        if let Some(error) = validate_recording_title(&title, &extension) {
            self.recording_title_error = Some(error);
            return Task::none();
        }

        if title == modal.title {
            self.recording_title_edit.clear();
            self.is_recording_title_editing = false;
            self.recording_title_error = None;
            return Task::none();
        }

        let old_path = modal.path.clone();
        let new_file_name = format!("{title}.{extension}");
        let new_path = old_path.with_file_name(&new_file_name);

        if new_path.exists() {
            self.recording_title_error =
                Some("A recording with that name already exists.".to_string());
            return Task::none();
        }

        if let Err(error) = fs::rename(&old_path, &new_path) {
            self.recording_title_error = Some(format!("Could not rename recording: {error}"));
            return Task::none();
        }

        let old_file_name = file_name_string(&old_path);
        if let Some(modal) = self.video_modal.as_mut() {
            modal.path = new_path.clone();
            modal.title = title;
        }

        if let Some(old_file_name) = old_file_name {
            let order = &mut self.settings.active_workspace_mut().recording_order;
            if let Some(index) = order
                .iter()
                .position(|file_name| file_name == &old_file_name)
            {
                order[index] = new_file_name;
            }
        }

        self.recording_title_edit.clear();
        self.is_recording_title_editing = false;
        self.recording_title_error = None;
        self.save_settings();
        self.load_recordings_task()
    }

    fn view(&self) -> Element<'_, Message> {
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
                        container(Space::new().width(Length::Fill).height(Length::Fill))
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
                        container(Space::new().width(Length::Fill).height(Length::Fill))
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

        let app: Element<_> = if self.is_recording_sort_open {
            stack![
                app,
                event_blocker(
                    mouse_area(
                        container(Space::new().width(Length::Fill).height(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fill)
                    )
                    .on_press(Message::ToggleRecordingSortMenu)
                ),
                container(container(self.recording_sort_menu()).width(122))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(
                        Padding::default()
                            .top(RECORDING_SORT_MENU_TOP)
                            .left(RECORDING_SORT_MENU_LEFT)
                    )
                    .align_x(alignment::Horizontal::Left)
                    .align_y(alignment::Vertical::Top)
            ]
            .into()
        } else {
            app
        };

        let dialog_progress = ease_out(self.modal_progress);

        if self.workspace_name_action.is_some() {
            stack![
                app,
                dialog::backdrop(dialog_progress, Message::CancelWorkspaceName),
                container(
                    mouse_area(self.workspace_name_modal(dialog_progress)).on_press(Message::Noop)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if self.show_delete_workspace_confirmation {
            stack![
                app,
                dialog::backdrop(dialog_progress, Message::CancelDeleteWorkspace),
                container(
                    mouse_area(self.delete_workspace_confirmation(dialog_progress))
                        .on_press(Message::Noop)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if self.show_close_confirmation {
            stack![
                app,
                dialog::backdrop(dialog_progress, Message::Noop),
                container(self.close_confirmation(dialog_progress))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if self.show_about_dialog {
            stack![
                app,
                dialog::backdrop(dialog_progress, Message::CloseAboutRoton),
                container(mouse_area(self.about_dialog(dialog_progress)).on_press(Message::Noop))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if self.video_modal.is_some() {
            stack![
                app,
                dialog::backdrop(dialog_progress, Message::CloseModal),
                container(
                    mouse_area(self.recording_modal(dialog_progress)).on_press(Message::Noop)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
            ]
            .into()
        } else if let Some(kind) = self.selected_node {
            stack![
                app,
                dialog::backdrop(dialog_progress, Message::CloseModal),
                container(mouse_area(self.modal(kind, dialog_progress)).on_press(Message::Noop))
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

    fn close_confirmation(&self, progress: f32) -> Element<'_, Message> {
        let palette = self
            .workspace_theme
            .component_palette()
            .scale_alpha(progress);

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
                    Space::new().width(Length::Fill),
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
            progress,
            palette,
        )
    }

    fn delete_workspace_confirmation(&self, progress: f32) -> Element<'_, Message> {
        let palette = self
            .workspace_theme
            .component_palette()
            .scale_alpha(progress);
        let workspace_name = truncate_text(&self.settings.active_workspace().name, 34);

        dialog::panel(
            column![
                text("Remove Workspace?").size(18),
                text(format!("Remove \"{workspace_name}\" from Roton?")).size(13),
                text("Recordings in its save folder will stay on disk.")
                    .size(13)
                    .color(palette.text_muted),
                row![
                    ui_button::themed_text_button(
                        "Cancel",
                        ButtonVariant::Secondary,
                        Some(Message::CancelDeleteWorkspace),
                        palette,
                    ),
                    Space::new().width(Length::Fill),
                    ui_button::themed_text_button(
                        "Remove",
                        ButtonVariant::Danger,
                        Some(Message::ConfirmDeleteWorkspace),
                        palette,
                    ),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(12),
            420.0,
            progress,
            palette,
        )
    }

    fn about_dialog(&self, progress: f32) -> Element<'_, Message> {
        let theme = self.workspace_theme;
        let palette = theme.component_palette().scale_alpha(progress);
        let close_style = {
            let is_hovered = self.is_modal_close_hovered;
            move |iced_theme: &Theme| {
                fade_container_style(
                    if is_hovered {
                        ghost_card_hovered(theme)
                    } else {
                        ghost_card(iced_theme, theme)
                    },
                    progress,
                )
            }
        };
        let detail_row = |label: &'static str, value: &'static str| {
            row![
                text(label).size(12).color(palette.text_muted),
                Space::new().width(Length::Fill),
                text(value).size(12).color(palette.text),
            ]
            .align_y(alignment::Vertical::Center)
        };

        dialog::panel(
            column![
                row![
                    image("assets/rotonicon.png").width(44).height(44).opacity(progress),
                    column![
                        text("Roton").size(21),
                        text("Screen recorder").size(12).color(palette.text_muted),
                    ]
                    .spacing(4),
                    Space::new().width(Length::Fill),
                    mouse_area(
                        container(
                            svg("assets/icons/x.svg")
                                .width(16)
                                .height(16)
                                .opacity(progress),
                        )
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
                    container(Space::new().height(1))
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
                    Space::new().width(Length::Fill),
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
            progress,
            palette,
        )
    }

    fn recording_modal(&self, progress: f32) -> Element<'_, Message> {
        let Some(modal) = self.video_modal.as_ref() else {
            return Space::new().height(0).into();
        };

        let theme = self.workspace_theme;
        let palette = theme.component_palette().scale_alpha(progress);
        let close_style = {
            let is_hovered = self.is_modal_close_hovered;
            move |iced_theme: &Theme| {
                fade_container_style(
                    if is_hovered {
                        ghost_card_hovered(theme)
                    } else {
                        ghost_card(iced_theme, theme)
                    },
                    progress,
                )
            }
        };
        let title_error: Element<_> = self
            .recording_title_error
            .as_deref()
            .map(|error| {
                text(error)
                    .size(12)
                    .color(Color::from_rgb8(232, 126, 126).scale_alpha(progress))
                    .into()
            })
            .unwrap_or_else(|| Space::new().height(0).into());

        let title_row: Element<_> = if self.is_recording_title_editing {
            row![
                textbox::field(
                    "Recording title",
                    &self.recording_title_edit,
                    "recording-title",
                    Message::RecordingTitleChanged,
                    Message::SaveRecordingTitle,
                    palette,
                )
                .width(Length::Fill),
                text(format!(".{}", modal.extension))
                    .size(13)
                    .color(palette.text),
                ui_button::themed_compact_icon_button(
                    "assets/icons/x.svg",
                    ButtonVariant::Secondary,
                    Some(Message::CancelRenameRecording),
                    palette,
                ),
                button(
                    row![
                        svg("assets/icons/check.svg").width(14).height(14),
                        text("Save").size(13),
                    ]
                    .spacing(6)
                    .align_y(alignment::Vertical::Center),
                )
                .padding([10, 12])
                .style({
                    let accent = theme.accent().scale_alpha(progress);
                    let hover = theme.accent_hover().scale_alpha(progress);
                    move |_, status| recording_save_button_style(status, accent, hover)
                })
                .on_press(Message::SaveRecordingTitle),
            ]
            .spacing(8)
            .align_y(alignment::Vertical::Center)
            .into()
        } else {
            let file_name = format!("{}.{}", modal.title, modal.extension);
            row![
                mouse_area(
                    row![
                        text(truncate_text(&file_name, 54)).size(14),
                        svg("assets/icons/pencil-line.svg").width(14).height(14),
                    ]
                    .spacing(8)
                    .align_y(alignment::Vertical::Center)
                )
                .interaction(mouse::Interaction::Pointer)
                .on_press(Message::StartRenameRecording),
                Space::new().width(Length::Fill),
                mouse_area(
                    container(
                        svg("assets/icons/x.svg")
                            .width(16)
                            .height(16)
                            .opacity(progress),
                    )
                    .padding(8)
                    .style(close_style)
                )
                .on_enter(Message::HoverModalClose(true))
                .on_exit(Message::HoverModalClose(false))
                .on_press(Message::CloseModal),
            ]
            .align_y(alignment::Vertical::Center)
            .into()
        };

        let (video_width, video_height, panel_width) =
            video_modal_dimensions(modal.video_width, modal.video_height);

        let video_surface: Element<_> = if let Some(video) = modal.video.as_ref() {
            container(
                VideoPlayer::new(video)
                    .width(video_width)
                    .height(video_height)
                    .content_fit(ContentFit::Contain)
                    .on_new_frame(Message::VideoFrameRendered)
                    .on_end_of_stream(Message::VideoEnded)
                    .on_error(|error| Message::VideoError(error.to_string())),
            )
            .width(video_width)
            .height(video_height)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |_| video_surface_style(palette))
            .into()
        } else {
            let message = modal
                .load_error
                .as_deref()
                .unwrap_or("Could not load this video.");

            container(
                column![
                    svg("assets/icons/file-video-camera.svg")
                        .width(34)
                        .height(34),
                    text(message).size(12).color(palette.text_muted),
                ]
                .spacing(10)
                .align_x(alignment::Horizontal::Center),
            )
            .width(video_width)
            .height(video_height)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |_| video_surface_style(palette))
            .into()
        };

        let duration = modal.duration;
        let duration_secs = duration.as_secs_f64().max(0.1);
        let position = modal.position.clamp(0.0, duration_secs);
        let is_playing = modal.video.as_ref().is_some_and(|video| !video.paused());
        let play_icon = if !is_playing {
            "assets/icons/play.svg"
        } else {
            "assets/icons/pause.svg"
        };

        dialog::panel(
            column![
                title_row,
                title_error,
                video_surface,
                row![
                    ui_button::themed_compact_icon_button(
                        play_icon,
                        ButtonVariant::Secondary,
                        modal
                            .video
                            .is_some()
                            .then_some(Message::ToggleVideoPlayback),
                        palette,
                    ),
                    text(format!(
                        "{} / {}",
                        format_duration(Duration::from_secs_f64(position)),
                        format_duration(duration),
                    ))
                    .size(12)
                    .color(palette.text)
                    .width(78),
                    slider(0.0..=duration_secs, position, Message::SeekVideo)
                        .step(0.1)
                        .on_release(Message::SeekVideoReleased),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(10),
            panel_width,
            progress,
            palette,
        )
    }

    fn sidebar(&self) -> Element<'_, Message> {
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
            text(status)
                .size(12)
                .color(self.workspace_theme.component_palette().text_muted),
            Space::new().width(Length::Fill),
            text(format_elapsed(self.elapsed))
                .size(12)
                .color(self.workspace_theme.component_palette().text),
        ]
        .width(Length::Fill)
        .align_y(alignment::Vertical::Center);
        let palette = self.workspace_theme.component_palette();
        let search_row = row![
            textbox::field(
                "Search...",
                &self.recording_search,
                "recording-search",
                Message::RecordingSearchChanged,
                Message::Noop,
                palette,
            )
            .width(Length::Fill),
            ui_button::themed_compact_icon_button(
                "assets/icons/list-filter.svg",
                ButtonVariant::Side,
                Some(Message::ToggleRecordingSortMenu),
                palette,
            ),
        ]
        .spacing(8)
        .align_y(alignment::Vertical::Center);

        let header = container(
            column![self.workspace_selector(), controls, status_row, search_row,].spacing(14),
        )
        .width(Length::Fill)
        .padding(Padding::default().top(18).right(18).bottom(10).left(18));

        let list = scrollable(
            mouse_area(
                container(self.recording_list())
                    .width(Length::Fill)
                    .padding(Padding::default().right(18).bottom(18).left(18)),
            )
            .on_scroll(Message::ScrollRecordings),
        )
        .id("recording-list")
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::default().width(6).scroller_width(6),
        ))
        .style(recording_scrollable_style)
        .height(Length::Fill)
        .width(Length::Fill);

        let sidebar = container(column![header, list].spacing(0))
            .width(SIDEBAR_WIDTH)
            .height(Length::Fill)
            .style({
                let theme = self.workspace_theme;
                move |_| sidebar(theme)
            });

        sidebar.into()
    }

    fn recording_list(&self) -> Element<'_, Message> {
        let palette = self.workspace_theme.component_palette();
        let visible = self.visible_recordings();

        if visible.is_empty() {
            let message = if self.recording_search.trim().is_empty() {
                "No recordings"
            } else {
                "No matches"
            };

            return container(
                column![
                    svg("assets/icons/file-video-camera.svg")
                        .width(28)
                        .height(28),
                    text(message).size(12).color(palette.text_muted),
                ]
                .spacing(8)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .height(130)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |_| recording_empty_state(palette))
            .into();
        }

        visible
            .into_iter()
            .fold(column![].spacing(10), |column, recording| {
                column.push(self.recording_item(recording))
            })
            .into()
    }

    fn recording_item(&self, recording: &RecordingItem) -> Element<'_, Message> {
        let palette = self.workspace_theme.component_palette();
        let path = recording.path.clone();
        let is_dragging = self.dragging_recording.as_ref() == Some(&recording.path);
        let is_hovered = self.hovered_recording.as_ref() == Some(&recording.path);
        let title = truncate_text(&recording.title, 18);
        let extension = recording.extension.clone();
        let thumbnail_path = recording.thumbnail_path.clone();
        let duration = recording
            .duration
            .map(format_duration)
            .unwrap_or_else(|| "--:--".to_string());
        let thumbnail: Element<_> = if let Some(thumbnail_path) = thumbnail_path {
            container(
                image(thumbnail_path.to_string_lossy().to_string())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(ContentFit::Cover),
            )
            .width(72)
            .height(46)
            .style(move |_| recording_thumbnail_style(palette))
            .into()
        } else {
            container(
                svg("assets/icons/file-video-camera.svg")
                    .width(24)
                    .height(24),
            )
            .width(72)
            .height(46)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |_| recording_thumbnail_style(palette))
            .into()
        };

        let card = container(
            row![
                thumbnail,
                column![
                    text(title).size(13),
                    text(duration).size(11).color(palette.text_muted),
                    text(extension).size(11).color(palette.text_muted),
                ]
                .spacing(3)
                .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(alignment::Vertical::Center),
        )
        .padding(Padding::default().top(8).right(10).bottom(8).left(8))
        .width(Length::Fill)
        .style({
            let theme = self.workspace_theme;
            move |_| recording_item_style(theme, is_dragging, is_hovered)
        });

        if self.recording_sort == RecordingSort::Custom {
            let move_path = path.clone();
            mouse_area(card)
                .interaction(if is_dragging {
                    mouse::Interaction::Grabbing
                } else {
                    mouse::Interaction::Grab
                })
                .on_enter(Message::HoverRecording(Some(path.clone())))
                .on_exit(Message::HoverRecording(None))
                .on_press(Message::StartRecordingDrag(path.clone()))
                .on_move(move |_| Message::DragRecordingOver(move_path.clone()))
                .on_release(Message::FinishRecordingDrag)
                .on_double_click(Message::OpenRecording(path))
                .into()
        } else {
            mouse_area(card)
                .interaction(mouse::Interaction::Pointer)
                .on_enter(Message::HoverRecording(Some(path.clone())))
                .on_exit(Message::HoverRecording(None))
                .on_press(Message::OpenRecording(path))
                .into()
        }
    }

    fn recording_sort_menu(&self) -> Element<'_, Message> {
        let palette = self.workspace_theme.component_palette();

        let items = RecordingSort::ALL
            .into_iter()
            .fold(column![].spacing(0), |column, sort| {
                let is_selected = sort == self.recording_sort;
                let marker: Element<_> = if is_selected {
                    svg("assets/icons/check.svg").width(14).height(14).into()
                } else {
                    Space::new().width(14).height(14).into()
                };

                column.push(
                    button(
                        row![
                            text(sort.label()).size(12),
                            Space::new().width(Length::Fill),
                            marker,
                        ]
                        .spacing(8)
                        .align_y(alignment::Vertical::Center),
                    )
                    .padding([8, 10])
                    .width(Length::Fill)
                    .style(move |_, status| {
                        if is_selected {
                            component_styles::dropdown_option_selected_with_palette(status, palette)
                        } else {
                            component_styles::dropdown_option_with_palette(status, palette)
                        }
                    })
                    .on_press(Message::SelectRecordingSort(sort)),
                )
            });

        event_blocker(
            container(items)
                .padding([4, 0])
                .width(Length::Fill)
                .style(move |_| component_styles::context_menu_with_palette(palette)),
        )
    }

    fn workspace_selector(&self) -> Element<'_, Message> {
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

    fn workspace_actions_menu(&self) -> Element<'_, Message> {
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
                    Space::new().width(Length::Fill),
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
                container(Space::new().height(1))
                    .width(Length::Fill)
                    .style(move |_| component_styles::separator_with_palette(palette))
            )
            .padding([3, 0])
            .width(Length::Fill),
            menu_item("Rename", Some(Message::OpenRenameWorkspace)),
        ]
        .spacing(0);

        let items = if self.can_delete_active_workspace() {
            items.push(menu_item("Remove", Some(Message::DeleteWorkspace)))
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
            Space::new().width(13).height(13).into()
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

    fn workspace_name_modal(&self, progress: f32) -> Element<'_, Message> {
        let palette = self
            .workspace_theme
            .component_palette()
            .scale_alpha(progress);
        let is_create = self.workspace_name_action == Some(WorkspaceNameAction::Create);
        let title = if is_create {
            "New Workspace"
        } else {
            "Rename Workspace"
        };
        let description = if is_create {
            "Recordings start in a dedicated folder under ~/Videos/Roton"
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
                    .color(Color::from_rgb8(232, 126, 126).scale_alpha(progress))
                    .into()
            })
            .unwrap_or_else(|| Space::new().height(0).into());

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
                    palette,
                ),
                error,
                row![
                    Space::new().width(Length::Fill),
                    ui_button::themed_text_button(
                        "Cancel",
                        ButtonVariant::Secondary,
                        Some(Message::CancelWorkspaceName),
                        palette,
                    ),
                    ui_button::accent_text_button(
                        confirm_label,
                        Some(Message::ConfirmWorkspaceName),
                        self.workspace_theme.accent().scale_alpha(progress),
                        self.workspace_theme.accent_hover().scale_alpha(progress),
                    ),
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(12),
            420.0,
            progress,
            palette,
        )
    }

    fn titlebar(&self) -> Element<'_, Message> {
        let titlebar_action = |action, icon: &'static str, icon_size: u16, message| {
            mouse_area(
                container(image(icon).width(icon_size as u32).height(icon_size as u32))
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
                    Space::new().width(Length::Fill),
                    text("Roton").size(15),
                    Space::new().width(Length::Fill),
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

    fn titlebar_menu(&self) -> Element<'_, Message> {
        let theme = self.workspace_theme;
        let palette = theme.component_palette();
        let item = |index, label: &'static str, message| {
            mouse_area(
                container(
                    row![Space::new().width(25), text(label).size(13)]
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
                container(Space::new().height(1))
                    .width(Length::Fill)
                    .style(move |_| component_styles::separator_with_palette(palette)),
            )
            .padding([3, 0])
            .width(Length::Fill)
        };

        let minimal_marker: Element<_> = if self.is_minimal_window {
            svg("assets/icons/check.svg").width(15).height(15).into()
        } else {
            Space::new().width(15).height(15).into()
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
            Space::new().width(15).height(15).into()
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

    fn canvas(&self) -> Element<'_, Message> {
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

    fn node(&self, kind: NodeKind) -> Element<'_, Message> {
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

    fn modal(&self, kind: NodeKind, progress: f32) -> Element<'_, Message> {
        let palette = self
            .workspace_theme
            .component_palette()
            .scale_alpha(progress);
        let body: Element<_> = match kind {
            NodeKind::Output => column![
                text("Format").size(13),
                self.dropdown_with_palette(
                    &self.selected_format,
                    &self.formats,
                    Message::SelectFormat,
                    palette,
                ),
                text("Save Folder").size(13),
                row![
                    textbox::readonly(
                        truncate_text(&self.settings.active_workspace().save_path, 38),
                        palette,
                    ),
                    ui_button::themed_text_button(
                        "Choose",
                        ButtonVariant::Secondary,
                        (!self.is_config_locked()).then_some(Message::ChooseOutputFolder),
                        palette,
                    ),
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(10)
            .into(),
            NodeKind::Screen => column![
                text("Monitor").size(13),
                self.dropdown_with_palette(
                    self.selected_monitor.as_deref().unwrap_or("Select monitor"),
                    &self.monitors,
                    Message::SelectMonitor,
                    palette,
                ),
                self.switch_row(
                    "Show Cursor",
                    self.show_cursor,
                    self.is_show_cursor_hovered,
                    Message::ToggleShowCursor,
                    Message::HoverShowCursor(true),
                    Message::HoverShowCursor(false),
                    progress,
                ),
                self.switch_row(
                    "Record Screen Sound",
                    self.record_screen_sound,
                    self.is_screen_sound_hovered,
                    Message::ToggleScreenSound,
                    Message::HoverScreenSound(true),
                    Message::HoverScreenSound(false),
                    progress,
                ),
                row![
                    self.screen_mode_button(ScreenMode::Fullscreen, progress),
                    self.screen_mode_button(ScreenMode::SelectArea, progress),
                ]
                .spacing(10),
                if self.screen_mode == ScreenMode::SelectArea {
                    self.screen_area_panel(palette)
                } else {
                    Element::<Message>::from(
                        mouse_area(container(Space::new().height(0))).on_press(Message::Noop),
                    )
                },
            ]
            .spacing(12)
            .into(),
            NodeKind::Mic => column![
                text("Microphone").size(13),
                self.dropdown_with_palette(
                    self.selected_mic.as_deref().unwrap_or("Select microphone"),
                    &self.mics,
                    Message::SelectMic,
                    palette,
                ),
                self.mic_toggle_button(progress),
            ]
            .spacing(12)
            .into(),
        };

        dialog::panel(
            column![
                row![
                    text(format!("{} Config", kind.title())).size(18),
                    Space::new().width(Length::Fill),
                    mouse_area(
                        container(
                            svg("assets/icons/x.svg")
                                .width(16)
                                .height(16)
                                .opacity(progress),
                        )
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
            palette,
        )
    }

    fn screen_mode_button(&self, mode: ScreenMode, progress: f32) -> Element<'_, Message> {
        let icon = match mode {
            ScreenMode::Fullscreen => "assets/icons/fullscreen.svg",
            ScreenMode::SelectArea => "assets/icons/square-dashed-mouse-pointer.svg",
        };
        let palette = self
            .workspace_theme
            .component_palette()
            .scale_alpha(progress);
        let subtitle = match mode {
            ScreenMode::Fullscreen => self
                .selected_monitor_output()
                .unwrap_or_else(|| "Whole monitor".to_string()),
            ScreenMode::SelectArea => self
                .selected_area
                .as_deref()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "Choose area".to_string()),
        };
        let theme = self.workspace_theme;
        let locked = self.is_config_locked();
        let is_selected = self.screen_mode == mode;
        let is_hovered = self.hovered_screen_mode == Some(mode);
        let on_press = if locked {
            Message::Noop
        } else if mode == ScreenMode::SelectArea && self.selected_area.is_none() {
            Message::SelectArea
        } else {
            Message::SelectScreenMode(mode)
        };

        mouse_area(
            container(
                column![
                    svg(icon).width(48).height(48).opacity(progress),
                    text(mode.label())
                        .size(12)
                        .width(Length::Fill)
                        .align_x(alignment::Horizontal::Center),
                    text(truncate_text(&subtitle, 24))
                        .size(11)
                        .color(palette.text_muted)
                        .width(Length::Fill)
                        .align_x(alignment::Horizontal::Center),
                ]
                .spacing(8)
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .padding(16)
            .width(Length::Fill)
            .height(120)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |iced_theme| {
                fade_container_style(
                    if locked {
                        mode_card_disabled(iced_theme)
                    } else if is_selected {
                        selected_mode_card(theme)
                    } else if is_hovered {
                        node_card_hovered(theme)
                    } else {
                        node_card(theme)
                    },
                    progress,
                )
            }),
        )
        .on_enter(Message::HoverScreenMode(Some(mode)))
        .on_exit(Message::HoverScreenMode(None))
        .on_press(on_press)
        .into()
    }

    fn screen_area_panel(&self, palette: component_styles::Palette) -> Element<'_, Message> {
        let locked = self.is_config_locked();
        let area = self.selected_area.as_deref().unwrap_or("Not selected");
        let action_label = if self.selected_area.is_some() {
            "Change"
        } else {
            "Choose"
        };

        container(
            row![
                column![
                    text("Area").size(12).color(palette.text_muted),
                    text(truncate_text(area, 42)).size(13).color(palette.text),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                ui_button::themed_text_button(
                    action_label,
                    ButtonVariant::Secondary,
                    (!locked).then_some(Message::SelectArea),
                    palette,
                ),
            ]
            .spacing(10)
            .align_y(alignment::Vertical::Center),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .style(move |_| component_styles::input_surface_with_palette(palette))
        .into()
    }

    fn mic_toggle_button(&self, progress: f32) -> Element<'_, Message> {
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
                    svg(icon).width(54).height(54).opacity(progress),
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
                fade_container_style(
                    if locked {
                        mode_card_disabled(iced_theme)
                    } else if is_active {
                        selected_mode_card(theme)
                    } else if is_hovered {
                        node_card_hovered(theme)
                    } else {
                        node_card(theme)
                    },
                    progress,
                )
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
        progress: f32,
    ) -> Element<'_, Message> {
        let locked = self.is_config_locked();
        let theme = self.workspace_theme;
        container(
            row![
                text(label).size(13),
                Space::new().width(Length::Fill),
                mouse_area(
                    container(
                        row![
                            if is_active {
                                Space::new().width(Length::Fill)
                            } else {
                                Space::new().width(0)
                            },
                            container(Space::new().width(16).height(16)).style(move |theme| {
                                fade_container_style(
                                    if locked {
                                        switch_knob_disabled(theme)
                                    } else if is_active {
                                        switch_knob_active(theme)
                                    } else {
                                        switch_knob(theme)
                                    },
                                    progress,
                                )
                            }),
                            if is_active {
                                Space::new().width(0)
                            } else {
                                Space::new().width(Length::Fill)
                            },
                        ]
                        .align_y(alignment::Vertical::Center)
                    )
                    .padding(3)
                    .width(42)
                    .height(22)
                    .style(move |iced_theme| {
                        if locked {
                            fade_container_style(switch_track_disabled(iced_theme), progress)
                        } else if is_active {
                            if is_hovered {
                                fade_container_style(switch_track_active_hovered(theme), progress)
                            } else {
                                fade_container_style(switch_track_active(theme), progress)
                            }
                        } else if is_hovered {
                            fade_container_style(switch_track_hovered(iced_theme), progress)
                        } else {
                            fade_container_style(switch_track(iced_theme), progress)
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
        .style(move |theme| fade_container_style(switch_row(theme), progress))
        .into()
    }

    fn dropdown_with_palette(
        &self,
        selected: &str,
        options: &[String],
        on_select: fn(String) -> Message,
        palette: component_styles::Palette,
    ) -> Element<'_, Message> {
        dropdown::select(
            selected,
            options
                .iter()
                .map(|option| OptionItem::new(option, on_select(option.clone())).into()),
            36,
            self.is_config_locked(),
            palette,
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

fn with_latest_window(
    action: impl Fn(window::Id) -> Task<Message> + Send + 'static,
) -> Task<Message> {
    window::latest().and_then(action)
}

fn panel(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.app_background().into()),
        text_color: Some(Color::from_rgb8(224, 222, 216)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn sidebar(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.sidebar_background().into()),
        text_color: Some(Color::from_rgb8(224, 222, 216)),
        border: border::width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn titlebar(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.titlebar_background().into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn titlebar_close(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(214, 212, 205)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn titlebar_close_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(50, 50, 47).into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(0).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn titlebar_icon_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
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
        snap: false,
    }
}

fn titlebar_menu_item(_: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(6).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn titlebar_menu_item_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface_hover().into()),
        text_color: Some(Color::from_rgb8(246, 244, 238)),
        border: border::rounded(6).width(0),
        shadow: Shadow::default(),
        snap: false,
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
        snap: false,
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
        snap: false,
    }
}

fn record_button_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent_hover().into()),
        text_color: Some(Color::from_rgb8(232, 242, 255)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn stop_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(170, 43, 48).into()),
        text_color: Some(Color::from_rgb8(255, 235, 235)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn stop_button_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(142, 34, 40).into()),
        text_color: Some(Color::from_rgb8(255, 235, 235)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn side_button(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(47, 47, 44).into()),
        text_color: Some(Color::from_rgb8(230, 228, 220)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn side_button_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(58, 58, 54).into()),
        text_color: Some(Color::from_rgb8(240, 238, 230)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn pause_button_active(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(158, 122, 38).into()),
        text_color: Some(Color::from_rgb8(255, 245, 210)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn pause_button_active_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(136, 101, 28).into()),
        text_color: Some(Color::from_rgb8(255, 245, 210)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn pause_button_blink_off(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(112, 86, 24).into()),
        text_color: Some(Color::from_rgb8(255, 245, 210)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn fade_container_style(mut style: container::Style, progress: f32) -> container::Style {
    style.background = style.background.map(|background| match background {
        Background::Color(color) => color.scale_alpha(progress).into(),
        Background::Gradient(gradient) => gradient.scale_alpha(progress).into(),
    });
    style.text_color = style.text_color.map(|color| color.scale_alpha(progress));
    style.border.color = style.border.color.scale_alpha(progress);
    style.shadow.color = style.shadow.color.scale_alpha(progress);
    style
}

fn switch_row(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_track(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(68, 68, 63).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_track_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(82, 82, 76).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_track_active(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent().into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_track_active_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.accent_hover().into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_track_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(50, 50, 46).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(11).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_knob(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(218, 216, 208).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_knob_active(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(238, 246, 255).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn switch_knob_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(120, 118, 112).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn node_card(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface().into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn selected_mode_card(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.selected_surface().into()),
        text_color: Some(Color::from_rgb8(235, 242, 250)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn mode_card_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(43, 43, 40).into()),
        text_color: Some(Color::from_rgb8(132, 130, 124)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn node_card_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface_hover().into()),
        text_color: Some(Color::from_rgb8(244, 242, 235)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn node_card_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(41, 41, 38).into()),
        text_color: Some(Color::from_rgb8(150, 148, 140)),
        border: border::rounded(12).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn ghost_card(_: &Theme, theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.disabled_surface().into()),
        text_color: Some(Color::from_rgb8(170, 168, 160)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn ghost_card_hovered(theme: WorkspaceTheme) -> container::Style {
    container::Style {
        background: Some(theme.surface_hover().into()),
        text_color: Some(Color::from_rgb8(232, 230, 222)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn recording_empty_state(palette: component_styles::Palette) -> container::Style {
    container::Style {
        background: Some(palette.field.scale_alpha(0.45).into()),
        text_color: Some(palette.text_muted),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn recording_thumbnail_style(palette: component_styles::Palette) -> container::Style {
    container::Style {
        background: Some(palette.field_disabled.into()),
        text_color: Some(palette.text_muted),
        border: border::rounded(6)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.1))
            .width(1),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn recording_item_style(
    theme: WorkspaceTheme,
    is_dragging: bool,
    is_hovered: bool,
) -> container::Style {
    container::Style {
        background: if is_dragging {
            Some(theme.selected_surface().into())
        } else if is_hovered {
            Some(theme.surface().into())
        } else {
            None
        },
        text_color: Some(Color::from_rgb8(236, 234, 228)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn recording_scrollable_style(_: &Theme, status: scrollable::Status) -> scrollable::Style {
    let is_hovered_or_dragged = matches!(
        status,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        }
    );
    let thumb = if is_hovered_or_dragged {
        Color::from_rgb8(156, 163, 175)
    } else {
        Color::from_rgb8(209, 213, 219)
    };
    let rail = scrollable::Rail {
        background: None,
        border: border::rounded(999).width(0),
        scroller: scrollable::Scroller {
            background: thumb.into(),
            border: border::rounded(999).width(0),
        },
    };

    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Color::from_rgba(0.0, 0.0, 0.0, 0.0).into(),
            border: border::rounded(999).width(0),
            shadow: Shadow::default(),
            icon: thumb,
        },
    }
}

fn video_modal_dimensions(source_width: u32, source_height: u32) -> (f32, f32, f32) {
    const MAX_VIDEO_WIDTH: f32 = 920.0;
    const MAX_VIDEO_HEIGHT: f32 = 370.0;
    const MIN_CONTROLS_WIDTH: f32 = 360.0;
    const PANEL_HORIZONTAL_PADDING: f32 = 36.0;

    let source_width = source_width.max(1) as f32;
    let source_height = source_height.max(1) as f32;
    let scale = (MAX_VIDEO_WIDTH / source_width)
        .min(MAX_VIDEO_HEIGHT / source_height)
        .min(1.0);
    let video_width = (source_width * scale).max(1.0);
    let video_height = (source_height * scale).max(1.0);
    let panel_width = video_width.max(MIN_CONTROLS_WIDTH) + PANEL_HORIZONTAL_PADDING;

    (video_width, video_height, panel_width)
}

fn video_surface_style(palette: component_styles::Palette) -> container::Style {
    container::Style {
        background: Some(
            Color::from_rgb8(18, 18, 17)
                .scale_alpha(palette.panel.a)
                .into(),
        ),
        text_color: Some(palette.text),
        border: border::rounded(7)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.1).scale_alpha(palette.panel.a))
            .width(1),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn recording_save_button_style(
    status: button::Status,
    accent: Color,
    hover: Color,
) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => hover,
        button::Status::Disabled => Color::from_rgb8(43, 43, 40).scale_alpha(accent.a),
        button::Status::Active => accent,
    };
    let text_color = if matches!(status, button::Status::Disabled) {
        Color::from_rgb8(126, 124, 118).scale_alpha(accent.a)
    } else {
        Color::from_rgb8(232, 242, 255).scale_alpha(accent.a)
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

async fn scan_recordings(folder: PathBuf) -> Vec<RecordingItem> {
    let mut recordings = fs::read_dir(folder)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_supported_video_file(path))
        .filter_map(|path| {
            let metadata = fs::metadata(&path).ok()?;
            let file_name = file_name_string(&path)?;
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("mp4")
                .to_ascii_uppercase();
            let title = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&file_name)
                .to_string();

            Some(RecordingItem {
                duration: probe_video_duration(&path),
                thumbnail_path: create_cached_video_thumbnail(&path),
                modified: metadata.modified().unwrap_or(UNIX_EPOCH),
                path,
                file_name,
                title,
                extension,
            })
        })
        .collect::<Vec<_>>();

    recordings.sort_by(|a, b| b.modified.cmp(&a.modified));
    recordings
}

fn is_supported_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "mkv" | "webm"
            )
        })
        .unwrap_or(false)
}

fn file_name_string(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|file_name| file_name.to_str())
        .map(ToOwned::to_owned)
}

fn validate_recording_title(title: &str, extension: &str) -> Option<String> {
    if title.is_empty() {
        return Some("Recording title cannot be empty.".to_string());
    }

    if title.chars().count() > 140 {
        return Some("Recording title must be 140 characters or fewer.".to_string());
    }

    if title == "."
        || title == ".."
        || title
            .chars()
            .any(|character| character == '/' || character == '\\' || character.is_control())
    {
        return Some("Recording title cannot contain path separators.".to_string());
    }

    let extension_suffix = format!(".{}", extension.to_ascii_lowercase());
    if title.to_ascii_lowercase().ends_with(&extension_suffix) {
        return Some(format!("Leave {extension_suffix} outside the title box."));
    }

    None
}

#[derive(Debug, Default)]
struct VideoMetadata {
    duration: Option<Duration>,
    dimensions: Option<(u32, u32)>,
}

#[derive(serde::Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(serde::Deserialize)]
struct FfprobeStream {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(serde::Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

fn probe_video_duration(video_path: &Path) -> Option<Duration> {
    probe_video_metadata(video_path).duration
}

fn probe_video_metadata(video_path: &Path) -> VideoMetadata {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height:format=duration")
        .arg("-of")
        .arg("json")
        .arg(video_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let Ok(output) = output else {
        return VideoMetadata::default();
    };

    if !output.status.success() {
        return VideoMetadata::default();
    }

    let Ok(output) = serde_json::from_slice::<FfprobeOutput>(&output.stdout) else {
        return VideoMetadata::default();
    };

    let duration = output
        .format
        .and_then(|format| format.duration)
        .and_then(|duration| duration.parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite())
        .map(|seconds| Duration::from_secs_f64(seconds.max(0.0)));
    let dimensions = output.streams.into_iter().find_map(|stream| {
        let width = stream.width?;
        let height = stream.height?;

        (width > 0 && height > 0).then_some((width, height))
    });

    VideoMetadata {
        duration,
        dimensions,
    }
}

fn create_cached_video_thumbnail(video_path: &Path) -> Option<PathBuf> {
    let thumbnail_dir = std::env::temp_dir().join("roton-recording-thumbnails");
    fs::create_dir_all(&thumbnail_dir).ok()?;

    let thumbnail_path = thumbnail_dir.join(format!("{}.png", thumbnail_cache_key(video_path)));

    if thumbnail_path.exists() {
        return Some(thumbnail_path);
    }

    create_video_thumbnail_at(video_path, &thumbnail_path).then_some(thumbnail_path)
}

fn thumbnail_cache_key(video_path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    video_path.hash(&mut hasher);

    if let Ok(metadata) = fs::metadata(video_path) {
        metadata.len().hash(&mut hasher);
        metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or_default()
            .hash(&mut hasher);
    }

    hasher.finish()
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

    create_video_frame_at(video_path, &thumbnail_path, 1.0, 320).then_some(thumbnail_path)
}

fn create_video_thumbnail_at(video_path: &Path, thumbnail_path: &Path) -> bool {
    create_video_frame_at(video_path, thumbnail_path, 1.0, 320)
}

fn create_video_frame_at(
    video_path: &Path,
    output_path: &Path,
    position_seconds: f64,
    width: u32,
) -> bool {
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(format!("{:.3}", position_seconds.max(0.0)))
        .arg("-i")
        .arg(video_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!("scale={width}:-1"))
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok();

    status.is_some_and(|status| status.success())
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

fn format_elapsed(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds / 60) % 60;
    let seconds = total_seconds % 60;
    let centiseconds = duration.subsec_millis() / 10;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}.{centiseconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}.{centiseconds:02}")
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds / 60) % 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn scroll_delta_to_offset(delta: mouse::ScrollDelta) -> scrollable::AbsoluteOffset {
    match delta {
        mouse::ScrollDelta::Lines { x, y } => scrollable::AbsoluteOffset {
            x: -x * 150.0,
            y: -y * 150.0,
        },
        mouse::ScrollDelta::Pixels { x, y } => scrollable::AbsoluteOffset {
            x: -x * 2.4,
            y: -y * 2.4,
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_elapsed_under_an_hour() {
        assert_eq!(format_elapsed(Duration::from_millis(62_345)), "01:02.34");
    }

    #[test]
    fn formats_elapsed_at_an_hour() {
        assert_eq!(
            format_elapsed(Duration::from_millis(3_661_007)),
            "01:01:01.00"
        );
    }
}
