use log::warn;
use macros::widget_style;

use crate::{
    context::{LoadExtent, ManageDirtyFlags, ManageIntrinsic},
    decorator::{
        content::Content, DecoratorExt, DrawDecorator, EventHitTestDecorator, MeasureDecorator,
    },
    events::{
        EventContext, EventHandling, EventHitTest, EventRouter, HitTestResult,
        PendingEvent,
    },
    stage::{
        draw::{draw_debug_bounds, Draw, DrawContext, Drawer},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext, InvalidateVisitor, RebuildStatus},
        layout::{Layout, LayoutContext},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
    },
    types::{
        alignment::Alignment,
        border::Border,
        extent::Extent,
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        offset::Offset,
        spacing::Spacing,
        style::{Configure, StyleProperty, WidgetStyle},
        Color, Point,
    },
    widget::{
        flex_container::FlexContainer, Widget, WidgetGetType, WidgetInformation, WidgetSizingMode,
    },
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
#[derive(bon::Builder)]
pub struct Container {
    /// An optional identifier for this widget.
    ///
    /// If left empty, an ID will be automatically generated during
    /// compilation. Setting this manually allows the widget to be
    /// targeted by external configurations and makes the widget tree
    /// significantly easier to navigate during debugging.
    #[builder(skip)]
    id: WidgetId,

    #[builder(into)]
    key: Option<WidgetKey>,

    #[builder(into, default)]
    class: WidgetClass,

    /// The fill color or gradient applied to the entire area of the container.
    ///
    /// This defines the visual surface that sits behind any nested child
    /// widgets. It covers the full rectangular area of the container,
    /// providing a solid or decorative base. If not set, the container
    /// is typically transparent, allowing the parent's background to
    /// show through.
    #[builder(with = |v: Color| StyleProperty::Explicit(v), default)]
    background_color: StyleProperty<Color>,

    /// The visual frame and corner shaping applied to the container's edges.
    ///
    /// This field defines the stroke thickness, color, and curvature of
    /// the widget's boundary. It provides a clear visual distinction
    /// between the container's internal content and the rest of the
    /// layout.
    #[builder(with = |v: Border| StyleProperty::Explicit(v), default)]
    border: StyleProperty<Border>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(with = |v: Spacing| StyleProperty::Explicit(v), default)]
    spacing: StyleProperty<Spacing>,

    /// The rules for positioning content within the available internal space.
    ///
    /// This determines how the content (like text or nested widgets)
    /// anchors itself when the container is larger than the content
    /// it holds. It manages the distribution of "extra" space along
    /// the horizontal and vertical axes.
    #[builder(with = |v: Alignment| StyleProperty::Explicit(v), default)]
    alignment: StyleProperty<Alignment>,

    /// A hard-coded, fixed dimension for this axis.
    ///
    /// When set, the container will occupy exactly this many units regardless
    /// of its content's size or the parent's constraints. This effectively
    /// "locks" the widget's size, preventing it from expanding or
    /// shrinking during the layout pass.
    #[builder(with = |v: usize| StyleProperty::Explicit(v), default)]
    width: StyleProperty<usize>,

    /// A hard-coded, fixed dimension for this axis.
    ///
    /// When set, the container will occupy exactly this many units regardless
    /// of its content's size or the parent's constraints. This effectively
    /// "locks" the widget's size, preventing it from expanding or
    /// shrinking during the layout pass.
    #[builder(with = |v: usize| StyleProperty::Explicit(v), default)]
    height: StyleProperty<usize>,

    /// The single nested widget managed by this container.
    ///
    /// As a single-child provider, the container acts as a wrapper,
    /// applying its own alignment, background, and border rules to
    /// this inner element.
    child: Option<Widget>,
}

/// A targeted configuration set used to override or provide specific
/// parameters for a Container widget based on its unique identifier.
///
/// Instead of traversing the widget tree to modify an existing Container,
/// this struct allows external systems to inject layout and styling
/// data—such as alignment and borders—directly into the widget's
/// compilation phase. If no configuration is associated with a
/// widget's ID, it continues to use its own internal state.
#[widget_style(kind = minimal, targets(Container, FlexContainer))]
#[derive(bon::Builder, Debug, Clone)]
pub struct ContainerStyle {
    pub background_color: Color,
    pub border: Border,
    pub spacing: Spacing,
    pub alignment: Alignment,
}

impl WidgetInformation for Container {
    fn get_id(&self) -> WidgetId {
        self.id
    }

    fn set_id(&mut self, id: WidgetId) {
        self.id = id;
    }

    fn get_key(&self) -> Option<&WidgetKey> {
        self.key.as_ref()
    }

    fn get_class(&self) -> WidgetClass {
        self.class.clone()
    }
}

impl WidgetGetType for Container {
    fn get_type(&self) -> &'static str {
        "container"
    }
}

impl WidgetSizingMode for Container {
    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Fixed
    }
}

impl<C> Init<C> for Container
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
            self.configure(container_style.clone());
        }

        if let Some(child) = &mut self.child {
            child.init(context);
        }
    }
}

impl<C> Invalidate<C> for Container
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, _context: &mut C, style: WidgetStyle) {
        if let WidgetStyle::Container(container_style) = style {
            self.configure(container_style);
        }
    }

    fn on_rebuild(&mut self, _context: &mut C) -> RebuildStatus {
        RebuildStatus::NothingChanged
    }

    fn invalidate_children(&mut self, visitor: &mut impl InvalidateVisitor<C>) {
        if let Some(child) = &mut self.child {
            visitor.invalidate(child);
        }
    }
}

impl Measure<f32> for Container {
    fn intrinsic_content<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        Content::intrinsic_fn(|| {
            if let Some(child) = &self.child {
                child.intrinsic(context)
            } else {
                measure::Intrinsic::default()
            }
        })
        .spacing(self.spacing.unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .intrinsic()
    }

    fn measure_children(&self, visitor: &mut impl measure::MeasureVisitor<f32>) {
        if let Some(child) = &self.child {
            visitor.measure(child);
        }
    }

    fn measure_content<C>(
        &self,
        context: &mut C,
        constraints: Constraints<Extent<f32>>,
    ) -> Extent<f32>
    where
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        Content::measure_fn(|container_constraints| {
            if let Some(child) = &self.child {
                child.measure(context, container_constraints)
            } else {
                Extent::default()
            }
        })
        .spacing(self.spacing.unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .measure(constraints)
    }
}

impl<C> Layout<C, f32> for Container
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &C) {
        if context.load(self.id).is_none() {
            warn!("Container widget with id {} didn't measured!", *self.id);
        }

        if let Some(child) = &mut self.child {
            child.layout(context);
        }
    }
}

impl<C> Draw<C, f32> for Container
where
    C: DrawContext<f32>,
{
    fn draw_content(
        &self,
        context: &C,
        offset: &Offset<f32>,
        provided_extent: Extent<f32>,
        drawer: &mut Drawer,
    ) {
        Content::draw_fn(
            |offset: &Offset<f32>, provided_extent: Extent<f32>, drawer: &mut Drawer| {
                if let Some(child) = &self.child {
                    let alignment = self.alignment.clone().unwrap_or_default();
                    let child_extent =
                        <C as LoadExtent<f32, WidgetId>>::load(context, child.get_id())
                            .unwrap_or_default();

                    let horizontal_start = alignment
                        .horizontal
                        .get_start(provided_extent.width, child_extent.width);
                    let vertical_start = alignment
                        .vertical
                        .get_start(provided_extent.height, child_extent.height);

                    let offset_for_child = *offset + Offset::new(horizontal_start, vertical_start);
                    child.draw(context, &offset_for_child, drawer);

                    if context.get_debug_options().show_layout_bounds {
                        draw_debug_bounds(drawer.surface.canvas(), *offset, provided_extent);
                    }
                }
            },
        )
        .spacing(self.spacing.unwrap_or_default())
        .background(self.background_color.clone().unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .draw(offset, provided_extent, drawer);
    }
}

impl<C> EventHitTest<f32, C> for Container
where
    C: EventContext<f32>,
{
    fn on_hit_test(
        &self,
        context: &C,
        local_coords: Point<f32>,
        provided_extent: Extent<f32>,
        router: &mut EventRouter,
    ) -> HitTestResult {
        Content::hit_test_fn(
            |local_coords: Point<f32>, _provided_extent: Extent<f32>, router: &mut EventRouter| {
                if let Some(child) = &self.child {
                    let result = child.hit_test(context, local_coords, router);

                    match result {
                        HitTestResult::Hit => router.set_next_index(self.id, 0),
                        HitTestResult::Missed | HitTestResult::Failed => (),
                    }

                    result
                } else {
                    HitTestResult::Missed
                }
            },
        )
        .spacing(self.spacing.unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .hit_test(self.id, local_coords, provided_extent, router)
    }
}

impl<C> EventHandling<f32, C> for Container
where
    C: EventContext<f32>,
{
    fn handle_events(
        &mut self,
        context: &mut C,
        _pending_events: Vec<PendingEvent>,
        _next_child: usize,
        router: &EventRouter,
    ) {
        if let Some(child) = &mut self.child {
            child.route_events(context, router);
        }
    }
}
