use crate::events::{EventContext, EventHandling, EventHitTest};
use crate::stage::deinit::{Deinit, DeinitContext};
use crate::stage::draw::{Draw, DrawContext};
use crate::stage::init::{Init, InitContext};
use crate::stage::invalidate::{Invalidate, InvalidateContext};
use crate::stage::layout::{Layout, LayoutContext};
use crate::stage::measure::{Constraints, Measure, MeasureContext};
use crate::types::Extent;
use crate::widget::{WidgetGetType, WidgetInformationContext, WidgetKey, WidgetSizingMode};
use crate::WidgetId;
use macros::widget;

/// Creates a constraint box for a single child that overrides the constraints during measurement stage.
///
/// NOTE: The maximum must be greater than or equal to the minimum for each dimension.
/// Otherwise the behavior will be undefined!
#[widget(kind = minimal)]
#[derive(bon::Builder, Default, Debug)]
pub struct ConstrainedBox {
    #[style]
    #[dirty(NEEDS_MEASURE)]
    min_width: usize,

    #[style]
    #[dirty(NEEDS_MEASURE)]
    max_width: usize,

    #[style]
    #[dirty(NEEDS_MEASURE)]
    min_height: usize,

    #[style]
    #[dirty(NEEDS_MEASURE)]
    max_height: usize,
}

impl ConstrainedBox {
    fn constraints(&self) -> Constraints<Extent<f32>> {
        let min = Extent::new(
            self.min_width.unwrap_or(0) as f32,
            self.min_height.unwrap_or(0) as f32,
        );
        let max = Extent::new(
            self.max_width.map(|val| *val as f32).unwrap_or(f32::MAX),
            self.max_height.map(|val| *val as f32).unwrap_or(f32::MAX),
        );

        Constraints { min, max }
    }
}

impl WidgetGetType for ConstrainedBox {
    fn get_type(&self) -> &'static str {
        "constrained_box"
    }
}

impl<C> WidgetSizingMode<C> for ConstrainedBox
where
    C: WidgetInformationContext,
{
    fn sizing_mode(&self, _context: &C) -> crate::stage::measure::SizingMode {
        use crate::types::style::StyleProperty::*;
        if let (
            Explicit(min_width),
            Explicit(max_width),
            Explicit(min_height),
            Explicit(max_height),
        ) = (
            self.min_width,
            self.max_width,
            self.min_height,
            self.max_height,
        ) {
            if min_width == max_width && min_height == max_height {
                return crate::stage::measure::SizingMode::Fixed;
            }
        }

        crate::stage::measure::SizingMode::Dynamic
    }
}

impl<C> Init<C> for ConstrainedBox
where
    C: InitContext,
{
    fn on_init(&mut self, _context: &mut C) {}
}

impl<C> Deinit<C> for ConstrainedBox
where
    C: DeinitContext,
{
    fn on_deinit(&mut self, _context: &mut C) {}
}

impl<C> Invalidate<C> for ConstrainedBox
where
    C: InvalidateContext,
{
    fn on_rebuild(&mut self, _context: &mut C) -> crate::stage::invalidate::RebuildStatus {
        crate::stage::invalidate::RebuildStatus::NothingChanged
    }
}

impl<C> Measure<C, f32> for ConstrainedBox
where
    C: MeasureContext<f32>,
{
    fn intrinsic_content(&self, context: &mut C) -> crate::stage::measure::Intrinsic<f32>
    where
        C: crate::context::ManageIntrinsic<f32, WidgetId>,
    {
        let mut intrinsic = context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| context.widget_intrinsic(child_widget_id))
            .unwrap_or_default();

        let constraints = self.constraints();
        intrinsic.min.clamp(constraints.min, constraints.max);
        intrinsic.max.clamp(constraints.min, constraints.max);

        intrinsic
    }

    fn measure_content(
        &self,
        context: &mut C,
        incoming_constraints: crate::stage::measure::Constraints<crate::types::Extent<f32>>,
    ) -> crate::types::Extent<f32>
    where
        C: crate::stage::measure::ManageMeasures<f32, WidgetId>,
    {
        /// Finds the intersection between both constraints.
        ///
        /// If there's no intersection between them, then picks optimal from one of both constraints
        /// by such rule: find minimum of both maximums and use it as new min and max.
        fn find_intersection(
            left_constraints: Constraints<f32>,
            right_constraints: Constraints<f32>,
        ) -> Constraints<f32> {
            let min = left_constraints.min.max(right_constraints.min);
            let max = left_constraints.max.min(right_constraints.max);

            if min <= max {
                Constraints { min, max }
            } else {
                Constraints { min: max, max }
            }
        }

        context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| {
                let constraints = self.constraints();

                let width_constraints = find_intersection(
                    Constraints {
                        min: incoming_constraints.min.width,
                        max: incoming_constraints.max.width,
                    },
                    Constraints {
                        min: constraints.min.width,
                        max: constraints.max.width,
                    },
                );

                let height_constraints = find_intersection(
                    Constraints {
                        min: incoming_constraints.min.height,
                        max: incoming_constraints.max.height,
                    },
                    Constraints {
                        min: constraints.min.height,
                        max: constraints.max.height,
                    },
                );
                context.measure_widget(
                    child_widget_id,
                    Constraints {
                        min: Extent::new(width_constraints.min, height_constraints.min),
                        max: Extent::new(width_constraints.max, height_constraints.max),
                    },
                )
            })
            .unwrap_or_default()
            .clamp_with(incoming_constraints.min, incoming_constraints.max)
    }
}

impl<C> Layout<C, f32> for ConstrainedBox
where
    C: LayoutContext<f32>,
{
    fn on_layout(
        &mut self,
        context: &mut C,
        local_coord: crate::types::Point<f32>,
        _provided_extent: Extent<f32>,
    ) {
        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.layout_widget(child_widget_id, local_coord);
        }
    }
}

impl<C> Draw<C, f32> for ConstrainedBox
where
    C: DrawContext<f32>,
{
    fn draw_content(
        &self,
        context: &C,
        offset: &crate::types::Offset<f32>,
        _provided_extent: Extent<f32>,
        drawer: &mut crate::stage::draw::Drawer,
    ) {
        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.draw_widget(child_widget_id, offset, drawer);
        }
    }
}

impl<C> EventHitTest<C, f32> for ConstrainedBox
where
    C: EventContext<f32>,
{
    fn on_hit_test(
        &self,
        context: &C,
        local_coords: crate::types::Point<f32>,
        _provided_extent: Extent<f32>,
        router: &mut crate::events::EventRouter,
    ) -> crate::events::HitTestResult {
        context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| {
                context.hit_test_widget(child_widget_id, local_coords, router)
            })
            .inspect(|hit_result| match hit_result {
                crate::events::HitTestResult::Hit => router.set_next_index(self.id, 0),
                crate::events::HitTestResult::Failed | crate::events::HitTestResult::Missed => (),
            })
            .unwrap_or(crate::events::HitTestResult::Missed)
    }
}

impl<C> EventHandling<C, f32> for ConstrainedBox
where
    C: EventContext<f32>,
{
    fn handle_events(
        &mut self,
        context: &mut C,
        _pending_events: Vec<crate::events::PendingEvent>,
        _next_child: usize,
        router: &crate::events::EventRouter,
    ) {
        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.route_events_to_widget(child_widget_id, router);
        }
    }
}
