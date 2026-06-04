use super::{overlay::event_blocker, styles};
use iced::widget::{button, column, container, text};
use iced::{alignment, Element, Length};

pub struct Item<'a, Message> {
    label: &'a str,
    on_press: Option<Message>,
}

impl<'a, Message> Item<'a, Message> {
    pub fn new(label: &'a str, on_press: Message) -> Self {
        Self {
            label,
            on_press: Some(on_press),
        }
    }

    pub fn disabled(label: &'a str) -> Self {
        Self {
            label,
            on_press: None,
        }
    }
}

pub fn view<'a, Message: Clone + 'a>(
    items: impl IntoIterator<Item = Item<'a, Message>>,
) -> Element<'a, Message> {
    let items = items
        .into_iter()
        .fold(column![].spacing(0), |column, item| {
            column.push(menu_item(item))
        });

    event_blocker(
        container(items)
            .padding([4, 0])
            .width(Length::Fill)
            .style(|_| styles::context_menu_with_palette(styles::Palette::default())),
    )
}

fn menu_item<'a, Message: Clone + 'a>(item: Item<'a, Message>) -> Element<'a, Message> {
    button(
        text(item.label)
            .size(13)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Left),
    )
        .padding([9, 10])
        .width(Length::Fill)
        .style(|_, status| {
            styles::context_menu_option_with_palette(status, styles::Palette::default())
        })
        .on_press_maybe(item.on_press)
        .into()
}
