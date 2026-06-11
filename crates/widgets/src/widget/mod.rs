pub mod animated_visibility;
pub mod container;
pub mod flex_container;
pub mod image;
pub mod text;

use crate::{
    context::{LoadExtent, ManageDirtyFlags, ManageIntrinsic},
    events::{self, EventContext, EventHandling, EventHitTest},
    stage::{
        draw::{Draw, DrawContext, Drawer},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext, InvalidateVisitor, RebuildStatus},
        layout::Layout,
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
    },
    types::{
        Point, WidgetId, WidgetStyle, extent::Extent, identifiers::{WidgetClass, WidgetKey}, offset::Offset
    },
    widget::animated_visibility::AnimatedVisibility,
};

pub use {
    container::{Container, ContainerBuilder, ContainerStyle},
    flex_container::{FlexContainer, FlexContainerBuilder},
    image::{FitMode, Image, ImageBuilder, ImageInfo, ImageStyle, MipmapMode, ResizingMethod},
    text::{Font, FontStyle, Text, TextAlignment, TextBuilder, TextStyle},
};

pub trait WidgetInformation {
    fn get_id(&self) -> WidgetId;

    fn set_id(&mut self, id: WidgetId);

    fn get_key(&self) -> Option<&WidgetKey>;

    fn get_class(&self) -> WidgetClass;
}

pub trait WidgetGetType {
    fn get_type(&self) -> &'static str;
}

pub trait WidgetSizingMode {
    /// Returns the sizing policy of the widget.
    ///
    /// This method is used to distinguish between widgets with pre-defined (fixed) dimensions and those that adapt their size dynamically based on the layout context.
    fn sizing_mode(&self) -> SizingMode;
}

/// A metadata interface for inspecting a widget's identity and dimensions.
///
/// This trait acts as a standardized "Request" system. It allows the
/// layout engine or debugging tools to query essential information
/// from any widget variant without needing to understand that
/// widget's specific internal logic.
pub trait WidgetBase: WidgetInformation + WidgetGetType + WidgetSizingMode {}

impl<W> WidgetBase for W where W: WidgetInformation + WidgetGetType + WidgetSizingMode {}

/// A container enum for all supported widget types.
///
/// This allows dynamic storage and composition of widgets, including complex
/// layouts such as a [`FlexContainer`] holding multiple widgets.
pub enum Widget {
    Image(Box<Image>),
    Text(Box<Text>),
    Container(Box<Container>),
    FlexContainer(Box<FlexContainer>),
    AnimatedVisibility(Box<AnimatedVisibility>),
}

macro_rules! delegate {
    ($self:ident.$method_name:ident($($tokens:tt),*)) => {
        match $self {
            Widget::Image(image) => image.$method_name($($tokens),*),
            Widget::Text(text) => text.$method_name($($tokens),*),
            Widget::Container(container) => container.$method_name($($tokens),*),
            Widget::FlexContainer(flex_container) => flex_container.$method_name($($tokens),*),
            Widget::AnimatedVisibility(animated_visibility) => animated_visibility.$method_name($($tokens),*),
        }
    };
}

impl WidgetInformation for Widget {
    fn get_id(&self) -> WidgetId {
        delegate!(self.get_id())
    }

    fn set_id(&mut self, id: WidgetId) {
        delegate!(self.set_id(id));
    }

    fn get_key(&self) -> Option<&WidgetKey> {
        delegate!(self.get_key())
    }

    fn get_class(&self) -> WidgetClass {
        delegate!(self.get_class())
    }
}

impl WidgetGetType for Widget {
    /// Returns the type of this widget as a human-readable string.
    ///
    /// This is primarily intended for logging and debugging, allowing developers
    /// to inspect which kind of widget is being processed at runtime.
    fn get_type(&self) -> &'static str {
        delegate!(self.get_type())
    }
}

impl WidgetSizingMode for Widget {
    fn sizing_mode(&self) -> SizingMode {
        delegate!(self.sizing_mode())
    }
}

impl<C> Init<C> for Widget
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        delegate!(self.on_init(context));
    }
}

impl<C> Invalidate<C> for Widget
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, context: &mut C, style: WidgetStyle) {
        delegate!(self.on_style_update(context, style));
    }

    fn on_rebuild(&mut self, context: &mut C) -> RebuildStatus {
        delegate!(self.on_rebuild(context))
    }

    fn invalidate_children(&mut self, visitor: &mut impl InvalidateVisitor<C>) {
        delegate!(self.invalidate_children(visitor));
    }
}

impl Measure<f32> for Widget {
    fn intrinsic_content<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        delegate!(self.intrinsic_content(context))
    }

    fn measure_children(&self, visitor: &mut impl measure::MeasureVisitor<f32>) {
        delegate!(self.measure_children(visitor))
    }

    fn measure_content<C>(
        &self,
        context: &mut C,
        constraints: Constraints<Extent<f32>>,
    ) -> Extent<f32>
    where
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        delegate!(self.measure_content(context, constraints))
    }
}

impl<C> Layout<C, f32> for Widget
where
    C: LoadExtent<f32, WidgetId>,
{
    fn layout(&mut self, context: &C) {
        delegate!(self.layout(context))
    }
}

impl<C> Draw<C, f32> for Widget
where
    C: DrawContext<f32>,
{
    fn draw_content(
        &self,
        context: &C,
        offset: &Offset<f32>,
        provided_extent: Extent<f32>,
        output: &mut Drawer,
    ) {
        delegate!(self.draw_content(context, offset, provided_extent, output));
    }
}

impl<C> EventHitTest<f32, C> for Widget
where
    C: EventContext<f32>,
{
    fn on_hit_test(
        &self,
        context: &C,
        local_coords: Point<f32>,
        provided_extent: Extent<f32>,
        router: &mut events::EventRouter,
    ) -> events::HitTestResult {
        delegate!(self.on_hit_test(context, local_coords, provided_extent, router))
    }
}

impl<C> EventHandling<f32, C> for Widget where C: EventContext<f32> {
    fn handle_events(
            &mut self,
            context: &mut C,
            pending_events: Vec<events::PendingEvent>,
            next_child: usize,
            router: &events::EventRouter,
        ) {
        delegate!(self.handle_events(context, pending_events, next_child, router))
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

impl From<AnimatedVisibility> for Widget {
    fn from(value: AnimatedVisibility) -> Self {
        Widget::AnimatedVisibility(value.into())
    }
}

#[macro_export]
macro_rules! make_widget {
    ($name:ident { $($field_name:ident: $val:expr),* $(,)? }) => {
            $name::builder()
                $(.$field_name($val))*
                .build()
    };
}
