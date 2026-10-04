use macros::widget;

use crate::{
    context::{LoadExtent, ManageIntrinsic},
    decorator::{
        content::Content, DecoratorExt, DrawDecorator, EventHitTestDecorator, LayoutDecorator,
        MeasureDecorator,
    },
    events::{EventContext, EventHandling, EventHitTest, EventRouter, HitTestResult, PendingEvent},
    stage::{
        deinit::{Deinit, DeinitContext},
        draw::{draw_debug_bounds, Draw, DrawContext, Drawer},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext, RebuildStatus},
        layout::{Layout, LayoutContext},
        measure::{self, Constraints, ManageMeasures, Measure, MeasureContext, SizingMode},
    },
    types::{
        extent::Extent,
        identifiers::{WidgetId, WidgetKey},
        offset::Offset,
        spacing::Spacing,
        Point,
    },
    widget::{WidgetGetType, WidgetInformationContext, WidgetSizingMode},
};

/// A simple box that stays the same size.
///
/// Unlike the [`FlexBox`], which stretches and moves to fit many things,
/// the `Box` is a rigid frame for just **one** child widget.
///
/// The Box does three main things:
/// * It sets a fixed width and height that never change.
/// * It holds exactly one child widget inside itself.
/// * It acts as a wall, so the child inside cannot push the box to make it bigger.
///
/// Use this when you need a UI element to stay exactly the same,
/// like a fixed icon or a status light that should never grow or shrink.
#[widget(kind = container)]
#[derive(bon::Builder, Default)]
pub struct Box {
    /// A hard-coded, fixed dimension for this axis.
    ///
    /// When set, the container will occupy exactly this many units regardless
    /// of its content's size or the parent's constraints. This effectively
    /// "locks" the widget's size, preventing it from expanding or
    /// shrinking during the layout pass.
    #[style]
    #[dirty(NEEDS_MEASURE)]
    width: usize,

    /// A hard-coded, fixed dimension for this axis.
    ///
    /// When set, the container will occupy exactly this many units regardless
    /// of its content's size or the parent's constraints. This effectively
    /// "locks" the widget's size, preventing it from expanding or
    /// shrinking during the layout pass.
    #[style]
    #[dirty(NEEDS_MEASURE)]
    height: usize,
}

impl WidgetGetType for Box {
    fn get_type(&self) -> &'static str {
        "box"
    }
}

impl<C> WidgetSizingMode<C> for Box
where
    C: WidgetInformationContext,
{
    fn sizing_mode(&self, _context: &C) -> SizingMode {
        SizingMode::Fixed
    }
}

impl<C> Init<C> for Box
where
    C: InitContext,
{
    fn on_init(&mut self, _context: &mut C) {}
}

impl<C> Deinit<C> for Box where C: DeinitContext {}

impl<C> Invalidate<C> for Box
where
    C: InvalidateContext,
{
    fn on_rebuild(&mut self, _context: &mut C) -> RebuildStatus {
        RebuildStatus::NothingChanged
    }
}

impl<C> Measure<C, f32> for Box
where
    C: MeasureContext<f32>,
{
    fn intrinsic_content(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId>,
    {
        Content::intrinsic_fn(|| {
            context
                .childrens_identifiers_of(self.id)
                .first()
                .and_then(|child_widget_id| context.widget_intrinsic(child_widget_id))
                .unwrap_or_default()
        })
        .spacing(self.padding.unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .spacing(self.margin.unwrap_or_default())
        .intrinsic()
    }

    fn measure_content(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: ManageMeasures<f32, WidgetId>,
    {
        Content::measure_fn(|container_constraints| {
            context
                .childrens_identifiers_of(self.id)
                .first()
                .and_then(|child_widget_id| {
                    context.measure_widget(child_widget_id, container_constraints)
                })
                .unwrap_or_default()
        })
        .spacing(self.padding.unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .spacing(self.margin.unwrap_or_default())
        .measure(constraints)
    }
}

impl<C> Layout<C, f32> for Box
where
    C: LayoutContext<f32>,
{
    fn on_layout(
        &mut self,
        context: &mut C,
        local_coord: Point<f32>,
        provided_extent: Extent<f32>,
    ) {
        Content::layout_fn(|local_coord: Point<f32>, _provided_extent: Extent<f32>| {
            if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
                context.layout_widget(child_widget_id, local_coord);
            }
        })
        .spacing(self.padding.unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .spacing(self.margin.unwrap_or_default())
        .layout(local_coord, provided_extent);
    }
}

impl<C> Draw<C, f32> for Box
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
                if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
                    let alignment = self.alignment.clone().unwrap_or_default();
                    let child_extent =
                        <C as LoadExtent<f32, WidgetId>>::load(context, *child_widget_id)
                            .unwrap_or_default();

                    let horizontal_start = alignment
                        .horizontal
                        .get_start(provided_extent.width, child_extent.width);
                    let vertical_start = alignment
                        .vertical
                        .get_start(provided_extent.height, child_extent.height);

                    let offset_for_child = *offset + Offset::new(horizontal_start, vertical_start);
                    context.draw_widget(child_widget_id, &offset_for_child, drawer);

                    if context.get_debug_options().show_layout_bounds {
                        draw_debug_bounds(drawer.surface.canvas(), *offset, provided_extent);
                    }
                }
            },
        )
        .spacing(self.padding.unwrap_or_default())
        .background(self.background_color.clone().unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .spacing(self.margin.unwrap_or_default())
        .draw(offset, provided_extent, drawer);
    }
}

impl<C> EventHitTest<C, f32> for Box
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
                if let Some(hit_result) = context
                    .childrens_identifiers_of(self.id)
                    .first()
                    .and_then(|child_widget_id| {
                        context.hit_test_widget(child_widget_id, local_coords, router)
                    })
                {
                    match hit_result {
                        HitTestResult::Hit => router.set_next_index(self.id, 0),
                        HitTestResult::Missed | HitTestResult::Failed => (),
                    }

                    hit_result
                } else {
                    HitTestResult::Missed
                }
            },
        )
        .spacing(self.padding.unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .box_size(
            self.width.as_option().map(|&width| width as f32),
            self.height.as_option().map(|&height| height as f32),
        )
        .spacing(self.margin.unwrap_or_default())
        .hit_test(self.id, local_coords, provided_extent, router)
    }
}

impl<C> EventHandling<C, f32> for Box
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
        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.route_events_to_widget(child_widget_id, router);
        }
    }
}
