pub mod animated_visibility;
pub mod r#box;
pub mod flexbox;
pub mod image;
pub mod text;
pub mod button;

use crate::{
    context::{LoadExtent, ManageDirtyFlags, ManageIntrinsic, ManageWidgetData, WidgetTreeAccess},
    events::{self, EventContext, EventHandling, EventHitTest},
    forest::{Get, GetCarefully},
    stage::{
        deinit::{Deinit, DeinitContext},
        draw::{Draw, DrawContext, Drawer},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext, RebuildStatus},
        layout::{Layout, LayoutContext},
        measure::{self, Constraints, ManageMeasures, Measure, MeasureContext, SizingMode},
        rebuild::WidgetDiff,
    },
    types::{
        extent::Extent,
        identifiers::{WidgetClass, WidgetKey},
        offset::Offset,
        Point, WidgetId, WidgetStyle,
    },
    widget::animated_visibility::AnimatedVisibility,
};

pub use {
    flexbox::{FlexBox, FlexBoxBuilder},
    image::{FitMode, Image, ImageBuilder, ImageInfo, ImageStyle, MipmapMode, ResizingMethod},
    r#box::{Box, BoxBuilder},
    text::{Font, FontStyle, Text, TextAlignment, TextBuilder, TextStyle},
};

pub trait WidgetInformation {
    fn get_id(&self) -> WidgetId;

    fn set_id(&mut self, id: WidgetId);

    fn get_key(&self) -> Option<&WidgetKey>;

    fn get_class(&self) -> WidgetClass;
}

impl<T: WidgetInformation + ?Sized> Get<WidgetId> for &T {
    fn get(&self) -> WidgetId {
        self.get_id()
    }
}

impl<C> Get<WidgetId> for std::boxed::Box<dyn Widget<C>>
where
    C: WidgetContext,
{
    fn get(&self) -> WidgetId {
        self.get_id()
    }
}

impl<T: WidgetInformation> GetCarefully<WidgetKey> for T {
    fn get_carefully(&self) -> Option<&WidgetKey> {
        self.get_key()
    }
}

impl<C> GetCarefully<WidgetKey> for std::boxed::Box<dyn Widget<C>>
where
    C: WidgetContext,
{
    fn get_carefully(&self) -> Option<&WidgetKey> {
        self.get_key()
    }
}

pub trait WidgetGetType {
    /// Returns the type of this widget as a human-readable string.
    ///
    /// This is primarily intended for logging and debugging, allowing developers
    /// to inspect which kind of widget is being processed at runtime.
    fn get_type(&self) -> &'static str;
}

impl<C> WidgetGetType for std::boxed::Box<dyn Widget<C>>
where
    C: WidgetContext,
{
    fn get_type(&self) -> &'static str {
        (**self).get_type()
    }
}

pub trait WidgetInformationContext:
    WidgetTreeAccess<WidgetId> + ManageWidgetData<WidgetId>
{
    fn widget_sizing_mode(&self, widget_id: &WidgetId) -> Option<SizingMode>;
}

pub trait WidgetSizingMode<C>
where
    C: WidgetInformationContext,
{
    /// Returns the sizing policy of the widget.
    ///
    /// This method is used to distinguish between widgets with pre-defined (fixed) dimensions and those that adapt their size dynamically based on the layout context.
    fn sizing_mode(&self, context: &C) -> SizingMode;
}

/// A metadata interface for inspecting a widget's identity and dimensions.
///
/// This trait acts as a standardized "Request" system. It allows the
/// layout engine or debugging tools to query essential information
/// from any widget variant without needing to understand that
/// widget's specific internal logic.
pub trait WidgetBase<C>: WidgetInformation + WidgetGetType + WidgetSizingMode<C>
where
    C: WidgetInformationContext,
{
}

impl<W, C> WidgetBase<C> for W
where
    W: WidgetInformation + WidgetGetType + WidgetSizingMode<C>,
    C: WidgetInformationContext,
{
}

pub trait WidgetContext:
    WidgetInformationContext
    + InitContext
    + DeinitContext
    + InvalidateContext
    + MeasureContext<f32>
    + LoadExtent<f32, WidgetId>
    + DrawContext<f32>
    + EventContext<f32>
{
}

impl<C> WidgetContext for C where
    C: WidgetInformationContext
        + InitContext
        + DeinitContext
        + InvalidateContext
        + MeasureContext<f32>
        + LoadExtent<f32, WidgetId>
        + DrawContext<f32>
        + EventContext<f32>
{
}

pub trait Widget<C>:
    WidgetBase<C>
    + WidgetDiff
    + Init<C>
    + Deinit<C>
    + Invalidate<C>
    + Measure<C, f32>
    + Layout<C, f32>
    + Draw<C, f32>
    + EventHitTest<C, f32>
    + EventHandling<C, f32>
where
    C: WidgetContext,
{
}

impl<W, C> Widget<C> for W
where
    W: WidgetBase<C>
        + WidgetDiff
        + Init<C>
        + Deinit<C>
        + Invalidate<C>
        + Measure<C, f32>
        + Layout<C, f32>
        + Draw<C, f32>
        + EventHitTest<C, f32>
        + EventHandling<C, f32>,
    C: WidgetContext,
{
}

/// A container enum for all supported widget types.
///
/// This allows dynamic storage and composition of widgets, including complex
/// layouts such as a [`FlexContainer`] holding multiple widgets.
pub enum WidgetEnum {
    Image(std::boxed::Box<Image>),
    Text(std::boxed::Box<Text>),
    Container(std::boxed::Box<Box>),
    FlexContainer(std::boxed::Box<FlexBox>),
    AnimatedVisibility(std::boxed::Box<AnimatedVisibility>),
}

macro_rules! delegate {
    ($self:ident.$method_name:ident($($tokens:tt),*)) => {
        match $self {
            WidgetEnum::Image(image) => image.$method_name($($tokens),*),
            WidgetEnum::Text(text) => text.$method_name($($tokens),*),
            WidgetEnum::Container(container) => container.$method_name($($tokens),*),
            WidgetEnum::FlexContainer(flex_container) => flex_container.$method_name($($tokens),*),
            WidgetEnum::AnimatedVisibility(animated_visibility) => animated_visibility.$method_name($($tokens),*),
        }
    };
}

impl WidgetInformation for WidgetEnum {
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

impl WidgetGetType for WidgetEnum {
    /// Returns the type of this widget as a human-readable string.
    ///
    /// This is primarily intended for logging and debugging, allowing developers
    /// to inspect which kind of widget is being processed at runtime.
    fn get_type(&self) -> &'static str {
        delegate!(self.get_type())
    }
}

impl<C> WidgetSizingMode<C> for WidgetEnum
where
    C: WidgetInformationContext,
{
    fn sizing_mode(&self, context: &C) -> SizingMode {
        delegate!(self.sizing_mode(context))
    }
}

impl<C> Init<C> for WidgetEnum
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        delegate!(self.on_init(context));
    }
}

impl<C> Invalidate<C> for WidgetEnum
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, context: &mut C, style: WidgetStyle) {
        delegate!(self.on_style_update(context, style));
    }

    fn on_rebuild(&mut self, context: &mut C) -> RebuildStatus {
        delegate!(self.on_rebuild(context))
    }
}

impl<C> Measure<C, f32> for WidgetEnum
where
    C: MeasureContext<f32>,
{
    fn intrinsic_content(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        delegate!(self.intrinsic_content(context))
    }

    fn measure_content(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: ManageMeasures<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        delegate!(self.measure_content(context, constraints))
    }
}

impl<C> Layout<C, f32> for WidgetEnum
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &mut C) {
        delegate!(self.layout(context))
    }
}

impl<C> Draw<C, f32> for WidgetEnum
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

impl<C> EventHitTest<C, f32> for WidgetEnum
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

impl<C> EventHandling<C, f32> for WidgetEnum
where
    C: EventContext<f32>,
{
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

impl From<Image> for WidgetEnum {
    fn from(value: Image) -> Self {
        WidgetEnum::Image(value.into())
    }
}

impl From<Text> for WidgetEnum {
    fn from(value: Text) -> Self {
        WidgetEnum::Text(value.into())
    }
}

impl From<Box> for WidgetEnum {
    fn from(value: Box) -> Self {
        WidgetEnum::Container(value.into())
    }
}

impl From<FlexBox> for WidgetEnum {
    fn from(value: FlexBox) -> Self {
        WidgetEnum::FlexContainer(value.into())
    }
}

impl From<AnimatedVisibility> for WidgetEnum {
    fn from(value: AnimatedVisibility) -> Self {
        WidgetEnum::AnimatedVisibility(value.into())
    }
}

#[macro_export]
macro_rules! make_widget {
    ($context:ident <== $name:ident ( $($field_name:ident: $val:expr),* $(,)? ) { $($child_node_id:expr),*$(,)? }) => {{
            let node_id = $context.create_widget(Box::new($name::builder()
                $(.$field_name($val))*
                .build()));

            $(
                {
                    let child_node_id = $child_node_id;
                    $context.append_child(node_id, child_node_id);
                }
            )*

            node_id
    }};

    ($context:ident <== $name:ident ( $($field_name:ident: $val:expr),* $(,)? )) => {
            $context.create_widget(Box::new($name::builder()
                $(.$field_name($val))*
                .build()))
    };
}
