use config::{display::DisplayConfig, theme::Theme};
use dbus::notification::Notification;
use log::warn;
use text::PangoContext;

use crate::drawer::Drawer;

use super::types::{Offset, RectSize};

mod flex_container;
mod image;
pub(crate) mod text;

pub use flex_container::{
    Alignment, Direction, FlexContainer, FlexContainerBuilder, GBuilderAlignment,
    GBuilderFlexContainer, Position,
};
pub use image::{GBuilderWImage, WImage};
pub use text::{GBuilderWText, WText, WTextKind};

pub trait Draw {
    fn draw_with_offset(
        &self,
        offset: &Offset<usize>,
        pango_context: &PangoContext,
        drawer: &mut Drawer,
    ) -> pangocairo::cairo::Result<()>;

    fn draw(
        &self,
        pango_context: &PangoContext,
        drawer: &mut Drawer,
    ) -> pangocairo::cairo::Result<()> {
        self.draw_with_offset(&Default::default(), pango_context, drawer)
    }
}

#[derive(Clone)]
pub enum Widget {
    Image(WImage),
    Text(WText),
    FlexContainer(FlexContainer),
    Unknown,
}

impl Widget {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Widget::Unknown)
    }

    fn get_type(&self) -> &'static str {
        match self {
            Widget::Image(_) => "image",
            Widget::Text(_) => "text",
            Widget::FlexContainer(_) => "flex container",
            Widget::Unknown => "unknown",
        }
    }

    pub fn compile(&mut self, rect_size: RectSize<usize>, configuration: &WidgetConfiguration) {
        let state = match self {
            Widget::Image(image) => image.compile(rect_size, configuration),
            Widget::Text(text) => text.compile(rect_size, configuration),
            Widget::FlexContainer(container) => container.compile(rect_size, configuration),
            Widget::Unknown => CompileState::Success,
        };

        if let CompileState::Failure = state {
            warn!(
                "A {wtype} widget is not compiled due errors!",
                wtype = self.get_type()
            );
            *self = Widget::Unknown;
        }
    }

    pub fn len_by_direction(&self, direction: &Direction) -> usize {
        match direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    pub fn width(&self) -> usize {
        match self {
            Widget::Image(image) => image.width(),
            Widget::Text(text) => text.width(),
            Widget::FlexContainer(container) => container.max_width(),
            Widget::Unknown => 0,
        }
    }

    pub fn height(&self) -> usize {
        match self {
            Widget::Image(image) => image.height(),
            Widget::Text(text) => text.height(),
            Widget::FlexContainer(container) => container.max_height(),
            Widget::Unknown => 0,
        }
    }
}

impl Draw for Widget {
    fn draw_with_offset(
        &self,
        offset: &Offset<usize>,
        pango_context: &PangoContext,
        output: &mut Drawer,
    ) -> pangocairo::cairo::Result<()> {
        match self {
            Widget::Image(image) => image.draw_with_offset(offset, pango_context, output),
            Widget::Text(text) => text.draw_with_offset(offset, pango_context, output),
            Widget::FlexContainer(container) => {
                container.draw_with_offset(offset, pango_context, output)
            }
            Widget::Unknown => Ok(()),
        }
    }
}

pub enum CompileState {
    Success,
    Failure,
}

pub struct WidgetConfiguration<'a> {
    pub notification: &'a Notification,
    pub pango_context: &'a PangoContext,
    pub theme: &'a Theme,
    pub display_config: &'a DisplayConfig,
    pub override_properties: bool,
}

impl From<WImage> for Widget {
    fn from(value: WImage) -> Self {
        Widget::Image(value)
    }
}

impl From<WText> for Widget {
    fn from(value: WText) -> Self {
        Widget::Text(value)
    }
}

impl From<FlexContainer> for Widget {
    fn from(value: FlexContainer) -> Self {
        Widget::FlexContainer(value)
    }
}
