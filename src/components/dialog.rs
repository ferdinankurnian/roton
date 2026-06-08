use super::{overlay::event_blocker, styles};
use iced::widget::{container, mouse_area, Space};
use iced::{Element, Length};

pub fn backdrop<'a, Message: Clone + 'a>(progress: f32, on_press: Message) -> Element<'a, Message> {
    event_blocker(
        mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| styles::scrim(progress)),
        )
        .on_press(on_press),
    )
}

pub fn panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    width: f32,
    progress: f32,
    palette: styles::Palette,
) -> Element<'a, Message> {
    let eased_width = width * (0.96 + (0.04 * progress));
    let eased_padding = 14.0 + (4.0 * progress);

    container(content)
        .width(eased_width)
        .padding(eased_padding)
        .style(move |_| styles::dialog_panel_with_palette(progress, palette))
        .into()
}
