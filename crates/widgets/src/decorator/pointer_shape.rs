use crate::{
    decorator::{
        DecoratorType, DrawDecorator, EventHitTestDecorator, LayoutDecorator, MeasureDecorator,
    },
    events::PointerShape,
};

pub(crate) struct PointerShapeDecorator<N> {
    pub(super) next: N,
    pub(super) pointer_shape: PointerShape,
}

impl<T, N> MeasureDecorator<T> for PointerShapeDecorator<N>
where
    T: DecoratorType,
    N: MeasureDecorator<T>,
{
    fn intrinsic(&mut self) -> crate::stage::measure::Intrinsic<T> {
        self.next.intrinsic()
    }

    fn measure(
        &mut self,
        constraints: crate::stage::measure::Constraints<crate::types::Extent<T>>,
    ) -> crate::types::Extent<T> {
        self.next.measure(constraints)
    }
}

impl<T, N> LayoutDecorator<T> for PointerShapeDecorator<N>
where
    T: DecoratorType,
    N: LayoutDecorator<T>,
{
    fn layout(
        &mut self,
        local_coord: crate::types::Point<T>,
        provided_extent: crate::types::Extent<T>,
    ) {
        self.next.layout(local_coord, provided_extent);
    }
}

impl<T, N> DrawDecorator<T> for PointerShapeDecorator<N>
where
    T: DecoratorType,
    N: DrawDecorator<T>,
{
    fn draw(
        &self,
        offset: &crate::types::Offset<T>,
        provided_extent: crate::types::Extent<T>,
        drawer: &mut crate::stage::draw::Drawer,
    ) {
        self.next.draw(offset, provided_extent, drawer);
    }
}

impl<T, N> EventHitTestDecorator<T> for PointerShapeDecorator<N>
where
    T: DecoratorType,
    N: EventHitTestDecorator<T>,
{
    fn on_hit_test(
        &self,
        widget_id: crate::types::WidgetId,
        local_coords: crate::types::Point<T>,
        provided_extent: crate::types::Extent<T>,
        router: &mut crate::events::EventRouter,
    ) -> crate::events::HitTestResult {
        router.set_pointer_shape(widget_id, self.pointer_shape);

        self.next
            .hit_test(widget_id, local_coords, provided_extent, router)
    }
}
