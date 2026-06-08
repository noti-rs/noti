use std::marker::PhantomData;

use crate::{
    decorator::{DecoratorType, DrawDecorator, MeasureDecorator},
    draw::Drawer,
    measure::{Constraints, Intrinsic},
    types::{Extent, Offset},
};

pub(crate) struct Content<F, FnT> {
    function: F,
    _marker: PhantomData<FnT>,
}

pub(crate) struct IntrinsicFn;
pub(crate) struct MeasureFn;
pub(crate) struct DrawFn;

impl<F> Content<F, IntrinsicFn> {
    pub(crate) fn intrinsic_fn<T>(f: F) -> Self
    where
        F: FnMut() -> Intrinsic<T>,
        T: DecoratorType,
    {
        Self {
            function: f,
            _marker: PhantomData,
        }
    }
}

impl<F> Content<F, MeasureFn> {
    pub(crate) fn measure_fn<T>(f: F) -> Self
    where
        F: FnMut(Constraints<Extent<T>>) -> Extent<T>,
        T: DecoratorType,
    {
        Self {
            function: f,
            _marker: PhantomData,
        }
    }
}

impl<F> Content<F, DrawFn> {
    pub(crate) fn draw_fn<T>(f: F) -> Self
    where
        F: Fn(&Offset<T>, Extent<T>, &mut Drawer),
        T: DecoratorType,
    {
        Self {
            function: f,
            _marker: PhantomData,
        }
    }
}

impl<T, F> MeasureDecorator<T> for Content<F, IntrinsicFn>
where
    T: DecoratorType,
    F: FnMut() -> Intrinsic<T>,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        (self.function)()
    }

    fn measure(&mut self, _constraints: Constraints<Extent<T>>) -> Extent<T> {
        Extent::default()
    }
}

impl<T, F> DrawDecorator<T> for Content<F, IntrinsicFn>
where
    T: DecoratorType,
{
    fn draw(&self, _offset: &Offset<T>, _provided_extent: Extent<T>, _drawer: &mut Drawer) {
        // No op, just allow to implement trait
    }
}

impl<T, F> MeasureDecorator<T> for Content<F, MeasureFn>
where
    T: DecoratorType,
    F: FnMut(Constraints<Extent<T>>) -> Extent<T>,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        Intrinsic::default()
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        (self.function)(constraints)
    }
}

impl<T, F> DrawDecorator<T> for Content<F, MeasureFn>
where
    T: DecoratorType,
{
    fn draw(&self, _offset: &Offset<T>, _provided_extent: Extent<T>, _drawer: &mut Drawer) {
        // No op, just allow to implement trait
    }
}

impl<T, F> MeasureDecorator<T> for Content<F, DrawFn>
where
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        Intrinsic::default()
    }

    fn measure(&mut self, _constraints: Constraints<Extent<T>>) -> Extent<T> {
        Extent::default()
    }
}

impl<T, F> DrawDecorator<T> for Content<F, DrawFn>
where
    T: DecoratorType,
    F: Fn(&Offset<T>, Extent<T>, &mut Drawer),
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer) {
        (self.function)(offset, provided_extent, drawer)
    }
}
