use super::{overlay::default_cursor, styles};
use iced::widget::{button, svg, text};
use iced::{border, Color, Element, Shadow};

#[derive(Debug, Clone, Copy)]
pub enum Variant {
    Secondary,
    Danger,
    Side,
}

pub fn themed_text_button<'a, Message: Clone + 'a>(
    label: &'a str,
    variant: Variant,
    on_press: Option<Message>,
    palette: styles::Palette,
) -> Element<'a, Message> {
    default_cursor(
        button(text(label).size(13))
            .padding([10, 14])
            .style(move |_, status| style_with_palette(status, variant, palette))
            .on_press_maybe(on_press),
    )
}

pub fn accent_text_button<'a, Message: Clone + 'a>(
    label: &'a str,
    on_press: Option<Message>,
    accent: Color,
    hovered_accent: Color,
) -> Element<'a, Message> {
    default_cursor(
        button(text(label).size(13))
            .padding([10, 14])
            .style(move |_, status| primary_style(status, accent, hovered_accent))
            .on_press_maybe(on_press),
    )
}

pub fn themed_compact_icon_button<'a, Message: Clone + 'a>(
    icon: impl Into<svg::Handle>,
    variant: Variant,
    on_press: Option<Message>,
    palette: styles::Palette,
) -> Element<'a, Message> {
    default_cursor(
        button(svg(icon).width(18).height(18))
            .width(38)
            .height(38)
            .padding(10)
            .style(move |_, status| style_with_palette(status, variant, palette))
            .on_press_maybe(on_press),
    )
}

fn primary_style(status: button::Status, accent: Color, hovered_accent: Color) -> button::Style {
    let alpha = accent.a;
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => hovered_accent,
        button::Status::Disabled => Color::from_rgb8(43, 43, 40).scale_alpha(alpha),
        button::Status::Active => accent,
    };
    let text_color = if matches!(status, button::Status::Disabled) {
        Color::from_rgb8(126, 124, 118).scale_alpha(alpha)
    } else {
        Color::from_rgb8(232, 242, 255).scale_alpha(alpha)
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: border::rounded(8).width(0),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn style_with_palette(
    status: button::Status,
    variant: Variant,
    palette: styles::Palette,
) -> button::Style {
    let (background, hovered_background, text_color, border) = match variant {
        Variant::Secondary => (
            palette.field,
            palette.field_hover,
            palette.text,
            border::rounded(7).color(palette.separator).width(1),
        ),
        Variant::Danger => (
            Color::from_rgb8(170, 43, 48).scale_alpha(palette.panel.a),
            Color::from_rgb8(142, 34, 40).scale_alpha(palette.panel.a),
            Color::from_rgb8(255, 235, 235).scale_alpha(palette.panel.a),
            border::rounded(8).width(0),
        ),
        Variant::Side => (
            palette.field,
            palette.field_hover,
            palette.text,
            border::rounded(8).width(0),
        ),
    };

    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(hovered_background.into()),
            text_color,
            border,
            shadow: Shadow::default(),
            snap: false,
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.field_disabled.into()),
            text_color: palette.text_disabled,
            border,
            shadow: Shadow::default(),
            snap: false,
        },
        button::Status::Active => button::Style {
            background: Some(background.into()),
            text_color,
            border,
            shadow: Shadow::default(),
            snap: false,
        },
    }
}
