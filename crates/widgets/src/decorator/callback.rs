use std::marker::PhantomData;

use crate::{
    decorator::{DecoratorType, DrawDecorator, EventHitTestDecorator, MeasureDecorator},
    events::{EventNodeCapabilities, EventRouter, HitTestResult},
    stage::{
        draw::Drawer,
        measure::{Constraints, Intrinsic},
    },
    types::{Extent, Offset, Point, WidgetId},
};

pub(crate) struct EventDecorator<CallbackType, N> {
    next: N,
    _marker: PhantomData<CallbackType>,
}

pub(crate) struct Hoverable;
pub(crate) struct Pressable;
pub(crate) struct Clickable;

impl<N> EventDecorator<Hoverable, N> {
    pub(crate) fn hoverable(next: N) -> Self {
        Self {
            next,
            _marker: PhantomData,
        }
    }
}

impl<N> EventDecorator<Pressable, N> {
    #[allow(unused)]
    pub(crate) fn pressable(next: N) -> Self {
        Self {
            next,
            _marker: PhantomData,
        }
    }
}

impl<N> EventDecorator<Clickable, N> {
    #[allow(unused)]
    pub(crate) fn clickable(next: N) -> Self {
        Self {
            next,
            _marker: PhantomData,
        }
    }
}

impl<CallbackType, N, T> MeasureDecorator<T> for EventDecorator<CallbackType, N>
where
    N: MeasureDecorator<T>,
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        self.next.intrinsic()
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        self.next.measure(constraints)
    }
}

impl<CallbackType, N, T> DrawDecorator<T> for EventDecorator<CallbackType, N>
where
    N: DrawDecorator<T>,
    T: DecoratorType,
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer) {
        self.next.draw(offset, provided_extent, drawer);
    }
}

impl<N, T> EventHitTestDecorator<T> for EventDecorator<Hoverable, N>
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
        router.add_capability(widget_id, EventNodeCapabilities::HOVER);

        let mut result = self
            .next
            .hit_test(widget_id, local_coords, provided_extent, router);

        match &result {
            HitTestResult::Hit => (),
            HitTestResult::Missed => result = HitTestResult::Hit,
            HitTestResult::Failed => (),
        }

        result
    }
}

impl<N, T> EventHitTestDecorator<T> for EventDecorator<Pressable, N>
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
        router.add_capability(widget_id, EventNodeCapabilities::PRESS);

        let mut result = self
            .next
            .hit_test(widget_id, local_coords, provided_extent, router);

        match &result {
            HitTestResult::Hit => (),
            HitTestResult::Missed => result = HitTestResult::Hit,
            HitTestResult::Failed => (),
        }

        result
    }
}

impl<N, T> EventHitTestDecorator<T> for EventDecorator<Clickable, N>
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
        router.add_capability(widget_id, EventNodeCapabilities::CLICK);

        let mut result = self
            .next
            .hit_test(widget_id, local_coords, provided_extent, router);

        match &result {
            HitTestResult::Hit => (),
            HitTestResult::Missed => result = HitTestResult::Hit,
            HitTestResult::Failed => (),
        }

        result
    }
}
