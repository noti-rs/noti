pub mod animation;
pub mod color;
pub mod drawer;
pub mod events;
pub mod image;
pub mod types;
pub mod widget;

use std::time::Duration;

use config::{display::DisplayConfig, theme::Theme};
use dbus::notification::Notification;
use log::warn;
use shared::value::TryFromValue;

use crate::{
    animation::{
        Easing, fade::{FadeIn, FadeOut}, pop::{PopIn, PopOut}, translate::Translate
    }, drawer::Drawer, events::{Action, DispatchEvent}, types::direction::Direction, widget::container::Container
};

use types::{extent::Extent2D, offset::Offset};

use widget::{flex_container::FlexContainer, image::WImage, text::WText};

/// A minimal trait for drawing any widget onto a [`Drawer`].
///
/// This trait abstracts over the rendering process, allowing each widget to
/// define its own drawing logic. The backend uses EGL and a Skia surface, so
/// drawing always happens relative to a top-left origin.
///
/// The [`draw_with_offset`] method is the core API and should be used whenever
/// a widget must be rendered at a specific position. The [`draw`] method is a
/// convenience wrapper that draws the widget at the origin `(0, 0)`.
pub trait Draw {
    /// Draws the widget using the given [`Drawer`], applying an explicit offset
    /// from the top-left origin.
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer);

    /// Draws the widget at the origin `(0, 0)`.
    ///
    /// This is a convenience method for `draw_with_offset` with no displacement.
    fn draw(&self, drawer: &mut Drawer) {
        self.draw_with_offset(&Default::default(), drawer)
    }
}

/// A container enum for all supported widget types.
///
/// This allows dynamic storage and composition of widgets, including complex
/// layouts such as a [`FlexContainer`] holding multiple widgets.
#[derive(Clone)]
pub enum Widget {
    Image(WImage),
    Text(WText),
    Container(Container),
    FlexContainer(FlexContainer),
    /// Placeholder for unsupported or unrecognized widgets; safely ignored during drawing.
    Unknown,
}

impl Widget {
    /// Check whether the widget is [`Widget::Unknown`].
    pub fn is_unknown(&self) -> bool {
        matches!(self, Widget::Unknown)
    }

    /// Returns the type of this widget as a human-readable string.
    ///
    /// This is primarily intended for logging and debugging, allowing developers
    /// to inspect which kind of widget is being processed at runtime.
    fn get_type(&self) -> &'static str {
        match self {
            Widget::Image(_) => "image",
            Widget::Text(_) => "text",
            Widget::Container(_) => "container",
            Widget::FlexContainer(_) => "flex container",
            Widget::Unknown => "unknown",
        }
    }

    /// Returns the size of this widget along the specified [`Direction`].
    ///
    /// If the direction is [`Direction::Horizontal`], this method is equivalent
    /// to calling [`Self::width`].  
    /// If the direction is [`Direction::Vertical`], it is equivalent
    /// to calling [`Self::height`].
    pub fn len_by_direction(&self, direction: &Direction) -> usize {
        match direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    /// Returns the fixed, post-compiled width of this widget.
    ///
    /// The width value is determined during widget compilation.
    pub fn width(&self) -> usize {
        match self {
            Widget::Image(image) => image.width(),
            Widget::Text(text) => text.width(),
            Widget::Container(container) => container.width(),
            Widget::FlexContainer(flex_container) => flex_container.max_width(),
            Widget::Unknown => 0,
        }
    }

    /// Returns the fixed, post-compiled height of this widget.
    ///
    /// The height value is determined during widget compilation.
    pub fn height(&self) -> usize {
        match self {
            Widget::Image(image) => image.height(),
            Widget::Text(text) => text.height(),
            Widget::Container(container) => container.height(),
            Widget::FlexContainer(flex_container) => flex_container.max_height(),
            Widget::Unknown => 0,
        }
    }

    pub fn fade_in<E: Into<Easing>>(self, duration: Duration, easing: E) -> FadeIn {
        FadeIn::new(self, duration, easing.into())
    }

    pub fn fade_out<E: Into<Easing>>(self, duration: Duration, easing: E) -> FadeOut {
        FadeOut::new(self, duration, easing.into())
    }

    pub fn pop_in<E: Into<Easing>>(self, duration: Duration, easing: E) -> PopIn {
        PopIn::new(self, duration, easing.into())
    }

    pub fn pop_out<E: Into<Easing>>(self, duration: Duration, easing: E) -> PopOut {
        PopOut::new(self, duration, easing.into())
    }

    pub fn translate<E: Into<Easing>>(
        self,
        start: animation::translate::Point,
        end: animation::translate::Point,
        duration: Duration,
        easing: E,
    ) -> Translate {
        Translate::new(self, start, end, duration, easing.into())
    }

    pub fn slide_horizontally<E: Into<Easing>>(
        self,
        start_x: f32,
        end_x: f32,
        duration: Duration,
        easing: E,
    ) -> Translate {
        Translate::new(
            self,
            animation::translate::Point::new(start_x, 0.0),
            animation::translate::Point::new(end_x, 0.0),
            duration,
            easing.into(),
        )
    }

    pub fn slide_vertically<E: Into<Easing>>(
        self,
        start_y: f32,
        end_y: f32,
        duration: Duration,
        easing: E,
    ) -> Translate {
        Translate::new(
            self,
            animation::translate::Point::new(0.0, start_y),
            animation::translate::Point::new(0.0, end_y),
            duration,
            easing.into(),
        )
    }
}

impl Draw for Widget {
    fn draw_with_offset(&self, offset: &Offset<usize>, output: &mut Drawer) {
        match self {
            Widget::Image(image) => image.draw_with_offset(offset, output),
            Widget::Text(text) => text.draw_with_offset(offset, output),
            Widget::Container(container) => container.draw_with_offset(offset, output),
            Widget::FlexContainer(flex_container) => {
                flex_container.draw_with_offset(offset, output)
            }
            Widget::Unknown => (),
        }
    }
}

impl DispatchEvent for Widget {
    fn dispatch_event(&self, event: events::Event) -> events::Action {
        match self {
            Widget::Image(wimage) => wimage.dispatch_event(event),
            Widget::Text(wtext) => wtext.dispatch_event(event),
            Widget::Container(container) => container.dispatch_event(event),
            Widget::FlexContainer(flex_container) => flex_container.dispatch_event(event),
            Widget::Unknown => Action::None,
        }
    }
}

impl TryFromValue for Widget {}

/// Compiles this widget and computes its final layout properties.
///
/// During compilation, this widget calculates its post-compilation width,
/// height, spacing, margins, fill behavior, and other layout properties.
/// The provided [`Extent2D`] defines the available drawing space for this
/// widget, while the [`WidgetConfiguration`] provides shared context such as
/// theme, fonts, and notification data.
///
/// # Why This Matters
/// Widgets cannot know their final size or layout until compilation is
/// performed. Call this method before querying [`width`], [`height`],
/// or before drawing, to ensure the widget is placed correctly on screen.
///
/// # Parameters
/// - `available_extent` – The size of the available space in which this widget may fit.
/// - `configuration` – Shared context and configuration data for the compilation.
pub trait Compile {
    fn compile(
        &mut self,
        available_extent: Extent2D<usize>,
        configuration: &WidgetConfiguration,
    ) -> CompileState;
}

impl Compile for Widget {
    fn compile(
        &mut self,
        available_extent: Extent2D<usize>,
        configuration: &WidgetConfiguration,
    ) -> CompileState {
        let state = match self {
            Widget::Image(image) => image.compile(available_extent, configuration),
            Widget::Text(text) => text.compile(available_extent, configuration),
            Widget::Container(container) => container.compile(available_extent, configuration),
            Widget::FlexContainer(flex_container) => {
                flex_container.compile(available_extent, configuration)
            }
            Widget::Unknown => CompileState::Success,
        };

        if let CompileState::Failure = &state {
            warn!(
                "A {wtype} widget is not compiled due errors!",
                wtype = self.get_type()
            );
            *self = Widget::Unknown;
        }

        state
    }
}

pub enum CompileState {
    Success,
    Failure,
}

/// Provides shared context and resources for widget compilation.
///
/// This struct bundles together data such as the current notification,
/// theme, font collection, and display configuration, so widgets can
/// compute their layout in a consistent way.
///
/// Used as input to [`Widget::compile`] and other widget compilation
/// methods to ensure all layout decisions respect the same configuration.
pub struct WidgetConfiguration<'a> {
    pub notification: &'a Notification,
    pub theme: &'a Theme,
    pub font_collection: skia_safe::textlayout::FontCollection,
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

impl From<Container> for Widget {
    fn from(value: Container) -> Self {
        Widget::Container(value)
    }
}

impl From<FlexContainer> for Widget {
    fn from(value: FlexContainer) -> Self {
        Widget::FlexContainer(value)
    }
}
