use std::ops::{Add, AddAssign, Sub, SubAssign};

use num_traits::FromPrimitive;

use crate::types::{Extent, Offset, Spacing};

#[derive(Default, Debug, Clone, Copy)]
pub struct Point<T>
where
    T: Default + Copy,
{
    pub x: T,
    pub y: T,
}

impl<T> Point<T>
where
    T: Default + Copy,
{
    pub fn is_any_negative(&self) -> bool
    where
        T: PartialOrd,
    {
        self.x < T::default() || self.y < T::default()
    }

    pub fn is_inside_of(&self, extent: Extent<T>) -> bool
    where
        T: PartialOrd,
    {
        !self.is_any_negative() && self.x <= extent.width && self.y <= extent.height
    }
}

impl<T> Add for Point<T>
where
    T: Default + Copy + Add<Output = T>,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T> AddAssign for Point<T>
where
    T: Default + Copy + Add<Output = T>,
{
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
    }
}
impl<T> Sub for Point<T>
where
    T: Default + Copy + Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl<T> SubAssign for Point<T>
where
    T: Default + Copy + Sub<Output = T>,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.x = self.x - rhs.x;
        self.y = self.y - rhs.y;
    }
}

impl<T> From<Spacing> for Point<T>
where
    T: Default + Copy + FromPrimitive,
{
    fn from(value: Spacing) -> Self {
        // INFO: since our coordinate system is placed at top left corner, we use top and left
        // offstes
        let x = T::from_usize(value.left).unwrap_or_default();
        let y = T::from_usize(value.top).unwrap_or_default();

        Point { x, y }
    }
}

impl<T> From<Offset<T>> for Point<T>
where
    T: Default + Copy,
{
    fn from(value: Offset<T>) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}
