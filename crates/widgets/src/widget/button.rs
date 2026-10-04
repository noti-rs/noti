use crate::{
    context::ScopedContext,
    decorator::{content::Content, DecoratorExt, EventHitTestDecorator},
    events::{EventContext, EventHandling, EventHitTest, EventRouter, HitTestResult},
    stage::{
        deinit::{Deinit, DeinitContext},
        draw::{Draw, DrawContext},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext},
        layout::{Layout, LayoutContext},
        measure::{Measure, MeasureContext},
    },
    types::{Extent, Point},
    widget::{WidgetGetType, WidgetInformationContext, WidgetKey, WidgetSizingMode},
    WidgetId,
};
use macros::widget;

#[widget(kind = minimal)]
#[callback(on_click)]
#[callback(on_press)]
#[derive(bon::Builder)]
pub struct Button {}

impl WidgetGetType for Button {
    fn get_type(&self) -> &'static str {
        "button"
    }
}

impl<C> WidgetSizingMode<C> for Button
where
    C: WidgetInformationContext,
{
    fn sizing_mode(&self, context: &C) -> crate::stage::measure::SizingMode {
        context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| context.widget_sizing_mode(child_widget_id))
            .unwrap_or(crate::stage::measure::SizingMode::Fixed)
    }
}

impl<C> Init<C> for Button
where
    C: InitContext,
{
    fn on_init(&mut self, _context: &mut C) {}
}

impl<C> Deinit<C> for Button
where
    C: DeinitContext,
{
    fn on_deinit(&mut self, _context: &mut C) {}
}

impl<C> Invalidate<C> for Button
where
    C: InvalidateContext,
{
    fn on_rebuild(&mut self, _context: &mut C) -> crate::stage::invalidate::RebuildStatus {
        crate::stage::invalidate::RebuildStatus::NothingChanged
    }
}

impl<C> Measure<C, f32> for Button
where
    C: MeasureContext<f32>,
{
    fn intrinsic_content(&self, context: &mut C) -> crate::stage::measure::Intrinsic<f32>
    where
        C: crate::context::ManageIntrinsic<f32, WidgetId>,
    {
        context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| context.widget_intrinsic(child_widget_id))
            .unwrap_or_default()
    }

    fn measure_content(
        &self,
        context: &mut C,
        constraints: crate::stage::measure::Constraints<crate::types::Extent<f32>>,
    ) -> crate::types::Extent<f32>
    where
        C: crate::stage::measure::ManageMeasures<f32, WidgetId>,
    {
        context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| context.measure_widget(child_widget_id, constraints))
            .unwrap_or_default()
    }
}

impl<C> Layout<C, f32> for Button
where
    C: LayoutContext<f32>,
{
    fn on_layout(
        &mut self,
        context: &mut C,
        local_coord: Point<f32>,
        _provided_extent: Extent<f32>,
    ) {
        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.layout_widget(child_widget_id, local_coord);
        }
    }
}

impl<C> Draw<C, f32> for Button
where
    C: DrawContext<f32>,
{
    fn draw_content(
        &self,
        context: &C,
        offset: &crate::types::Offset<f32>,
        _provided_extent: crate::types::Extent<f32>,
        drawer: &mut crate::stage::draw::Drawer,
    ) {
        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.draw_widget(child_widget_id, offset, drawer);
        }
    }
}

impl<C> EventHitTest<C, f32> for Button
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
                        HitTestResult::Hit => {
                            router.set_next_index(self.id, 0);
                        }
                        HitTestResult::Missed | HitTestResult::Failed => (),
                    }

                    hit_result
                } else {
                    HitTestResult::Missed
                }
            },
        )
        .clickable()
        .pressable()
        .hit_test(self.id, local_coords, provided_extent, router)
    }
}

impl<C> EventHandling<C, f32> for Button
where
    C: EventContext<f32>,
{
    fn handle_events(
        &mut self,
        context: &mut C,
        pending_events: Vec<crate::events::PendingEvent>,
        _next_child: usize,
        router: &EventRouter,
    ) {
        for event in pending_events {
            match event {
                crate::events::PendingEvent::PressIn(_mouse_button) => {
                    if let Some(on_press_handler) = self.on_press.as_mut() {
                        on_press_handler(ScopedContext::new(context), ())
                    }
                }
                crate::events::PendingEvent::Click(_mouse_button) => {
                    if let Some(on_click_handler) = self.on_click.as_mut() {
                        on_click_handler(ScopedContext::new(context), ());
                    }
                }
                _ => (),
            }
        }

        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.route_events_to_widget(child_widget_id, router);
        }
    }
}
