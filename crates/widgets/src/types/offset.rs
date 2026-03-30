use std::ops::{Add, AddAssign};

use super::spacing::Spacing;

/// A set of instructions that tells a widget where to sit on the screen.
///
/// This struct describes a shift away from the **top-left corner**.
/// Without this information, a widget would not know where to start
/// drawing itself and might end up in the wrong place.
///
/// It stores two values to help widgets find their home:
/// * **x:** How many steps to move to the **right** from the left edge.
/// * **y:** How many steps to move **down** from the top edge.
///
/// **Important:** This is not a "Point" on a map. Instead, think of it
/// as a "Shift." It tells the system how far to move away from a
/// corner before it starts working.
#[derive(Debug, Default, Clone, Copy)]
pub struct Offset<T>
where
    T: Add<Output = T> + Default + Copy,
{
    pub x: T,
    pub y: T,
}

impl<T> Offset<T>
where
    T: Add<Output = T> + Default + Copy,
{
    pub fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    pub fn new_x(x: T) -> Self {
        Self {
            x,
            ..Default::default()
        }
    }

    pub fn new_y(y: T) -> Self {
        Self {
            y,
            ..Default::default()
        }
    }

    pub fn no_offset() -> Self {
        Self::default()
    }
}

impl<T> Add<Offset<T>> for Offset<T>
where
    T: Add<Output = T> + Default + Copy,
{
    type Output = Offset<T>;

    fn add(self, rhs: Offset<T>) -> Self::Output {
        Offset {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T> AddAssign<Offset<T>> for Offset<T>
where
    T: Add<Output = T> + AddAssign<T> + Default + Copy,
{
    fn add_assign(&mut self, rhs: Offset<T>) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl From<&Offset<usize>> for Offset<f32> {
    fn from(value: &Offset<usize>) -> Self {
        Self {
            x: value.x as f32,
            y: value.y as f32,
        }
    }
}

impl From<Offset<usize>> for Offset<f32> {
    fn from(value: Offset<usize>) -> Self {
        Self {
            x: value.x as f32,
            y: value.y as f32,
        }
    }
}

impl From<Spacing> for Offset<usize> {
    fn from(value: Spacing) -> Self {
        Offset {
            x: value.left,
            y: value.top,
        }
    }
}

impl From<&Spacing> for Offset<usize> {
    fn from(value: &Spacing) -> Self {
        Offset {
            x: value.left,
            y: value.top,
        }
    }
}
