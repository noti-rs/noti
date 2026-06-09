use log::warn;

use crate::{
    context::{AnimationQuery, GetDebugOptions, LoadExtent},
    types::{extent::Extent, offset::Offset, Bgra, Color, WidgetId},
    widget::{Widget, WidgetBase},
};

pub(crate) trait DrawContext<T>:
    LoadExtent<T, WidgetId> + AnimationQuery<WidgetId> + GetDebugOptions
where
    T: Default + Copy,
{
}

impl<C, T> DrawContext<T> for C
where
    C: LoadExtent<T, WidgetId> + AnimationQuery<WidgetId> + GetDebugOptions,
    T: Default + Copy,
{
}

pub(crate) trait Draw<C, T>: WidgetBase
where
    C: DrawContext<T>,
    T: Default + Copy + Into<f32>,
{
    fn draw(&self, context: &C, offset: &Offset<T>, drawer: &mut Drawer) {
        let Some(provided_extent) = <C as LoadExtent<T, WidgetId>>::load(context, self.get_id())
        else {
            warn!(
                "{} widget with id {} didn't measured! Refused to draw.",
                self.get_type(),
                *self.get_id()
            );
            return;
        };

        self.draw_content(context, offset, provided_extent, drawer);

        if context.get_debug_options().show_layout_bounds {
            let mut paint = skia_safe::Paint::default();
            paint.set_anti_alias(true);

            paint.set_color4f(skia_safe::Color4f::new(0.95, 0.23, 0.99, 1.0), None);
            paint.set_style(skia_safe::PaintStyle::Stroke);
            paint.set_stroke_width(2.0);

            if let Some(dash) = skia_safe::PathEffect::dash(&[6.0, 6.0], 0.0) {
                paint.set_path_effect(dash);
            }

            drawer.surface.canvas().draw_rect(
                skia_safe::Rect::from_xywh(
                    offset.x.into(),
                    offset.y.into(),
                    provided_extent.width.into(),
                    provided_extent.height.into(),
                ),
                &paint,
            );
        }
    }

    fn draw_content(
        &self,
        context: &C,
        offset: &Offset<T>,
        provided_extent: Extent<T>,
        drawer: &mut Drawer,
    );
}

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

    pub(crate) fn draw_into_offscreen<C>(
        &mut self,
        context: &C,
        offset: &Offset<f32>,
        widget: &Widget,
    ) -> skia_safe::Image
    where
        C: DrawContext<f32>,
    {
        let image_info = self.surface.image_info();
        let mut offscreen = skia_safe::surfaces::raster(&image_info, None, None).unwrap();
        offscreen.canvas().clear(skia_safe::Color::TRANSPARENT);

        let mut offscreen_drawer = Drawer::use_surface(offscreen);
        widget.draw(context, offset, &mut offscreen_drawer);
        offscreen_drawer.surface.image_snapshot()
    }
}

/// A helper trait for applying a [`Color`] to Skia paint objects.
///
/// This trait abstracts the conversion of high-level color types
/// (solid fills, gradients, etc.) into Skia's low-level representation,
/// making it easy to consistently apply colors across drawing code.
pub trait UseColor {
    fn use_color(&mut self, color: &Color, offset: Offset<f32>, extent: Extent<f32>);
}

impl UseColor for skia_safe::Paint {
    fn use_color(&mut self, color: &Color, offset: Offset<f32>, extent: Extent<f32>) {
        self.set_anti_alias(true);

        match color {
            Color::LinearGradient(linear_gradient) => {
                fn dot_product(vec1: [f32; 2], vec2: [f32; 2]) -> f32 {
                    vec1[0] * vec2[0] + vec1[1] * vec2[1]
                }

                let (half_width, half_height) = (extent.width / 2.0, extent.height / 2.0);

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

pub(crate) fn draw_debug_bounds(
    canvas: &skia_safe::Canvas,
    offset: Offset<f32>,
    extent: Extent<f32>,
) {
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);

    paint.set_style(skia_safe::PaintStyle::Stroke);
    paint.set_stroke_width(2.0);
    paint.set_color4f(skia_safe::Color4f::new(0.53, 0.97, 0.48, 0.7), None);
    paint.set_path_effect(None);

    canvas.draw_rect(
        skia_safe::Rect::from_xywh(offset.x, offset.y, extent.width, extent.height),
        &paint,
    );
}
