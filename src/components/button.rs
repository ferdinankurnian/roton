use super::overlay::default_cursor;
use iced::widget::{button, svg, text};
use iced::{border, Color, Element, Length, Shadow, Theme};

#[derive(Debug, Clone, Copy)]
pub enum Variant {
    Primary,
    Secondary,
    Danger,
    Side,
}

pub fn text_button<'a, Message: Clone + 'a>(
    label: &'a str,
    variant: Variant,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    default_cursor(
        button(text(label).size(13))
            .padding([10, 14])
            .style(move |theme, status| style(theme, status, variant))
            .on_press_maybe(on_press),
    )
}

pub fn fill_text_button<'a, Message: Clone + 'a>(
    label: &'a str,
    variant: Variant,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    default_cursor(
        button(text(label).size(13))
            .padding([10, 14])
            .width(Length::Fill)
            .style(move |theme, status| style(theme, status, variant))
            .on_press_maybe(on_press),
    )
}

pub fn compact_icon_button<'a, Message: Clone + 'a>(
    icon: impl Into<svg::Handle>,
    variant: Variant,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    default_cursor(
        button(svg(icon).width(18).height(18))
            .width(38)
            .height(38)
            .padding(10)
            .style(move |theme, status| style(theme, status, variant))
            .on_press_maybe(on_press),
    )
}

fn style(_: &Theme, status: button::Status, variant: Variant) -> button::Style {
    let (background, hovered_background, text_color, border) = match variant {
        Variant::Primary => (
            Color::from_rgb8(47, 94, 158),
            Color::from_rgb8(37, 79, 138),
            Color::from_rgb8(232, 242, 255),
            border::rounded(8).width(0),
        ),
        Variant::Secondary => (
            Color::from_rgb8(43, 43, 40),
            Color::from_rgb8(55, 55, 51),
            Color::from_rgb8(210, 208, 200),
            border::rounded(7)
                .color(Color::from_rgb8(76, 75, 70))
                .width(1),
        ),
        Variant::Danger => (
            Color::from_rgb8(170, 43, 48),
            Color::from_rgb8(142, 34, 40),
            Color::from_rgb8(255, 235, 235),
            border::rounded(8).width(0),
        ),
        Variant::Side => (
            Color::from_rgb8(47, 47, 44),
            Color::from_rgb8(58, 58, 54),
            Color::from_rgb8(230, 228, 220),
            border::rounded(8).width(0),
        ),
    };

    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(hovered_background.into()),
            text_color,
            border,
            shadow: Shadow::default(),
        },
        button::Status::Disabled => button::Style {
            background: Some(Color::from_rgb8(43, 43, 40).into()),
            text_color: Color::from_rgb8(126, 124, 118),
            border,
            shadow: Shadow::default(),
        },
        button::Status::Active => button::Style {
            background: Some(background.into()),
            text_color,
            border,
            shadow: Shadow::default(),
        },
    }
}
