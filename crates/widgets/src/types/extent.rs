use std::{
    cmp::Ordering,
    ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign},
};

use crate::types::Direction;

use super::spacing::Spacing;

/// A simple way to describe the **size** of a flat area.
///
/// This struct acts like a ruler. It tells you exactly how much space
/// a widget takes up on the screen.
///
/// It stores two pieces of information:
/// * **width:** How far the area goes from left to right.
/// * **height:** How far the area goes from top to bottom.
///
/// **Important:** Only use this to talk about "how big" something is.
/// Do not use it to describe a "Point" or a location. It measures
/// the space inside a box, not where the box sits on the map.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Extent<T>
where
    T: Default + Copy,
{
    pub width: T,
    pub height: T,
}

impl<T> Extent<T>
where
    T: Default + Copy,
{
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }

    pub fn new_square(side: T) -> Self {
        Self {
            width: side,
            height: side,
        }
    }

    pub fn new_from_direction(main: T, cross: T, direction: &Direction) -> Self {
        match direction {
            Direction::Horizontal => Self {
                width: main,
                height: cross,
            },
            Direction::Vertical => Self {
                width: cross,
                height: main,
            },
        }
    }

    pub fn new_width(width: T) -> Self {
        Self {
            width,
            ..Default::default()
        }
    }

    pub fn new_height(height: T) -> Self {
        Self {
            height,
            ..Default::default()
        }
    }

    pub fn area(&self) -> T
    where
        T: Mul<Output = T>,
    {
        self.width * self.height
    }

    pub fn by_direction(&self, direction: &Direction) -> T {
        match direction {
            Direction::Horizontal => self.width,
            Direction::Vertical => self.height,
        }
    }

    pub fn shrink_by(&mut self, spacing: &Spacing)
    where
        T: PartialOrd + Sub<Output = T> + num_traits::FromPrimitive,
    {
        let horizontal = T::from_usize(spacing.horizontal()).unwrap_or_default();
        let vertical = T::from_usize(spacing.vertical()).unwrap_or_default();

        self.width = if self.width > horizontal {
            self.width - horizontal
        } else {
            T::default()
        };

        self.height = if self.height > vertical {
            self.height - vertical
        } else {
            T::default()
        };
    }

    pub fn shrink_to_with(&self, spacing: &Spacing) -> Self
    where
        T: PartialOrd + Sub<Output = T> + num_traits::FromPrimitive,
    {
        let mut shrinked_extent = *self;
        shrinked_extent.shrink_by(spacing);
        shrinked_extent
    }

    pub fn clamp(&mut self, min: Self, max: Self)
    where
        T: PartialOrd,
    {
        if let Some(Ordering::Less) = self.width.partial_cmp(&min.width) {
            self.width = min.width;
        }

        if let Some(Ordering::Less) = self.height.partial_cmp(&min.height) {
            self.height = min.height;
        }

        if let Some(Ordering::Greater) = self.width.partial_cmp(&max.width) {
            self.width = max.width;
        }

        if let Some(Ordering::Greater) = self.height.partial_cmp(&max.height) {
            self.height = max.height;
        }
    }

    pub fn clamp_with(&self, min: Self, max: Self) -> Self
    where
        T: PartialOrd,
    {
        let mut clamped_extent = *self;
        clamped_extent.clamp(min, max);
        clamped_extent
    }

    pub fn is_collapsed(&self) -> bool
    where
        T: PartialEq,
    {
        self.width == T::default() || self.height == T::default()
    }

    pub fn to_flex(&self, direction: &Direction) -> FlexExtent<T> {
        match direction {
            Direction::Horizontal => FlexExtent {
                main: self.width,
                cross: self.height,
            },
            Direction::Vertical => FlexExtent {
                main: self.height,
                cross: self.width,
            },
        }
    }
}

impl From<Extent<usize>> for Extent<f32> {
    fn from(value: Extent<usize>) -> Self {
        Self {
            width: value.width as f32,
            height: value.height as f32,
        }
    }
}

impl From<Extent<f32>> for Extent<usize> {
    fn from(value: Extent<f32>) -> Self {
        Self {
            width: value.width.round() as usize,
            height: value.height.round() as usize,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FlexExtent<T>
where
    T: Default + Copy,
{
    pub main: T,
    pub cross: T,
}

impl<T> FlexExtent<T>
where
    T: Default + Copy,
{
    pub fn to_normal(&self, direction: &Direction) -> Extent<T> {
        match direction {
            Direction::Horizontal => Extent {
                width: self.main,
                height: self.cross,
            },
            Direction::Vertical => Extent {
                width: self.cross,
                height: self.main,
            },
        }
    }
}

macro_rules! impl_ops {
    ($type:ident, $field1:ident, $field2:ident) => {
        impl<T> Add<$type<T>> for $type<T>
        where
            T: Add<Output = T> + Default + Copy,
        {
            type Output = $type<T>;

            fn add(self, rhs: $type<T>) -> Self::Output {
                Self {
                    $field1: self.$field1 + rhs.$field1,
                    $field2: self.$field2 + rhs.$field2,
                }
            }
        }

        impl<T> AddAssign<$type<T>> for $type<T>
        where
            T: AddAssign<T> + Default + Copy,
        {
            fn add_assign(&mut self, rhs: $type<T>) {
                self.$field1 += rhs.$field1;
                self.$field2 += rhs.$field2;
            }
        }

        impl<T> Sub<$type<T>> for $type<T>
        where
            T: Sub<Output = T> + Default + Copy,
        {
            type Output = $type<T>;

            fn sub(self, rhs: $type<T>) -> Self::Output {
                Self {
                    $field1: self.$field1 - rhs.$field1,
                    $field2: self.$field2 - rhs.$field2,
                }
            }
        }

        impl<T> SubAssign<$type<T>> for $type<T>
        where
            T: SubAssign<T> + Default + Copy,
        {
            fn sub_assign(&mut self, rhs: $type<T>) {
                self.$field1 -= rhs.$field1;
                self.$field2 -= rhs.$field2;
            }
        }

        impl<T> Mul<T> for $type<T>
        where
            T: Mul<Output = T> + Default + Copy,
        {
            type Output = $type<T>;

            fn mul(self, rhs: T) -> Self::Output {
                Self {
                    $field1: self.$field1 * rhs,
                    $field2: self.$field2 * rhs,
                }
            }
        }

        impl<T> MulAssign<T> for $type<T>
        where
            T: Mul<Output = T> + Default + Copy,
        {
            fn mul_assign(&mut self, rhs: T) {
                self.$field1 = self.$field1 * rhs;
                self.$field2 = self.$field2 * rhs;
            }
        }

        impl<T> Mul<T> for &$type<T>
        where
            T: Mul<Output = T> + Default + Copy,
        {
            type Output = $type<T>;

            fn mul(self, rhs: T) -> Self::Output {
                $type {
                    $field1: self.$field1 * rhs,
                    $field2: self.$field2 * rhs,
                }
            }
        }
    };
}

impl_ops!(Extent, width, height);
impl_ops!(FlexExtent, main, cross);
