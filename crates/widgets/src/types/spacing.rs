use std::ops::{Add, AddAssign};

use macros::GenericBuilder;
use shared::value::TryFromValue;

use crate::types::Extent;

/// Defines the internal buffer zone between a widget's boundary and its content.
///
/// Think of `Spacing` as a way to "shrink" the available room inside a box.
/// It pushes the content away from the edges by a specific amount on
/// each side.
///
/// While the outer size of the widget stays the same, the space where
/// the content is allowed to live gets smaller based on these four
/// measurements. This ensures that text, images, or nested widgets
/// don't touch the very edge of their container.
#[derive(GenericBuilder, Debug, Default, Clone, Copy)]
#[gbuilder(name(SpacingGBuilder), derive(Clone), constructor)]
pub struct Spacing {
    /// The gap pushed down from the top edge.
    #[gbuilder(default(0), aliases(vertical, all))]
    pub top: usize,

    /// The gap pushed in from the right edge.
    #[gbuilder(default(0), aliases(horizontal, all))]
    pub right: usize,

    /// The gap pushed up from the bottom edge.
    #[gbuilder(default(0), aliases(vertical, all))]
    pub bottom: usize,

    /// The gap pushed in from the left edge.
    #[gbuilder(default(0), aliases(horizontal, all))]
    pub left: usize,
}

impl TryFromValue for Spacing {}

impl Spacing {
    /// Creates a new `Spacing` where the same value is applied to all four sides.
    ///
    /// This is the quickest way to create a uniform buffer around your content
    /// when you don't need independent control over specific edges.
    pub fn all_directional(val: usize) -> Self {
        Self {
            top: val,
            bottom: val,
            right: val,
            left: val,
        }
    }

    /// Creates a new `Spacing` using separate values for the vertical and
    /// horizontal axes.
    ///
    /// The first value sets both the top and bottom, while the second value
    /// sets both the left and right. This follows a pattern similar to
    /// CSS shorthand for defining symmetric spacing.
    pub fn cross(vertical: usize, horizontal: usize) -> Self {
        Self {
            top: vertical,
            bottom: vertical,
            right: horizontal,
            left: horizontal,
        }
    }

    /// Returns the total combined thickness of the left and right spacing.
    ///
    /// This is useful during layout calculations to determine how much
    /// total width is being "consumed" by the horizontal buffer.
    pub fn horizontal(&self) -> usize {
        self.left + self.right
    }

    /// Returns the total combined thickness of the top and bottom spacing.
    ///
    /// This is useful during layout calculations to determine how much
    /// total height is being "consumed" by the vertical buffer.
    pub fn vertical(&self) -> usize {
        self.top + self.bottom
    }
}

impl Add<Spacing> for Spacing {
    type Output = Spacing;

    fn add(self, rhs: Spacing) -> Self::Output {
        Spacing {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl Add<Spacing> for &Spacing {
    type Output = Spacing;

    fn add(self, rhs: Spacing) -> Self::Output {
        Spacing {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl AddAssign<Spacing> for Spacing {
    fn add_assign(&mut self, rhs: Spacing) {
        self.top += rhs.top;
        self.right += rhs.right;
        self.bottom += rhs.bottom;
        self.left += rhs.left;
    }
}

impl<T> From<Spacing> for Extent<T>
where
    T: Default + Copy + num_traits::FromPrimitive,
{
    fn from(value: Spacing) -> Self {
        Self {
            width: T::from_usize(value.horizontal()).unwrap_or_default(),
            height: T::from_usize(value.vertical()).unwrap_or_default(),
        }
    }
}

impl<T> From<&Spacing> for Extent<T>
where
    T: Default + Copy + num_traits::FromPrimitive,
{
    fn from(value: &Spacing) -> Self {
        Self {
            width: T::from_usize(value.horizontal()).unwrap_or_default(),
            height: T::from_usize(value.vertical()).unwrap_or_default(),
        }
    }
}
