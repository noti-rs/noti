pub mod container;
pub mod flex_container;
pub mod image;
pub mod text;

use shared::value::TryFromValue;

use crate::{
    context::{
        GenerateId, GetFont, GetState, GetStyle, LoadExtent, ManageDirtyFlags, ManageIntrinsic,
        RegisterKey, Subscribe,
    },
    drawer::Drawer,
    events::{self, DispatchEvent},
    types::{
        dirty_flags::DirtyFlags,
        extent::Extent,
        identifiers::{WidgetClass, WidgetKey},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
        offset::Offset,
        WidgetId,
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
};

/// A metadata interface for inspecting a widget's identity and dimensions.
///
/// This trait acts as a standardized "Request" system. It allows the
/// layout engine or debugging tools to query essential information
/// from any widget variant without needing to understand that
/// widget's specific internal logic.
pub trait WidgetInfo {
    fn get_id(&self) -> WidgetId;

    fn get_class(&self) -> WidgetClass;

    fn get_type(&self) -> &'static str;

    /// Returns the sizing policy of the widget.
    ///
    /// This method is used to distinguish between widgets with pre-defined (fixed) dimensions and those that adapt their size dynamically based on the layout context.
    fn sizing_mode(&self) -> SizingMode;
}

pub(crate) trait Invalidate<C>
where
    C: ManageDirtyFlags<WidgetId> + GetState,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags;
}

pub(crate) trait Initialize<C>
where
    C: GenerateId
        + RegisterKey<WidgetKey, WidgetId>
        + GetState
        + Subscribe<WidgetId>
        + GetStyle
        + GetFont,
{
    fn initialize(&mut self, context: &mut C);
}

pub(crate) trait Layout<C, T, Id>
where
    C: LoadExtent<T, Id>,
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn layout(&mut self, context: &C);
}

pub(crate) trait Draw<C, T>
where
    C: LoadExtent<T, WidgetId>,
    T: Default + Copy,
    Self: WidgetInfo,
{
    fn draw(&self, context: &C, offset: &Offset<T>, drawer: &mut Drawer) {
        self.draw_on(context, offset, drawer);
        // let Some(presence_state) = <C as LoadPresenceState<WidgetId>>::load(context, self.get_id())
        // else {
        //     self.draw_on(context, offset, drawer);
        //     return;
        // };
        //
        // match presence_state.phase {
        //     PresencePhase::Allocating | PresencePhase::Releasing => return,
        //     PresencePhase::Appearing | PresencePhase::Showing => todo!(),
        //     PresencePhase::Disappearing => todo!(),
        // }
    }

    fn draw_on(&self, context: &C, offset: &Offset<T>, drawer: &mut Drawer);
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
}

macro_rules! delegate {
    ($self:ident.$method_name:ident($($tokens:tt),*)) => {
        match $self {
            Widget::Image(image) => image.$method_name($($tokens),*),
            Widget::Text(text) => text.$method_name($($tokens),*),
            Widget::Container(container) => container.$method_name($($tokens),*),
            Widget::FlexContainer(flex_container) => flex_container.$method_name($($tokens),*),
        }
    };
}

impl WidgetInfo for Widget {
    fn get_id(&self) -> WidgetId {
        delegate!(self.get_id())
    }

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

    fn sizing_mode(&self) -> SizingMode {
        delegate!(self.sizing_mode())
    }
}

impl<C> Invalidate<C> for Widget
where
    C: ManageDirtyFlags<WidgetId> + GetState,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        delegate!(self.invalidate(context))
    }
}

impl<C> Initialize<C> for Widget
where
    C: GenerateId
        + RegisterKey<WidgetKey, WidgetId>
        + GetState
        + Subscribe<WidgetId>
        + GetStyle
        + GetFont,
{
    fn initialize(&mut self, context: &mut C) {
        delegate!(self.initialize(context));
    }
}

impl Measure<f32, WidgetId> for Widget {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId>,
    {
        delegate!(self.get_intrinsic(context))
    }

    fn measure<C>(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
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

impl<C> Draw<C, f32> for Widget
where
    C: LoadExtent<f32, WidgetId>,
{
    fn draw_on(&self, context: &C, offset: &Offset<f32>, output: &mut Drawer) {
        // INFO: DO NOT USE `draw` METHOD! ONLY `draw_on`
        delegate!(self.draw_on(context, offset, output));
    }
}

impl<C> DispatchEvent<C, f32> for Widget
where
    C: LoadExtent<f32, WidgetId>,
{
    fn dispatch_event(&self, context: &C, event: events::Event) -> events::Action {
        delegate!(self.dispatch_event(context, event))
    }
}

impl TryFromValue for Widget {}

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
