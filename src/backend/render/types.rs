use std::ops::{Add, AddAssign};

use crate::config::spacing::Spacing;

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
