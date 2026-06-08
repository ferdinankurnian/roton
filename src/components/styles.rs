use iced::widget::{button, container, text_input as text_input_widget};
use iced::{border, Color, Shadow};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub panel: Color,
    pub field: Color,
    pub field_hover: Color,
    pub field_disabled: Color,
    pub menu: Color,
    pub option_hover: Color,
    pub option_selected: Color,
    pub option_selected_hover: Color,
    pub separator: Color,
    pub text: Color,
    pub text_strong: Color,
    pub text_muted: Color,
    pub text_disabled: Color,
    pub selection: Color,
}

impl Palette {
    pub fn neutral() -> Self {
        Self {
            panel: Color::from_rgb8(42, 42, 39),
            field: Color::from_rgb8(52, 52, 49),
            field_hover: Color::from_rgb8(58, 58, 54),
            field_disabled: Color::from_rgb8(45, 45, 42),
            menu: Color::from_rgb8(52, 52, 49),
            option_hover: Color::from_rgb8(66, 66, 62),
            option_selected: Color::from_rgb8(61, 61, 57),
            option_selected_hover: Color::from_rgb8(72, 72, 68),
            separator: Color::from_rgb8(74, 74, 70),
            text: Color::from_rgb8(214, 212, 205),
            text_strong: Color::from_rgb8(246, 244, 238),
            text_muted: Color::from_rgb8(138, 136, 130),
            text_disabled: Color::from_rgb8(126, 124, 118),
            selection: Color::from_rgb8(47, 94, 158),
        }
    }

    pub fn scale_alpha(self, factor: f32) -> Self {
        Self {
            panel: self.panel.scale_alpha(factor),
            field: self.field.scale_alpha(factor),
            field_hover: self.field_hover.scale_alpha(factor),
            field_disabled: self.field_disabled.scale_alpha(factor),
            menu: self.menu.scale_alpha(factor),
            option_hover: self.option_hover.scale_alpha(factor),
            option_selected: self.option_selected.scale_alpha(factor),
            option_selected_hover: self.option_selected_hover.scale_alpha(factor),
            separator: self.separator.scale_alpha(factor),
            text: self.text.scale_alpha(factor),
            text_strong: self.text_strong.scale_alpha(factor),
            text_muted: self.text_muted.scale_alpha(factor),
            text_disabled: self.text_disabled.scale_alpha(factor),
            selection: self.selection.scale_alpha(factor),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::neutral()
    }
}

pub fn dialog_panel_with_palette(progress: f32, palette: Palette) -> container::Style {
    container::Style {
        background: Some(palette.panel.into()),
        text_color: Some(palette.text_strong),
        border: border::rounded(8).width(0),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35 * progress),
            offset: iced::Vector::new(0.0, 10.0 * progress),
            blur_radius: 24.0 * progress,
        },
        snap: false,
    }
}

pub fn scrim(progress: f32) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.42 * progress).into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn input_surface_with_palette(palette: Palette) -> container::Style {
    container::Style {
        background: Some(palette.field.into()),
        text_color: Some(palette.text),
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn text_input_with_palette(palette: Palette) -> text_input_widget::Style {
    text_input_widget::Style {
        background: palette.field.into(),
        border: border::rounded(7).width(0),
        icon: palette.text,
        placeholder: palette.text_muted,
        value: palette.text,
        selection: palette.selection,
    }
}

pub fn dropdown_field_with_palette(status: button::Status, palette: Palette) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => (palette.field_hover, palette.text),
        button::Status::Disabled => (palette.field_disabled, palette.text_muted),
        button::Status::Active => (palette.field, palette.text),
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(7).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn dropdown_menu_with_palette(palette: Palette) -> container::Style {
    container::Style {
        background: Some(palette.menu.into()),
        text_color: Some(palette.text),
        border: border::rounded(7).width(0),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
        snap: false,
    }
}

pub fn dropdown_option_with_palette(status: button::Status, palette: Palette) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => {
            (palette.option_hover, palette.text_strong)
        }
        button::Status::Disabled => (Color::TRANSPARENT, palette.text_disabled),
        button::Status::Active => (Color::TRANSPARENT, palette.text),
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(5).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn dropdown_option_selected_with_palette(
    status: button::Status,
    palette: Palette,
) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => {
            (palette.option_selected_hover, palette.text_strong)
        }
        button::Status::Disabled => (palette.field_disabled, palette.text_disabled),
        button::Status::Active => (palette.option_selected, palette.text_strong),
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(5).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn context_menu_with_palette(palette: Palette) -> container::Style {
    dropdown_menu_with_palette(palette)
}

pub fn context_menu_option_with_palette(status: button::Status, palette: Palette) -> button::Style {
    dropdown_option_with_palette(status, palette)
}

pub fn separator_with_palette(palette: Palette) -> container::Style {
    container::Style {
        background: Some(palette.separator.into()),
        text_color: Some(Color::TRANSPARENT),
        border: border::rounded(1).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}
