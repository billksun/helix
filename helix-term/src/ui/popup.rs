use crate::{
    compositor::{Component, Compositor, Context, EventResult},
    ctrl, key,
};
use crossterm::event::Event;
use tui::buffer::Buffer as Surface;

use helix_core::Position;
use helix_view::graphics::{Margin, Rect};

// TODO: share logic with Menu, it's essentially Popup(render_fn), but render fn needs to return
// a width/height hint. maybe Popup(Box<Component>)

pub struct Popup<T: Component> {
    contents: T,
    position: Option<Position>,
    margin: Margin,
    size: (u16, u16),
    child_size: (u16, u16),
    scroll: usize,
    id: &'static str,
}

impl<T: Component> Popup<T> {
    pub fn new(id: &'static str, contents: T) -> Self {
        Self {
            contents,
            position: None,
            margin: Margin {
                vertical: 0,
                horizontal: 0,
            },
            size: (0, 0),
            child_size: (0, 0),
            scroll: 0,
            id,
        }
    }

    pub fn set_position(&mut self, pos: Option<Position>) {
        self.position = pos;
    }

    pub fn margin(mut self, margin: Margin) -> Self {
        self.margin = margin;
        self
    }

    pub fn get_rel_position(&mut self, viewport: Rect, cx: &Context) -> (u16, u16) {
        let position = self
            .position
            .get_or_insert_with(|| cx.editor.cursor().0.unwrap_or_default());

        let (width, height) = self.size;

        // if there's a orientation preference, use that
        // if we're on the top part of the screen, do below
        // if we're on the bottom part, do above

        // -- make sure frame doesn't stick out of bounds
        let mut rel_x = position.col as u16;
        let mut rel_y = position.row as u16;
        if viewport.width <= rel_x + width {
            rel_x = rel_x.saturating_sub((rel_x + width).saturating_sub(viewport.width));
        }

        // TODO: be able to specify orientation preference. We want above for most popups, below
        // for menus/autocomplete.
        if viewport.height > rel_y + height {
            rel_y += 1 // position below point
        } else {
            rel_y = rel_y.saturating_sub(height) // position above point
        }

        (rel_x, rel_y)
    }

    pub fn get_size(&self) -> (u16, u16) {
        (self.size.0, self.size.1)
    }

    pub fn scroll(&mut self, offset: usize, direction: bool) {
        if direction {
            self.scroll += offset;

            let max_offset = self.child_size.1.saturating_sub(self.size.1);
            self.scroll = (self.scroll + offset).min(max_offset as usize);
        } else {
            self.scroll = self.scroll.saturating_sub(offset);
        }
    }

    pub fn contents(&self) -> &T {
        &self.contents
    }

    pub fn contents_mut(&mut self) -> &mut T {
        &mut self.contents
    }
}

impl<T: Component> Component for Popup<T> {
    fn handle_event(&mut self, event: Event, cx: &mut Context) -> EventResult {
        let key = match event {
            Event::Key(event) => event,
            Event::Resize(_, _) => {
                // TODO: calculate inner area, call component's handle_event with that area
                return EventResult::Ignored;
            }
            _ => return EventResult::Ignored,
        };

        let close_fn = EventResult::Consumed(Some(Box::new(|compositor: &mut Compositor, _| {
            // remove the layer
            compositor.pop();
        })));

        match key.into() {
            // esc or ctrl-c aborts the completion and closes the menu
            key!(Esc) | ctrl!('c') => close_fn,
            ctrl!('d') => {
                self.scroll(self.size.1 as usize / 2, true);
                EventResult::Consumed(None)
            }
            ctrl!('u') => {
                self.scroll(self.size.1 as usize / 2, false);
                EventResult::Consumed(None)
            }
            _ => self.contents.handle_event(event, cx),
        }
        // for some events, we want to process them but send ignore, specifically all input except
        // tab/enter/ctrl-k or whatever will confirm the selection/ ctrl-n/ctrl-p for scroll.
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        let max_width = 120.min(viewport.0);
        let max_height = 26.min(viewport.1.saturating_sub(2)); // add some spacing in the viewport

        let inner = Rect::new(0, 0, max_width, max_height).inner(&self.margin);

        let (width, height) = self
            .contents
            .required_size((inner.width, inner.height))
            .expect("Component needs required_size implemented in order to be embedded in a popup");

        self.child_size = (width, height);
        self.size = (
            (width + self.margin.horizontal * 2).min(max_width),
            (height + self.margin.vertical * 2).min(max_height),
        );

        // re-clamp scroll offset
        let max_offset = self.child_size.1.saturating_sub(self.size.1);
        self.scroll = self.scroll.min(max_offset as usize);

        Some(self.size)
    }

    fn render(&mut self, viewport: Rect, surface: &mut Surface, cx: &mut Context) {
        // trigger required_size so we recalculate if the child changed
        self.required_size((viewport.width, viewport.height));

        cx.scroll = Some(self.scroll);

        let (rel_x, rel_y) = self.get_rel_position(viewport, cx);

        // clip to viewport
        let area = viewport.intersection(Rect::new(rel_x, rel_y, self.size.0, self.size.1));

        // clear area
        let background = cx.editor.theme.get("ui.popup");
        surface.clear_with(area, background);

        let inner = area.inner(&self.margin);
        self.contents.render(inner, surface, cx);
    }

    fn id(&self) -> Option<&'static str> {
        Some(self.id)
    }
}
