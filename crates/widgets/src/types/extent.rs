use std::{
    cmp::Ordering,
    ops::{Add, AddAssign, Mul, Sub},
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
#[derive(Debug, Default, Clone, Copy)]
pub struct Extent2D<T>
where
    T: Default + Copy,
{
    pub width: T,
    pub height: T,
}

impl<T> Extent2D<T>
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
}

impl<T> Extent2D<T>
where
    T: Default + Copy + PartialOrd + Sub<Output = T> + num_traits::FromPrimitive,
{
    pub fn shrink_by(&mut self, spacing: &Spacing) {
        let mut width = self.width - T::from_usize(spacing.horizontal()).unwrap_or_default();
        if let Some(Ordering::Less) = width.partial_cmp(&Default::default()) {
            width = Default::default();
        }
        self.width = width;

        let mut height = self.height - T::from_usize(spacing.horizontal()).unwrap_or_default();
        if let Some(Ordering::Less) = height.partial_cmp(&Default::default()) {
            height = Default::default();
        }
        self.height = height;
    }
}

impl From<Extent2D<usize>> for Extent2D<f32> {
    fn from(value: Extent2D<usize>) -> Self {
        Self {
            width: value.width as f32,
            height: value.height as f32,
        }
    }
}

impl From<Extent2D<f32>> for Extent2D<usize> {
    fn from(value: Extent2D<f32>) -> Self {
        Self {
            width: value.width.round() as usize,
            height: value.height.round() as usize,
        }
    }
}

impl<T> Add<Extent2D<T>> for Extent2D<T>
where
    T: Add<Output = T> + Default + Copy,
{
    type Output = Extent2D<T>;

    fn add(self, rhs: Extent2D<T>) -> Self::Output {
        Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl<T> AddAssign<Extent2D<T>> for Extent2D<T>
where
    T: AddAssign<T> + Default + Copy,
{
    fn add_assign(&mut self, rhs: Extent2D<T>) {
        self.width += rhs.width;
        self.height += rhs.height;
    }
}
