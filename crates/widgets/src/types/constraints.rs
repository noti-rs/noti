use crate::types::Extent2D;

pub enum SizingMode {
    Fixed,
    Dynamic,
}

impl SizingMode {
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed)
    }
}

pub struct Constraints<T>
where
    T: Default + Copy,
{
    pub min: Extent2D<T>,
    pub max: Extent2D<T>,
}

impl<T> Constraints<T>
where
    T: Default + Copy,
{
    pub fn new_tight(width: T, height: T) -> Self {
        Self {
            min: Extent2D::new(width, height),
            max: Extent2D::new(width, height),
        }
    }

    pub fn new_soft(width: T, height: T) -> Self {
        Self {
            min: Extent2D::default(),
            max: Extent2D::new(width, height),
        }
    }

    pub fn from_extent_tight(extent: Extent2D<T>) -> Self {
        Self::new_tight(extent.width, extent.height)
    }

    pub fn from_extent_soft(extent: Extent2D<T>) -> Self {
        Self::new_soft(extent.width, extent.height)
    }
}
