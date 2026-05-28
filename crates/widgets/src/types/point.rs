use std::ops::{Add, AddAssign};

#[derive(Default, Debug, Clone, Copy)]
pub struct Point<T>
where
    T: Default + Copy,
{
    pub x: T,
    pub y: T,
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
