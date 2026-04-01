pub mod animation;
pub mod drawer;
pub mod events;
pub mod image;
pub mod types;
pub mod widget;

use std::{collections::HashMap, time::Duration};

use log::warn;
use shared::value::TryFromValue;

use crate::{
    animation::{
        fade::{FadeIn, FadeOut},
        pop::{PopIn, PopOut},
        translate::Translate,
        Easing,
    },
    drawer::Drawer,
    events::DispatchEvent,
    types::{
        constraints::{Constraints, SizingMode},
        data::{AssociatedData, WidgetConfig, WidgetData},
        direction::Direction,
        widget_id::WidgetId,
    },
    widget::{Container, FlexContainer, Image, Text, Unknown},
};

use types::{extent::Extent2D, offset::Offset};

/// A metadata interface for inspecting a widget's identity and dimensions.
///
/// This trait acts as a standardized "Request" system. It allows the
/// layout engine or debugging tools to query essential information
/// from any widget variant without needing to understand that
/// widget's specific internal logic.
pub trait WidgetInfo {
    /// Returns the static, pre-defined category of the widget.
    ///
    /// This string (e.g., "text", "flex_container") is used to differentiate
    /// between widget classes. It serves as the foundation for the
    /// automatic ID generation system within the [`CompileCtx`],
    /// ensuring that even unnamed widgets can be tracked and debugged.
    fn get_type(&self) -> &'static str;

    /// Returns the current calculated width of the widget.
    ///
    /// This value represents the horizontal space the widget occupies
    /// after its last compilation pass.
    fn width(&self) -> usize;

    /// Returns the current calculated height of the widget.
    ///
    /// This value represents the vertical space the widget occupies
    /// after its last compilation pass.
    fn height(&self) -> usize;

    /// Returns the sizing policy of the widget.
    ///
    /// This method is used to distinguish between widgets with pre-defined (fixed) dimensions and those that adapt their size dynamically based on the layout context.
    fn sizing_mode(&self) -> SizingMode;
}

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
        constraints: Constraints<f32>,
        compile_ctx: &mut CompileCtx,
    ) -> CompileResult;
}

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
    Image(Image),
    Text(Text),
    Container(Container),
    FlexContainer(FlexContainer),
    Unknown(Unknown),
}

macro_rules! delegate {
    ($self:ident.$method_name:ident($($tokens:tt),*)) => {
        match $self {
            Widget::Image(image) => image.$method_name($($tokens),*),
            Widget::Text(text) => text.$method_name($($tokens),*),
            Widget::Container(container) => container.$method_name($($tokens),*),
            Widget::FlexContainer(flex_container) => flex_container.$method_name($($tokens),*),
            Widget::Unknown(unknown) => unknown.$method_name($($tokens),*),
        }
    };
}

impl Widget {
    /// Check whether the widget is [`Widget::Unknown`].
    pub fn is_unknown(&self) -> bool {
        matches!(self, Widget::Unknown(_))
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

impl WidgetInfo for Widget {
    /// Returns the type of this widget as a human-readable string.
    ///
    /// This is primarily intended for logging and debugging, allowing developers
    /// to inspect which kind of widget is being processed at runtime.
    fn get_type(&self) -> &'static str {
        delegate!(self.get_type())
    }

    /// Returns the fixed, post-compiled width of this widget.
    ///
    /// The width value is determined during widget compilation.
    fn width(&self) -> usize {
        delegate!(self.width())
    }

    /// Returns the fixed, post-compiled height of this widget.
    ///
    /// The height value is determined during widget compilation.
    fn height(&self) -> usize {
        delegate!(self.height())
    }

    fn sizing_mode(&self) -> SizingMode {
        delegate!(self.sizing_mode())
    }
}

impl Draw for Widget {
    fn draw_with_offset(&self, offset: &Offset<usize>, output: &mut Drawer) {
        delegate!(self.draw_with_offset(offset, output));
    }
}

impl DispatchEvent for Widget {
    fn dispatch_event(&self, event: events::Event) -> events::Action {
        delegate!(self.dispatch_event(event))
    }
}

impl TryFromValue for Widget {}

impl Compile for Widget {
    fn compile(
        &mut self,
        constraints: Constraints<f32>,
        compile_ctx: &mut CompileCtx,
    ) -> CompileResult {
        let state = delegate!(self.compile(constraints, compile_ctx));

        if let CompileResult::Failure = &state {
            warn!(
                "A {wtype} widget is not compiled due errors!",
                wtype = self.get_type()
            );
            *self = Widget::Unknown(Unknown);
        }

        state
    }
}

pub enum CompileResult {
    Success { used_extent: Extent2D<f32> },
    Failure,
}

/// The shared context provided to every widget during the compilation phase.
///
/// Think of this as a "Resource & Identity" packet. It carries all the
/// information that isn't part of the widget's local state, such as
/// shared font resources, externally associated data, and the tools
/// needed to generate unique IDs for every element in the layout.
pub struct CompileCtx {
    /// A lookup table for "Out-of-Band" widget information.
    ///
    /// This stores `AssociatedData` (pairs of WidgetData and WidgetConfig)
    /// indexed by `WidgetId`. It allows the layout engine to inject
    /// specific values into a widget during compilation without needing
    /// to modify the widget tree directly.
    pub(crate) data_pool: HashMap<WidgetId, AssociatedData>,

    /// The global registry of fonts used to calculate text shapes and sizes.
    pub font_collection: skia_safe::textlayout::FontCollection,

    /// A tracking system for automatic ID generation.
    ///
    /// This keeps count of how many widgets of each type (e.g., "Text", "Image")
    /// have been processed. If a widget doesn't have a manual ID, this is
    /// used to generate a readable, numbered name (like "text#1", "text#2"),
    /// which is vital for debugging and layout traceability.
    pub(crate) widget_type_counters: HashMap<String, usize>,
}

impl CompileCtx {
    pub fn new(font_collection: skia_safe::textlayout::FontCollection) -> Self {
        Self {
            data_pool: HashMap::new(),
            widget_type_counters: HashMap::new(),
            font_collection,
        }
    }

    pub(crate) fn generate_new_id(&mut self, widget_type: &'static str) -> WidgetId {
        let counter = self
            .widget_type_counters
            .entry(widget_type.to_string())
            .or_default();

        let id: WidgetId = format!("{widget_type}#{counter}").into();
        *counter += 1;

        id
    }

    /// Links a specific piece of data struct to a Widget ID.
    ///
    /// These methods are the primary way for the caller to "talk" to a
    /// specific widget from the outside. By adding information to the
    /// `CompileCtx` before compilation begins, you ensure that the
    /// targeted widget can pull the correct values when it needs them.
    pub fn assign_data<Id: Into<WidgetId>>(&mut self, widget_id: Id, data: WidgetData) {
        self.data_pool
            .entry(widget_id.into())
            .and_modify(|associated_data| associated_data.data = data.clone().into())
            .or_insert_with(|| AssociatedData {
                data: data.into(),
                config: None,
            });
    }

    /// Links a specific piece of a configuration struct to a Widget ID.
    ///
    /// These methods are the primary way for the caller to "talk" to a
    /// specific widget from the outside. By adding information to the
    /// `CompileCtx` before compilation begins, you ensure that the
    /// targeted widget can pull the correct values when it needs them.
    pub fn assign_config<Id: Into<WidgetId>>(&mut self, widget_id: Id, config: WidgetConfig) {
        self.data_pool
            .entry(widget_id.into())
            .and_modify(|associated_data| associated_data.config = config.clone().into())
            .or_insert_with(|| AssociatedData {
                data: None,
                config: config.into(),
            });
    }
}

impl From<Image> for Widget {
    fn from(value: Image) -> Self {
        Widget::Image(value)
    }
}

impl From<Text> for Widget {
    fn from(value: Text) -> Self {
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
