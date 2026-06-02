use super::styles;
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::Renderer as _;
use iced::advanced::{Clipboard, Overlay, Shell, Widget};
use iced::event::{self, Event};
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
) -> Element<'a, Message> {
    let field = container(
        row![
            text(truncate_text(&selected.into(), max_chars)).size(13),
            Space::with_width(Length::Fill),
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
            Entry::Option(option) => column.push(
                button(text(option.label).size(13))
                    .padding([9, 10])
                    .width(Length::Fill)
                    .style(styles::dropdown_option)
                    .on_press(option.on_select),
            ),
            Entry::Separator => column.push(
                container(Space::with_height(1))
                    .width(Length::Fill)
                    .style(styles::separator),
            ),
        });

    Element::new(Dropdown {
        field: field.into(),
        menu: container(options)
            .padding([4, 0])
            .width(Length::Fill)
            .style(styles::dropdown_menu)
            .into(),
        disabled,
    })
}

struct Dropdown<'a, Message> {
    field: Element<'a, Message>,
    menu: Element<'a, Message>,
    disabled: bool,
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
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.field
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.field
            .as_widget()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> event::Status {
        let state = tree.state.downcast_mut::<State>();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if self.disabled || !cursor.is_over(layout.bounds()) {
                    event::Status::Ignored
                } else {
                    state.is_open = !state.is_open;
                    event::Status::Captured
                }
            }
            _ => event::Status::Ignored,
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
        let style = styles::dropdown_field(theme, status);

        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: style.border,
                shadow: style.shadow,
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
        layout: Layout<'_>,
        _renderer: &Renderer,
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
        let menu = self.menu.as_widget().layout(
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
            .as_widget()
            .operate(self.tree, layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        let is_over = cursor.is_over(layout.bounds());

        if !is_over
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    | Event::Touch(iced::touch::Event::FingerPressed { .. })
            )
        {
            *self.is_open = false;
            return event::Status::Captured;
        }

        let status = self.menu.as_widget_mut().on_event(
            self.tree,
            event.clone(),
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );

        if status == event::Status::Captured
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                    | Event::Touch(iced::touch::Event::FingerLifted { .. })
            )
        {
            *self.is_open = false;
        }

        if status == event::Status::Captured || is_over {
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            self.menu
                .as_widget()
                .mouse_interaction(self.tree, layout, cursor, viewport, renderer)
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
