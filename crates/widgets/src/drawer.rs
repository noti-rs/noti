use config::display::Border;
use log::warn;

use crate::{
    color::{Bgra, Color},
    types::{extent::Extent2D, offset::Offset},
    Draw,
};

/// A simple wrapper around [`skia_safe::Surface`] used as the main
/// drawing target for widgets.
///
/// This type exists to provide a consistent way to access the
/// underlying Skia surface, making rendering code easier to work with.
pub struct Drawer {
    pub(crate) surface: skia_safe::Surface,
}

impl Drawer {
    /// Creates a new [`Drawer`] from an existing Skia surface.
    pub fn use_surface(sk_surface: skia_safe::Surface) -> Self {
        Self {
            surface: sk_surface,
        }
    }

    pub(crate) fn draw_into_offscreen(
        &mut self,
        offset: &Offset<usize>,
        widget: &crate::Widget,
    ) -> skia_safe::Image {
        let image_info = self.surface.image_info();
        let mut offscreen = skia_safe::surfaces::raster(&image_info, None, None).unwrap();
        offscreen.canvas().clear(skia_safe::Color::TRANSPARENT);

        let mut offscreen_drawer = Drawer::use_surface(offscreen);
        widget.draw_with_offset(offset, &mut offscreen_drawer);
        offscreen_drawer.surface.image_snapshot()
    }

    /// Fills the container’s background before drawing children.
    ///
    /// If configured, the background will be filled with rounded corners.
    /// This is the first step of the `draw` routine, ensuring that child
    /// widgets are rendered on top of a consistent background.
    pub(crate) fn fill_background(
        &mut self,
        offset: Offset<f32>,
        extent: Extent2D<f32>,
        border: &Border,
        background_color: &Color,
    ) {
        let outer_radius = (border.radius as f32)
            .min(extent.width / 2.0)
            .min(extent.height / 2.0);
        let inner_radius = (outer_radius - border.size as f32).max(0.0);
        let difference = border.size as f32;

        let canvas = self.surface.canvas();

        let rounded_rect = skia_safe::RRect::new_rect_xy(
            skia_safe::Rect::from_xywh(
                offset.x + difference,
                offset.y + difference,
                extent.width - difference * 2.0,
                extent.height - difference * 2.0,
            ),
            inner_radius,
            inner_radius,
        );

        let mut paint = skia_safe::Paint::default();
        paint.use_color(background_color, offset, extent);
        paint.set_anti_alias(true);

        canvas.draw_rrect(rounded_rect, &paint);
    }

    /// Draws the container’s border after all children have been rendered.
    ///
    /// This is the final step of the `draw` routine, allowing the border
    /// to visually wrap around both the background and the children.
    pub(crate) fn outline_border(
        &mut self,
        offset: Offset<f32>,
        extent: Extent2D<f32>,
        border: &Border,
        border_color: &Color,
    ) {
        if border.size == 0 {
            return;
        }

        let outer_radius = (border.radius as f32)
            .min(extent.width / 2.0)
            .min(extent.height / 2.0);
        let inner_radius = outer_radius - border.size as f32;

        let mut path = skia_safe::Path::new();

        path.add_rrect(
            skia_safe::RRect::new_rect_xy(
                skia_safe::Rect::from_xywh(offset.x, offset.y, extent.width, extent.height),
                outer_radius,
                outer_radius,
            ),
            None,
        );

        let border_size = border.size as f32;
        let base_rect = skia_safe::Rect::from_xywh(
            offset.x + border_size,
            offset.y + border_size,
            extent.width - border_size * 2.0,
            extent.height - border_size * 2.0,
        );
        if inner_radius <= 0.0 {
            path.add_rect(base_rect, None);
        } else {
            path.add_rrect(
                skia_safe::RRect::new_rect_xy(base_rect, inner_radius, inner_radius),
                None,
            );
        }

        let mut paint = skia_safe::Paint::default();
        paint.use_color(border_color, offset, extent);
        paint.set_anti_alias(true);

        path.set_fill_type(skia_safe::path::FillType::EvenOdd);
        self.surface.canvas().draw_path(&path, &paint);
    }
}

/// A helper trait for applying a [`Color`] to Skia paint objects.
///
/// This trait abstracts the conversion of high-level color types
/// (solid fills, gradients, etc.) into Skia's low-level representation,
/// making it easy to consistently apply colors across drawing code.
pub trait UseColor {
    fn use_color(&mut self, color: &Color, offset: Offset<f32>, frame_size: Extent2D<f32>);
}

impl UseColor for skia_safe::Paint {
    fn use_color(&mut self, color: &Color, offset: Offset<f32>, frame_size: Extent2D<f32>) {
        self.set_anti_alias(true);

        match color {
            Color::LinearGradient(linear_gradient) => {
                fn dot_product(vec1: [f32; 2], vec2: [f32; 2]) -> f32 {
                    vec1[0] * vec2[0] + vec1[1] * vec2[1]
                }

                let (half_width, half_height) = (frame_size.width / 2.0, frame_size.height / 2.0);

                // INFO: need to find a factor to multiply the x/y offsets to that distance where
                // prependicular line hits top left and top right corners. Without it part of area
                // will be filled by single non-gradientary color that is not acceptable.
                let x_offset = linear_gradient.grad_vector[0] * half_width;
                let y_offset = linear_gradient.grad_vector[1] * half_height;
                let norm = (x_offset * x_offset + y_offset * y_offset).sqrt();
                let factor =
                    dot_product([x_offset, y_offset], [half_width, half_height]) / (norm * norm);

                let start = skia_safe::Point::new(
                    offset.x + half_width - x_offset * factor,
                    offset.y + half_height + y_offset * factor,
                );
                let end = skia_safe::Point::new(
                    offset.x + half_width + x_offset * factor,
                    offset.y + half_height - y_offset * factor,
                );

                let mut colors: Vec<skia_safe::Color> = vec![];
                let mut offset = 0.0;
                let mut positions = vec![];
                for bgra in &linear_gradient.colors {
                    let bgra: Bgra<u8> = (*bgra).into();
                    colors.push(skia_safe::Color::from_argb(
                        bgra.alpha, bgra.red, bgra.green, bgra.blue,
                    ));
                    positions.push(offset);
                    offset += linear_gradient.segment_per_color;
                }

                let Some(shader) = skia_safe::Shader::linear_gradient(
                    (start, end),
                    skia_safe::gradient_shader::GradientShaderColors::Colors(&colors),
                    &*positions,
                    skia_safe::TileMode::Clamp,
                    skia_safe::gradient_shader::Flags::empty(),
                    None,
                ) else {
                    warn!("Failed to make gradient");
                    return;
                };

                self.set_shader(shader);
            }

            Color::Fill(bgra) => {
                self.set_color4f(
                    skia_safe::Color4f::new(bgra.red, bgra.green, bgra.blue, bgra.alpha),
                    None,
                );
            }
        }
    }
}
