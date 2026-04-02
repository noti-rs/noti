use crate::{
    context::{LoadExtent, LoadIntrinsic, SaveExtent, SaveIntrinsic},
    types::{Extent, WidgetId},
};

pub enum SizingMode {
    Fixed,
    Dynamic,
}

impl SizingMode {
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed)
    }
}

pub trait IntrinsicManagement<T, Id>: SaveIntrinsic<T, Id> + LoadIntrinsic<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> IntrinsicManagement<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: SaveIntrinsic<T, Id> + LoadIntrinsic<T, Id>,
{
}

pub trait ExtentManagement<T, Id>: SaveExtent<T, Id> + LoadExtent<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> ExtentManagement<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: SaveExtent<T, Id> + LoadExtent<T, Id>,
{
}

pub trait Measure<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn get_intrinsic<C>(&self, context: &mut C) -> Intrinsic<T>
    where
        C: IntrinsicManagement<T, Id>;

    fn measure<C>(&self, context: &mut C, constraints: Constraints<Extent<T>>) -> Extent<T>
    where
        C: IntrinsicManagement<T, Id> + ExtentManagement<T, Id>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Intrinsic<T>
where
    T: Default + Copy,
{
    pub min: Extent<T>,
    pub max: Extent<T>,
}

impl<T> Intrinsic<T>
where
    T: Default + Copy,
{
    pub fn new(min: Extent<T>, max: Extent<T>) -> Self {
        Self { min, max }
    }

    pub fn is_fixed(&self) -> bool
    where
        T: PartialEq,
    {
        self.min == self.max
    }
}

#[derive(Default, Clone, Copy)]
pub struct Constraints<T>
where
    T: Default + Copy,
{
    pub min: T,
    pub max: T,
}

impl<T> Constraints<T>
where
    T: Default + Copy,
{
    pub fn new_tight(val: T) -> Self {
        Self { min: val, max: val }
    }

    pub fn new_soft(val: T) -> Self {
        Self {
            min: T::default(),
            max: val,
        }
    }
}
