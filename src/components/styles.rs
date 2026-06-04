use iced::widget::{button, container, text_input as text_input_widget};
use iced::{border, Color, Shadow, Theme};

pub fn dialog_panel(progress: f32) -> container::Style {
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

pub fn scrim(progress: f32) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.42 * progress).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
    }
}

pub fn input_surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(52, 52, 49).into()),
        text_color: Some(Color::from_rgb8(214, 212, 205)),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

pub fn text_input(_: &Theme, _: text_input_widget::Status) -> text_input_widget::Style {
    text_input_widget::Style {
        background: Color::from_rgb8(52, 52, 49).into(),
        border: border::rounded(7).width(0),
        icon: Color::from_rgb8(214, 212, 205),
        placeholder: Color::from_rgb8(138, 136, 130),
        value: Color::from_rgb8(214, 212, 205),
        selection: Color::from_rgb8(47, 94, 158),
    }
}

pub fn dropdown_field(_: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => (
            Color::from_rgb8(58, 58, 54),
            Color::from_rgb8(232, 230, 222),
        ),
        button::Status::Disabled => (
            Color::from_rgb8(45, 45, 42),
            Color::from_rgb8(138, 136, 130),
        ),
        button::Status::Active => (
            Color::from_rgb8(52, 52, 49),
            Color::from_rgb8(214, 212, 205),
        ),
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
    }
}

pub fn dropdown_menu(_: &Theme) -> container::Style {
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

pub fn dropdown_option(_: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => (
            Color::from_rgb8(66, 66, 62),
            Color::from_rgb8(246, 244, 238),
        ),
        button::Status::Disabled => (Color::TRANSPARENT, Color::from_rgb8(126, 124, 118)),
        button::Status::Active => (Color::TRANSPARENT, Color::from_rgb8(232, 230, 222)),
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(5).width(0),
        shadow: Shadow::default(),
    }
}

pub fn context_menu(theme: &Theme) -> container::Style {
    dropdown_menu(theme)
}

pub fn context_menu_option(theme: &Theme, status: button::Status) -> button::Style {
    dropdown_option(theme, status)
}

pub fn separator(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(74, 74, 70).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(1).width(0),
        shadow: Shadow::default(),
    }
}
