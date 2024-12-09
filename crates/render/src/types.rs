use std::ops::{Add, AddAssign};

use config::spacing::Spacing;

#[derive(Debug, Default, Clone)]
pub struct RectSize {
    pub width: usize,
    pub height: usize,
}

impl RectSize {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    #[allow(dead_code)]
    pub fn new_width(width: usize) -> Self {
        Self { width, height: 0 }
    }

    #[allow(dead_code)]
    pub fn new_height(height: usize) -> Self {
        Self { width: 0, height }
    }

    pub fn shrink_by(&mut self, spacing: &Spacing) {
        self.width = self
            .width
            .saturating_sub(spacing.left() as usize + spacing.right() as usize);
        self.height = self
            .height
            .saturating_sub(spacing.top() as usize + spacing.bottom() as usize);
    }

    pub fn area(&self) -> usize {
        self.width * self.height
    }
}

impl Add<RectSize> for RectSize {
    type Output = RectSize;

    fn add(self, rhs: RectSize) -> Self::Output {
        Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl AddAssign<RectSize> for RectSize {
    fn add_assign(&mut self, rhs: RectSize) {
        self.width += rhs.width;
        self.height += rhs.height;
    }
}

#[derive(Debug, Default, Clone)]
pub struct Offset {
    pub x: usize,
    pub y: usize,
}

impl Offset {
    pub fn new(x: usize, y: usize) -> Self {
        Offset { x, y }
    }

    pub fn new_x(x: usize) -> Self {
        Offset { x, y: 0 }
    }

    pub fn new_y(y: usize) -> Self {
        Offset { x: 0, y }
    }

    pub fn no_offset() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Add<Offset> for Offset {
    type Output = Offset;

    fn add(self, rhs: Offset) -> Self::Output {
        Offset {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign<Offset> for Offset {
    fn add_assign(&mut self, rhs: Offset) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl From<Spacing> for Offset {
    fn from(value: Spacing) -> Self {
        Offset {
            x: value.left() as usize,
            y: value.top() as usize,
        }
    }
}

impl From<&Spacing> for Offset {
    fn from(value: &Spacing) -> Self {
        Offset {
            x: value.left() as usize,
            y: value.top() as usize,
        }
    }
}
