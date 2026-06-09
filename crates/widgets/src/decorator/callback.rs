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

pub(crate) struct CallbackDecorator<'a, C, CallbackType, N> {
    callback: &'a mut C,
    next: N,
    _marker: PhantomData<CallbackType>,
}

pub(crate) struct OnHover;
pub(crate) struct OnPress;
pub(crate) struct OnClick;

impl<'a, C, N> CallbackDecorator<'a, C, OnHover, N> {
    pub(crate) fn on_hover(callback: &'a mut C, next: N) -> Self {
        Self {
            callback,
            next,
            _marker: PhantomData,
        }
    }
}

impl<'a, C, N> CallbackDecorator<'a, C, OnPress, N> {
    pub(crate) fn on_press(callback: &'a mut C, next: N) -> Self {
        Self {
            callback,
            next,
            _marker: PhantomData,
        }
    }
}

impl<'a, C, N> CallbackDecorator<'a, C, OnClick, N> {
    pub(crate) fn on_click(callback: &'a mut C, next: N) -> Self {
        Self {
            callback,
            next,
            _marker: PhantomData,
        }
    }
}

impl<'a, C, CallbackType, N, T> MeasureDecorator<T> for CallbackDecorator<'a, C, CallbackType, N>
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

impl<'a, C, CallbackType, N, T> DrawDecorator<T> for CallbackDecorator<'a, C, CallbackType, N>
where
    N: DrawDecorator<T>,
    T: DecoratorType,
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer) {
        self.next.draw(offset, provided_extent, drawer);
    }
}

impl<'a, C, N, T> EventHitTestDecorator<T> for CallbackDecorator<'a, C, OnHover, N>
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
        router.add_capability(widget_id, EventNodeCapabilities::HOVERED);

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

impl<'a, C, N, T> EventHitTestDecorator<T> for CallbackDecorator<'a, C, OnPress, N>
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
        router.add_capability(widget_id, EventNodeCapabilities::PRESSED);

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

impl<'a, C, N, T> EventHitTestDecorator<T> for CallbackDecorator<'a, C, OnClick, N>
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
        router.add_capability(widget_id, EventNodeCapabilities::CLICKED);

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
