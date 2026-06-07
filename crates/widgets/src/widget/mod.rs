pub mod animated_visibility;
pub mod container;
pub mod flex_container;
pub mod image;
pub mod text;

use crate::{
    context::{
        AnimationQuery, GenerateId, GetDebugOptions, GetFont, GetState, GetStyle, LoadExtent,
        ManageAnimationRegistry, ManageDirtyFlags, ManageIntrinsic, RegisterKey, ScopedManageState,
        StateSubscription, StyleSubscription,
    },
    drawer::Drawer,
    events::{self, DispatchContext, DispatchEvent},
    types::{
        dirty_flags::DirtyFlags,
        extent::Extent,
        identifiers::{WidgetClass, WidgetKey},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
        offset::Offset,
        WidgetId,
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

pub(crate) trait InitContext:
    GenerateId
    + RegisterKey<WidgetKey, WidgetId>
    + GetState
    + StateSubscription<WidgetId>
    + StyleSubscription<WidgetClass, WidgetId>
    + ManageAnimationRegistry<WidgetId>
    + GetStyle
    + GetFont
{
}

impl<C> InitContext for C where
    C: GenerateId
        + RegisterKey<WidgetKey, WidgetId>
        + GetState
        + StateSubscription<WidgetId>
        + StyleSubscription<WidgetClass, WidgetId>
        + ManageAnimationRegistry<WidgetId>
        + GetStyle
        + GetFont
{
}

pub(crate) trait Init<C>: WidgetBase
where
    C: InitContext,
{
    fn init(&mut self, context: &mut C) {
        if *self.get_id() == 0 {
            self.set_id(context.generate_id());
        }

        if let Some(key) = self.get_key() {
            context.register_key(key.clone(), self.get_id());
        }

        self.on_init(context);
    }

    fn on_init(&mut self, context: &mut C);
}

pub(crate) trait LayoutContext<T>: LoadExtent<T, WidgetId>
where
    T: Default + Copy,
{
}

impl<C, T> LayoutContext<T> for C
where
    C: LoadExtent<T, WidgetId>,
    T: Default + Copy,
{
}

pub(crate) trait Layout<C, T>
where
    C: LayoutContext<T>,
    T: Default + Copy,
{
    fn layout(&mut self, context: &C);
}

pub(crate) trait InvalidateContext:
    ManageDirtyFlags<WidgetId>
    + ManageAnimationRegistry<WidgetId>
    + AnimationQuery<WidgetId>
    + GetState
    + GetStyle
    + ScopedManageState
{
}

impl<C> InvalidateContext for C where
    C: ManageDirtyFlags<WidgetId>
        + ManageAnimationRegistry<WidgetId>
        + AnimationQuery<WidgetId>
        + GetState
        + GetStyle
        + ScopedManageState
{
}

pub(crate) trait Invalidate<C>
where
    C: InvalidateContext,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags;
}

pub(crate) trait DrawContext<T>:
    LoadExtent<T, WidgetId> + AnimationQuery<WidgetId> + GetDebugOptions
where
    T: Default + Copy,
{
}

impl<C, T> DrawContext<T> for C
where
    C: LoadExtent<T, WidgetId> + AnimationQuery<WidgetId> + GetDebugOptions,
    T: Default + Copy,
{
}

pub(crate) trait Draw<C, T>: WidgetBase
where
    C: DrawContext<T>,
    T: Default + Copy,
{
    fn draw(&self, context: &C, offset: &Offset<T>, drawer: &mut Drawer) {
        // TODO: use this method as pre-action before actual drawing widget

        self.draw_on(context, offset, drawer);
    }

    fn draw_on(&self, context: &C, offset: &Offset<T>, drawer: &mut Drawer);
}

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

impl<C> Invalidate<C> for Widget
where
    C: InvalidateContext,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        delegate!(self.invalidate(context))
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

impl Measure<f32, WidgetId> for Widget {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
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
    fn draw_on(&self, context: &C, offset: &Offset<f32>, output: &mut Drawer) {
        // INFO: DO NOT USE `draw` METHOD! ONLY `draw_on`
        delegate!(self.draw_on(context, offset, output));
    }
}

impl<C> DispatchEvent<C, f32> for Widget
where
    C: DispatchContext<f32>,
{
    fn dispatch_event(&mut self, context: &mut C, event: events::Event) {
        delegate!(self.dispatch_event(context, event))
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

fn draw_debug_bounds(
    canvas: &skia_safe::Canvas,
    original_offset: Offset<f32>,
    provided_extent: Extent<f32>,
    actual_offset: Offset<f32>,
    actual_extent: Extent<f32>,
) {
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);

    paint.set_color4f(skia_safe::Color4f::new(0.95, 0.23, 0.99, 1.0), None);
    paint.set_style(skia_safe::PaintStyle::Stroke);
    paint.set_stroke_width(2.0);

    if let Some(dash) = skia_safe::PathEffect::dash(&[6.0, 6.0], 0.0) {
        paint.set_path_effect(dash);
    }

    canvas.draw_rect(
        skia_safe::Rect::from_xywh(
            original_offset.x,
            original_offset.y,
            provided_extent.width,
            provided_extent.height,
        ),
        &paint,
    );

    paint.set_color4f(skia_safe::Color4f::new(0.53, 0.97, 0.48, 0.7), None);
    paint.set_path_effect(None);

    canvas.draw_rect(
        skia_safe::Rect::from_xywh(
            actual_offset.x,
            actual_offset.y,
            actual_extent.width,
            actual_extent.height,
        ),
        &paint,
    );
}
