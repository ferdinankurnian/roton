use super::styles;
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::Renderer as _;
use iced::advanced::{Clipboard, Overlay, Shell, Widget};
use iced::event::Event;
use iced::widget::{button, column, container, row, svg, text, Space};
use iced::{
    alignment, Background, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
};

pub enum Entry<Message> {
    Option(OptionItem<Message>),
    Separator,
}

impl<Message> From<OptionItem<Message>> for Entry<Message> {
    fn from(option: OptionItem<Message>) -> Self {
        Self::Option(option)
    }
}

pub struct OptionItem<Message> {
    label: String,
    on_select: Message,
}

impl<Message> OptionItem<Message> {
    pub fn new(label: impl Into<String>, on_select: Message) -> Self {
        Self {
            label: label.into(),
            on_select,
        }
    }
}

pub fn select<'a, Message: Clone + 'a>(
    selected: impl Into<String>,
    entries: impl IntoIterator<Item = Entry<Message>>,
    max_chars: usize,
    disabled: bool,
    palette: styles::Palette,
) -> Element<'a, Message> {
    let selected = selected.into();
    let field = container(
        row![
            text(truncate_text(&selected, max_chars)).size(13),
            Space::new().width(Length::Fill),
            svg("assets/icons/chevrons-up-down.svg")
                .width(15)
                .height(15),
        ]
        .align_y(alignment::Vertical::Center),
    )
    .padding(10)
    .width(Length::Fill);

    let options = entries
        .into_iter()
        .fold(column![].spacing(0), |column, entry| match entry {
            Entry::Option(option) => {
                let is_selected = option.label == selected;
                let marker: Element<_> = if is_selected {
                    svg("assets/icons/check.svg").width(15).height(15).into()
                } else {
                    Space::new().width(15).height(15).into()
                };

                column.push(
                    button(
                        row![
                            text(option.label).size(13),
                            Space::new().width(Length::Fill),
                            marker,
                        ]
                        .spacing(10)
                        .align_y(alignment::Vertical::Center),
                    )
                    .padding([9, 10])
                    .width(Length::Fill)
                    .style(move |_, status| {
                        if is_selected {
                            styles::dropdown_option_selected_with_palette(status, palette)
                        } else {
                            styles::dropdown_option_with_palette(status, palette)
                        }
                    })
                    .on_press(option.on_select),
                )
            }
            Entry::Separator => column.push(
                container(Space::new().height(1))
                    .width(Length::Fill)
                    .style(move |_| styles::separator_with_palette(palette)),
            ),
        });

    Element::new(Dropdown {
        field: field.into(),
        menu: container(options)
            .padding([4, 0])
            .width(Length::Fill)
            .style(move |_| styles::dropdown_menu_with_palette(palette))
            .into(),
        disabled,
        palette,
    })
}

struct Dropdown<'a, Message> {
    field: Element<'a, Message>,
    menu: Element<'a, Message>,
    disabled: bool,
    palette: styles::Palette,
}

#[derive(Debug, Default)]
struct State {
    is_open: bool,
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Dropdown<'_, Message> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.field), Tree::new(&self.menu)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.field, &self.menu]);
    }

    fn size(&self) -> Size<Length> {
        self.field.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.field
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.field
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if !self.disabled && cursor.is_over(layout.bounds()) {
                    state.is_open = !state.is_open;
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if tree.state.downcast_ref::<State>().is_open {
            return;
        }

        let status = if self.disabled {
            button::Status::Disabled
        } else if cursor.is_over(layout.bounds()) {
            button::Status::Hovered
        } else {
            button::Status::Active
        };
        let style = styles::dropdown_field_with_palette(status, self.palette);

        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: style.border,
                shadow: style.shadow,
                snap: style.snap,
            },
            style
                .background
                .unwrap_or(Background::Color(iced::Color::TRANSPARENT)),
        );

        self.field.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style {
                text_color: style.text_color,
            },
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();

        state.is_open.then(|| {
            iced::advanced::overlay::Element::new(Box::new(DropdownOverlay {
                menu: &mut self.menu,
                tree: &mut tree.children[1],
                position: layout.position() + translation,
                width: layout.bounds().width,
                is_open: &mut state.is_open,
            }))
        })
    }
}

struct DropdownOverlay<'a, 'b, Message> {
    menu: &'a mut Element<'b, Message>,
    tree: &'a mut Tree,
    position: Point,
    width: f32,
    is_open: &'a mut bool,
}

impl<Message: Clone> Overlay<Message, Theme, Renderer> for DropdownOverlay<'_, '_, Message> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let menu = self.menu.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, Size::new(self.width, bounds.height))
                .width(self.width),
        );
        let size = menu.size();
        let y = self.position.y.min((bounds.height - size.height).max(0.0));

        menu.move_to(Point::new(self.position.x, y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.menu.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.menu
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let is_over = cursor.is_over(layout.bounds());

        if !is_over
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    | Event::Touch(iced::touch::Event::FingerPressed { .. })
            )
        {
            *self.is_open = false;
            shell.capture_event();
            return;
        }

        self.menu.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );

        if shell.is_event_captured()
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                    | Event::Touch(iced::touch::Event::FingerLifted { .. })
            )
        {
            *self.is_open = false;
        }

        if is_over {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            self.menu.as_widget().mouse_interaction(
                self.tree,
                layout,
                cursor,
                &layout.bounds(),
                renderer,
            )
        }
    }
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
