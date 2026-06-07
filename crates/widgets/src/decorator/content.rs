use std::marker::PhantomData;

use crate::{
    decorator::{DecoratorType, MeasureDecorator},
    measure::{Constraints, Intrinsic},
    types::Extent,
};

pub(crate) struct Content<F, FnT> {
    function: F,
    _marker: PhantomData<FnT>,
}

pub(crate) struct IntrinsicFn;
pub(crate) struct MeasureFn;

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
