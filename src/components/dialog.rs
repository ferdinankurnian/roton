use super::{overlay::event_blocker, styles};
use iced::widget::{container, mouse_area, Space};
use iced::{Element, Length};

pub fn backdrop<'a, Message: Clone + 'a>(progress: f32, on_press: Message) -> Element<'a, Message> {
    event_blocker(
        mouse_area(
            container(Space::with_width(Length::Fill).height(Length::Fill))
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
    container(content)
        .width(width)
        .padding(18)
        .style(move |_| styles::dialog_panel_with_palette(progress, palette))
        .into()
}
