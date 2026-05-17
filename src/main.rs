mod audio;
mod config;
mod recorder;

use audio::AudioDevice;
use config::Settings;
use display_info::DisplayInfo;
use iced::widget::{Space, column, container, image, mouse_area, row, stack, svg, text};
use iced::{
    Color, Element, Length, Shadow, Subscription, Task, Theme, alignment, application, border,
    time, window,
};
use notify_rust::Notification;
use recorder::Recorder;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

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
    OpenNode(NodeKind),
    HoverNode(Option<NodeKind>),
    HoverTitlebarClose(bool),
    HoverModalClose(bool),
    HoverRecord(bool),
    HoverPause(bool),
    HoverChoose(bool),
    HoverScreenMode(Option<ScreenMode>),
    HoverMicToggle(bool),
    HoverShowCursor(bool),
    HoverScreenSound(bool),
    PauseBlinkTick,
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
    ToggleShowCursor,
    ToggleScreenSound,
    ChooseOutputFolder,
    OutputFolderChosen(Option<String>),
    Noop,
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
    show_cursor: bool,
    record_screen_sound: bool,
    selected_node: Option<NodeKind>,
    hovered_node: Option<NodeKind>,
    is_titlebar_close_hovered: bool,
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
            show_cursor: true,
            record_screen_sound: false,
            selected_node: None,
            hovered_node: None,
            is_titlebar_close_hovered: false,
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
                    self.is_recording = false;
                    self.is_paused = false;
                    notify_recording_completed(self.settings.save_path.clone());
                } else {
                    self.is_recording = true;
                    self.is_paused = false;
                    self.pause_blink_on = true;
                    self.is_format_dropdown_open = false;
                    self.is_monitor_dropdown_open = false;
                    self.is_mic_dropdown_open = false;
                    self.hovered_dropdown_option = None;
                }
            }
            Message::TogglePause => {
                if self.is_recording {
                    self.is_paused = !self.is_paused;
                    if !self.is_paused {
                        self.pause_blink_on = true;
                    }
                }
            }
            Message::DragWindow => {
                return window::get_latest().and_then(window::drag);
            }
            Message::CloseWindow => {
                return window::get_latest().and_then(window::close);
            }
            Message::OpenNode(kind) => {
                self.selected_node = Some(kind);
                self.hovered_node = None;
                self.modal_progress = 0.0;
            }
            Message::HoverNode(kind) => {
                if self.selected_node.is_none() {
                    self.hovered_node = kind;
                }
            }
            Message::HoverTitlebarClose(is_hovered) => {
                self.is_titlebar_close_hovered = is_hovered;
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
            Message::Frame => {
                if self.selected_node.is_some() {
                    self.modal_progress = (self.modal_progress + 0.16).min(1.0);
                }
            }
            Message::CloseModal => {
                self.selected_node = None;
                self.hovered_node = None;
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

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        if self.selected_node.is_some() && self.modal_progress < 1.0 {
            subscriptions.push(window::frames().map(|_| Message::Frame));
        }

        if self.is_paused {
            subscriptions.push(
                time::every(std::time::Duration::from_millis(420)).map(|_| Message::PauseBlinkTick),
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

        if let Some(kind) = self.selected_node {
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
            app.into()
        }
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
        container(
            mouse_area(
                row![
                    container(image("assets/rotonicon.png").width(16).height(16)).padding([0, 10]),
                    Space::with_width(Length::Fill),
                    text("Roton").size(15),
                    Space::with_width(Length::Fill),
                    mouse_area(
                        container(svg("assets/icons/x.svg").width(15).height(15))
                            .padding(8)
                            .style(if self.is_titlebar_close_hovered {
                                titlebar_close_hovered
                            } else {
                                titlebar_close
                            })
                    )
                    .on_enter(Message::HoverTitlebarClose(true))
                    .on_exit(Message::HoverTitlebarClose(false))
                    .on_press(Message::CloseWindow),
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

    fn canvas(&self) -> Element<Message> {
        container(
            column![
                container(self.node(NodeKind::Output))
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Center),
                row![self.node(NodeKind::Screen), self.node(NodeKind::Mic),]
                    .spacing(24)
                    .align_y(alignment::Vertical::Center),
            ]
            .spacing(10)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(28)
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
            .width(148)
            .height(134)
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
            NodeKind::Screen => self.screen_mode.label().to_string(),
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
                        Message::SelectScreenMode(ScreenMode::SelectArea)
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
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn selected_mode_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(54, 59, 64).into()),
        text_color: Some(Color::from_rgb8(235, 242, 250)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn mode_card_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(43, 43, 40).into()),
        text_color: Some(Color::from_rgb8(132, 130, 124)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn node_card_hovered(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(50, 50, 47).into()),
        text_color: Some(Color::from_rgb8(244, 242, 235)),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

fn node_card_disabled(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(41, 41, 38).into()),
        text_color: Some(Color::from_rgb8(150, 148, 140)),
        border: border::rounded(8).width(0),
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

fn notify_recording_completed(save_path: String) {
    thread::spawn(move || {
        let notification = Notification::new()
            .summary("Recording completed!")
            .body("Click to open in file manager")
            .icon("video-x-generic")
            .action("open", "Open")
            .show();

        if let Ok(handle) = notification {
            handle.wait_for_action(|action| {
                if action == "default" || action == "open" {
                    let _ = Command::new("xdg-open").arg(&save_path).spawn();
                }
            });
        }
    });
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
