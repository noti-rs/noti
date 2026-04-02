//! The foundational building blocks of the UI layout engine.
//!
//! This module contains the specialized implementations of the `Widget` trait,
//! ranging from basic visual elements like text and images to complex
//! structural managers like flex containers.
//!
//! ### Architecture
//! Each widget in this module follows a consistent lifecycle:
//! 1. **Data Association:** Using a [`WidgetId`], a widget retrieves its
//!    specific data and configuration from a [`CompileCtx`].
//! 2. **Compilation:** The widget validates itself against spatial
//!    constraints and prepares its internal render state (e.g., building
//!    a Skia `Paragraph`).
//! 3. **Drawing:** The widget renders its visual representation onto a
//!    provided Skia surface.
//!
//! ### Core Widget Types
//! The module is organized into three primary categories:
//!
//! * **Visual Elements:**
//!   - [`Text`]: High-performance typography with support for mixed-style entities.
//!   - [`Image`]: Scalable pixel data with aspect-ratio awareness and mipmapping.
//!
//! * **Layout Containers:**
//!   - [`Container`]: A fixed-size wrapper for a single child, providing
//!     borders and backgrounds.
//!   - [`FlexContainer`]: A dynamic, axis-based manager for multiple children
//!     supporting complex alignments (Rows/Columns).
//!
//! * **Structural Helpers:**
//!   - [`Unknown`]: A Zero-Sized Type (ZST) used as a "no-op" placeholder
//!     to simplify macro-driven logic and handle failed compilations gracefully.
//!
//! ### Uniformity via Macros
//! By grouping these types here, the engine can utilize declarative macros
//! to perform uniform operations across all variants of the `Widget` enum
//! without needing to account for the internal complexity of each specific type.

pub mod container;
pub mod flex_container;
pub mod image;
pub mod text;
pub(crate) mod unknown;

use log::warn;

use std::{collections::HashMap, time::Duration};

use shared::value::TryFromValue;

use crate::{
    animation::{
        self, Easing, fade::{FadeIn, FadeOut}, pop::{PopIn, PopOut}, translate::Translate
    },
    context::{GenerateId, GetData, GetFont, GetStyle, LoadExtent},
    drawer::Drawer,
    events::{self, DispatchEvent},
    types::{
        WidgetId, data::{WidgetData, WidgetDependency, WidgetStyle}, direction::Direction, extent::Extent, identifiers::WidgetClass, measure::{self, Constraints, ExtentManagement, IntrinsicManagement, Measure, SizingMode}, offset::Offset
    },
};

pub use {
    container::{
        Container, ContainerBuilder, ContainerBuilderError, ContainerGBuilder, ContainerStyle,
    },
    flex_container::{
        FlexContainer, FlexContainerBuilder, FlexContainerBuilderError, FlexContainerGBuilder,
    },
    image::{
        FitMode, Image, ImageBuilder, ImageBuilderError, ImageGBuilder, ImageInfo, ImageStyle,
        MipmapMode, ResizingMethod,
    },
    text::{
        Font, FontGBuilder, FontStyle, Text, TextAlignment, TextBuilder, TextBuilderError,
        TextGBuilder, TextStyle,
    },
    unknown::Unknown,
};

/// A metadata interface for inspecting a widget's identity and dimensions.
///
/// This trait acts as a standardized "Request" system. It allows the
/// layout engine or debugging tools to query essential information
/// from any widget variant without needing to understand that
/// widget's specific internal logic.
pub trait WidgetInfo {
    fn get_class(&self) -> WidgetClass;

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
    fn width(&self) -> f32;

    /// Returns the current calculated height of the widget.
    ///
    /// This value represents the vertical space the widget occupies
    /// after its last compilation pass.
    fn height(&self) -> f32;

    /// Returns the sizing policy of the widget.
    ///
    /// This method is used to distinguish between widgets with pre-defined (fixed) dimensions and those that adapt their size dynamically based on the layout context.
    fn sizing_mode(&self) -> SizingMode;
}

pub trait Initialize<C>
where
    C: GenerateId + GetData + GetStyle + GetFont,
{
    fn initialize(&mut self, context: &mut C);
}

pub trait Layout<C, T, Id>
where
    C: LoadExtent<T, Id>,
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn layout(&mut self, context: &C);
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
        constraints: Constraints<Extent<f32>>,
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
    fn draw_with_offset(&self, offset: &Offset<f32>, drawer: &mut Drawer);

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
    Image(Box<Image>),
    Text(Box<Text>),
    Container(Box<Container>),
    FlexContainer(Box<FlexContainer>),
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
    pub fn len_by_direction(&self, direction: &Direction) -> f32 {
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
    fn get_class(&self) -> WidgetClass {
        delegate!(self.get_class())
    }

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
    fn width(&self) -> f32 {
        delegate!(self.width())
    }

    /// Returns the fixed, post-compiled height of this widget.
    ///
    /// The height value is determined during widget compilation.
    fn height(&self) -> f32 {
        delegate!(self.height())
    }

    fn sizing_mode(&self) -> SizingMode {
        delegate!(self.sizing_mode())
    }
}

impl<C> Initialize<C> for Widget
where
    C: GenerateId + GetData + GetStyle + GetFont,
{
    fn initialize(&mut self, context: &mut C) {
        delegate!(self.initialize(context));
    }
}

impl Measure<f32, WidgetId> for Widget {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: IntrinsicManagement<f32, WidgetId>,
    {
        delegate!(self.get_intrinsic(context))
    }

    fn measure<C>(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: IntrinsicManagement<f32, WidgetId> + ExtentManagement<f32, WidgetId>,
    {
        delegate!(self.measure(context, constraints))
    }
}

impl<C> Layout<C, f32, WidgetId> for Widget
where
    C: LoadExtent<f32, WidgetId>,
{
    fn layout(&mut self, context: &C) {
        delegate!(self.layout(context))
    }
}

impl Compile for Widget {
    fn compile(
        &mut self,
        constraints: Constraints<Extent<f32>>,
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

impl Draw for Widget {
    fn draw_with_offset(&self, offset: &Offset<f32>, output: &mut Drawer) {
        delegate!(self.draw_with_offset(offset, output));
    }
}

impl DispatchEvent for Widget {
    fn dispatch_event(&self, event: events::Event) -> events::Action {
        delegate!(self.dispatch_event(event))
    }
}

impl TryFromValue for Widget {}

pub enum CompileResult {
    Success { used_extent: Extent<f32> },
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
    pub(crate) data_pool: HashMap<WidgetClass, WidgetDependency>,

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

    pub(crate) fn generate_new_class(&mut self, widget_type: &'static str) -> WidgetClass {
        let counter = self
            .widget_type_counters
            .entry(widget_type.to_string())
            .or_default();

        let class: WidgetClass = format!("{widget_type}#{counter}").into();
        *counter += 1;

        class
    }

    /// Links a specific piece of data struct to a Widget ID.
    ///
    /// These methods are the primary way for the caller to "talk" to a
    /// specific widget from the outside. By adding information to the
    /// `CompileCtx` before compilation begins, you ensure that the
    /// targeted widget can pull the correct values when it needs them.
    pub fn assign_data<Class: Into<WidgetClass>>(&mut self, widget_class: Class, data: WidgetData) {
        self.data_pool
            .entry(widget_class.into())
            .and_modify(|associated_data| associated_data.data = data.clone().into())
            .or_insert_with(|| WidgetDependency {
                data: data.into(),
                style: None,
            });
    }

    /// Links a specific piece of a configuration struct to a Widget ID.
    ///
    /// These methods are the primary way for the caller to "talk" to a
    /// specific widget from the outside. By adding information to the
    /// `CompileCtx` before compilation begins, you ensure that the
    /// targeted widget can pull the correct values when it needs them.
    pub fn assign_config<Class: Into<WidgetClass>>(
        &mut self,
        widget_class: Class,
        config: WidgetStyle,
    ) {
        self.data_pool
            .entry(widget_class.into())
            .and_modify(|associated_data| associated_data.style = config.clone().into())
            .or_insert_with(|| WidgetDependency {
                data: None,
                style: config.into(),
            });
    }
}

impl From<Image> for Widget {
    fn from(value: Image) -> Self {
        Widget::Image(value.into())
    }
}

impl From<Text> for Widget {
    fn from(value: Text) -> Self {
        Widget::Text(value.into())
    }
}

impl From<Container> for Widget {
    fn from(value: Container) -> Self {
        Widget::Container(value.into())
    }
}

impl From<FlexContainer> for Widget {
    fn from(value: FlexContainer) -> Self {
        Widget::FlexContainer(value.into())
    }
}

impl From<Unknown> for Widget {
    fn from(value: Unknown) -> Self {
        Widget::Unknown(value)
    }
}
