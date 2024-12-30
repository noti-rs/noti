use std::ops::{Add, AddAssign, Mul};

use config::spacing::Spacing;

#[derive(Debug, Default, Clone, Copy)]
pub struct RectSize<T>
where
    T: Default + Copy,
{
    pub width: T,
    pub height: T,
}

impl<T> RectSize<T>
where
    T: Default + Copy,
{
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
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
}

impl RectSize<usize> {
    pub fn shrink_by(&mut self, spacing: &Spacing) {
        self.width = self
            .width
            .saturating_sub(spacing.left() as usize + spacing.right() as usize);
        self.height = self
            .height
            .saturating_sub(spacing.top() as usize + spacing.bottom() as usize);
    }
}

impl From<RectSize<usize>> for RectSize<f64> {
    fn from(value: RectSize<usize>) -> Self {
        Self {
            width: value.width as f64,
            height: value.height as f64,
        }
    }
}

impl<T> Add<RectSize<T>> for RectSize<T>
where
    T: Add<Output = T> + Default + Copy,
{
    type Output = RectSize<T>;

    fn add(self, rhs: RectSize<T>) -> Self::Output {
        Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl<T> AddAssign<RectSize<T>> for RectSize<T>
where
    T: AddAssign<T> + Default + Copy,
{
    fn add_assign(&mut self, rhs: RectSize<T>) {
        self.width += rhs.width;
        self.height += rhs.height;
    }
}

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

impl From<Offset<usize>> for Offset<f64> {
    fn from(value: Offset<usize>) -> Self {
        Self {
            x: value.x as f64,
            y: value.y as f64,
        }
    }
}

impl From<Spacing> for Offset<usize> {
    fn from(value: Spacing) -> Self {
        Offset {
            x: value.left() as usize,
            y: value.top() as usize,
        }
    }
}

impl From<&Spacing> for Offset<usize> {
    fn from(value: &Spacing) -> Self {
        Offset {
            x: value.left() as usize,
            y: value.top() as usize,
        }
    }
}
