use crate::{
    decorator::{DecoratorType, DrawDecorator, EventHitTestDecorator, MeasureDecorator},
    events::{EventRouter, HitTestResult},
    stage::{
        draw::Drawer,
        measure::{Constraints, Intrinsic},
    },
    types::{Extent, Offset, Point, Spacing, WidgetId},
};
pub(crate) struct SpacingDecorator<N> {
    pub(super) spacing: Spacing,
    pub(super) next: N,
}

impl<N, T> MeasureDecorator<T> for SpacingDecorator<N>
where
    N: MeasureDecorator<T>,
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        self.next.intrinsic() + self.spacing
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        let new_constraints = constraints.shrink_to_with(&self.spacing);

        let used_extent = self.next.measure(new_constraints) + self.spacing.into();

        used_extent.clamp_with(constraints.min, constraints.max)
    }
}

impl<N, T> DrawDecorator<T> for SpacingDecorator<N>
where
    N: DrawDecorator<T>,
    T: DecoratorType,
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer) {
        if provided_extent.is_collapsed() {
            return;
        }

        self.next.draw(
            &(*offset + self.spacing.into()),
            provided_extent.shrink_to_with(&self.spacing),
            drawer,
        )
    }
}

impl<N, T> EventHitTestDecorator<T> for SpacingDecorator<N>
where
    N: EventHitTestDecorator<T>,
    T: DecoratorType,
{
    fn on_hit_test(
        &self,
        widget_id: WidgetId,
        local_coords: Point<T>,
        provided_extent: Extent<T>,
        router: &mut EventRouter,
    ) -> HitTestResult {
        self.next.hit_test(
            widget_id,
            local_coords - self.spacing.into(),
            provided_extent.shrink_to_with(&self.spacing),
            router,
        )
    }
}
