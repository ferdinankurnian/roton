use super::styles;
use iced::widget::{container, text, text_input, TextInput};
use iced::{Element, Length};

pub fn field<'a, Message: Clone + 'a>(
    placeholder: &'a str,
    value: &'a str,
    id: &'static str,
    on_input: impl Fn(String) -> Message + 'a,
    on_submit: Message,
    palette: styles::Palette,
) -> TextInput<'a, Message> {
    text_input(placeholder, value)
        .id(id)
        .on_input(on_input)
        .on_submit(on_submit)
        .padding(10)
        .size(13)
        .style(move |_, _| styles::text_input_with_palette(palette))
}

pub fn readonly<'a, Message: 'a>(
    value: impl Into<String>,
    palette: styles::Palette,
) -> Element<'a, Message> {
    container(text(value.into()).size(13))
        .padding(10)
        .width(Length::Fill)
        .style(move |_| styles::input_surface_with_palette(palette))
        .into()
}
