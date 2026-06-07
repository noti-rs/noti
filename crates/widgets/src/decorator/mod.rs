use std::ops::{Add, Sub};

use num_traits::FromPrimitive;

use crate::{
    decorator::{border::BorderDecorator, box_size::BoxSizeDecorator, spacing::SpacingDecorator},
    measure::{Constraints, Intrinsic},
    types::{Border, Extent, Spacing},
};

pub(crate) mod background;
pub(crate) mod border;
pub(crate) mod box_size;
pub(crate) mod content;
pub(crate) mod spacing;

pub(crate) trait DecoratorType:
    Default + Copy + FromPrimitive + PartialOrd + Add<Output = Self> + Sub<Output = Self>
{
}

impl<T> DecoratorType for T where
    T: Default + Copy + FromPrimitive + PartialOrd + Add<Output = Self> + Sub<Output = Self>
{
}

pub(crate) trait MeasureDecorator<T>
where
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T>;
    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T>;
}

pub(crate) trait DecoratorExt<T>: MeasureDecorator<T> + Sized
where
    T: DecoratorType,
{
    fn spacing(self, spacing: Spacing) -> SpacingDecorator<Self> {
        SpacingDecorator {
            spacing,
            next: self,
        }
    }

    fn border(self, border: Border) -> BorderDecorator<Self> {
        BorderDecorator { border, next: self }
    }

    fn box_size<S: Into<Option<T>>>(self, width: S, height: S) -> BoxSizeDecorator<Self, T> {
        BoxSizeDecorator {
            width: width.into(),
            height: height.into(),
            ratio: None,
            next: self,
        }
    }

    fn box_size_with_ratio<S: Into<Option<T>>, R: Into<Option<f32>>>(
        self,
        width: S,
        height: S,
        ratio: R,
    ) -> BoxSizeDecorator<Self, T> {
        BoxSizeDecorator {
            width: width.into(),
            height: height.into(),
            ratio: ratio.into(),
            next: self,
        }
    }
}

impl<D, T> DecoratorExt<T> for D
where
    D: MeasureDecorator<T>,
    T: DecoratorType,
{
}
