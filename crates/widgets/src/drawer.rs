use log::warn;

use crate::{
    color::{Bgra, Color},
    types::{Offset, RectSize},
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
}

/// A helper trait for applying a [`Color`] to Skia paint objects.
///
/// This trait abstracts the conversion of high-level color types
/// (solid fills, gradients, etc.) into Skia's low-level representation,
/// making it easy to consistently apply colors across drawing code.
pub trait UseColor {
    fn use_color(&mut self, color: &Color, offset: Offset<f32>, frame_size: RectSize<f32>);
}

impl UseColor for skia_safe::Paint {
    fn use_color(&mut self, color: &Color, offset: Offset<f32>, frame_size: RectSize<f32>) {
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
