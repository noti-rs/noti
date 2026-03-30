use macros::GenericBuilder;

use crate::types::Color;

/// Defines the visual stroke and corner shaping for a widget's boundary.
///
/// This struct groups the properties required to draw a frame around a
/// container. It manages how thick the frame is, how rounded the
/// corners appear, and the specific color of the stroke.
#[derive(GenericBuilder, Debug, Clone, Default)]
#[gbuilder(name(BorderGBuilder), derive(Clone))]
pub struct Border {
    /// The thickness of the border line.
    ///
    /// This defines the width of the stroke drawn at the edge of the
    /// widget's boundary. A size of 0 effectively disables the border.
    pub size: usize,

    /// The radius used to create rounded corners for the boundary.
    ///
    /// This determines the degree of curvature for all four corners
    /// of the widget. A higher value creates a more circular appearance,
    /// while 0 results in sharp, rectangular corners.
    pub radius: usize,

    /// The color applied to the border stroke.
    ///
    /// Similar to a foreground color, this defines the "ink" used to
    /// draw the border line. It is drawn on top of or around the
    /// `background_color` of the container to provide visual separation.
    pub color: Color,
}
