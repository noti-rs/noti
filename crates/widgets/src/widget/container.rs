use config::spacing::Spacing;
use log::warn;

use crate::{
    color::{Bgra, Color},
    drawer::Drawer,
    events::{Action, DispatchEvent, Event, Point},
    types::{alignment::Alignment, extent::Extent2D, offset::Offset},
    Compile, CompileState, Draw, Widget, WidgetConfiguration,
};

/// A simple box that stays the same size.
///
/// Unlike the [`FlexContainer`], which stretches and moves to fit many things,
/// the `Container` is a rigid frame for just **one** child widget.
///
/// The Container does three main things:
/// * It sets a fixed width and height that never change.
/// * It holds exactly one child widget inside itself.
/// * It acts as a wall, so the child inside cannot push the box to make it bigger.
///
/// Use this when you need a UI element to stay exactly the same,
/// like a fixed icon or a status light that should never grow or shrink.
#[derive(macros::GenericBuilder, derive_builder::Builder, Clone)]
#[builder(pattern = "owned")]
#[gbuilder(name(GBuilderContainer), derive(Clone))]
pub struct Container {
    #[builder(private, default)]
    #[gbuilder(hidden, default(Bgra::default().into()))]
    background_color: Color,

    #[builder(private, default)]
    #[gbuilder(hidden, default(Bgra::default().into()))]
    border_color: Color,

    #[builder(default = "false")]
    #[gbuilder(default(false))]
    transparent_background: bool,

    #[gbuilder(default)]
    border: config::display::Border,

    #[gbuilder(default)]
    spacing: Spacing,

    alignment: Alignment,

    width: usize,

    height: usize,

    #[gbuilder(default(Box::new(Widget::Unknown)))]
    child: Box<Widget>,
}

impl Container {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

impl Compile for Container {
    fn compile(
        &mut self,
        available_extent: Extent2D<usize>,
        configuration: &WidgetConfiguration,
    ) -> CompileState {
        if self.width > available_extent.width
            || self.height > available_extent.height
            || self.width == 0
            || self.height == 0
        {
            return CompileState::Failure;
        }

        let colors = &configuration
            .theme
            .by_urgency(&configuration.notification.hints.urgency);

        self.background_color = colors.background.clone().into();
        self.border_color = colors.border.clone().into();

        let mut restricted_extent = Extent2D::new(self.width, self.height);
        restricted_extent.shrink_by(&(self.spacing + Spacing::all_directional(self.border.size)));

        self.child.compile(restricted_extent, configuration);

        if self.child.is_unknown() {
            warn!(
                "The container is empty because there is missing child widget or it doesn't fits to available space."
            );
        }

        CompileState::Success
    }
}

impl Draw for Container {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let actual_extent = Extent2D::new(self.width, self.height);

        let transparent_bg = self.transparent_background || self.background_color.is_transparent();
        if !transparent_bg {
            drawer.fill_background(
                (*offset).into(),
                Extent2D::new(self.width, self.height).into(),
                &self.border,
                &self.background_color,
            );
        }

        let inner_spacing = self.spacing + Spacing::all_directional(self.border.size);
        let mut inner_extent = actual_extent;
        inner_extent.shrink_by(&inner_spacing);

        if !self.child.is_unknown() {
            let horizontal_start = self
                .alignment
                .horizontal
                .get_start(inner_extent.width, self.child.width())
                + inner_spacing.left() as usize;
            let vertical_start = self
                .alignment
                .vertical
                .get_start(inner_extent.height, self.child.height())
                + inner_spacing.top() as usize;

            let actual_offset = *offset + Offset::new(horizontal_start, vertical_start);
            self.child.draw_with_offset(&actual_offset, drawer);
        }

        drawer.outline_border(
            (*offset).into(),
            actual_extent.into(),
            &self.border,
            &self.border_color,
        );
    }
}

impl DispatchEvent for Container {
    fn dispatch_event(&self, event: Event) -> Action {
        if self.child.is_unknown() || !event.kind.is_mouse() {
            return Action::None;
        }

        if event.local_coord.x > self.width || event.local_coord.y > self.height {
            return Action::None;
        }

        let inner_spacing = self.spacing + Spacing::all_directional(self.border.size);
        let mut inner_extent = Extent2D::new(self.width, self.height);
        inner_extent.shrink_by(&inner_spacing);

        let child_extent = Extent2D::new(self.child.width(), self.child.height());
        let horizontal_start = self
            .alignment
            .horizontal
            .get_start(inner_extent.width, child_extent.width)
            + inner_spacing.left() as usize;
        let vertical_start = self
            .alignment
            .vertical
            .get_start(inner_extent.height, child_extent.height)
            + inner_spacing.top() as usize;

        if event.local_coord.x < horizontal_start
            || event.local_coord.y < vertical_start
            || event.local_coord.x > horizontal_start + child_extent.width
            || event.local_coord.y > vertical_start + child_extent.height
        {
            return Action::None;
        }

        let mut modified_event = event.clone();
        modified_event.local_coord = Point {
            x: horizontal_start - event.local_coord.x,
            y: vertical_start - event.local_coord.y,
        };

        self.child.dispatch_event(modified_event)
    }
}
