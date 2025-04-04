use std::f64::consts::{FRAC_PI_2, PI};

use pangocairo::cairo::{Context, ImageSurface, LinearGradient};

use crate::{
    color::Color,
    types::{Offset, RectSize},
};

pub struct Drawer {
    pub(crate) surface: ImageSurface,
    pub(crate) context: Context,
}

impl Drawer {
    pub fn create(size: RectSize<usize>) -> pangocairo::cairo::Result<Self> {
        let surface = ImageSurface::create(
            pangocairo::cairo::Format::ARgb32,
            size.width as i32,
            size.height as i32,
        )?;

        let context = Context::new(&surface)?;

        Ok(Self { surface, context })
    }
}

pub(crate) trait MakeRounding {
    fn make_rounding(
        &self,
        offset: Offset<f64>,
        rect_size: RectSize<f64>,
        outer_radius: f64,
        inner_radius: f64,
    );
}

impl MakeRounding for Context {
    fn make_rounding(
        &self,
        offset: Offset<f64>,
        rect_size: RectSize<f64>,
        mut outer_radius: f64,
        mut inner_radius: f64,
    ) {
        debug_assert!(outer_radius >= inner_radius);
        let minimal_threshold = (rect_size.height / 2.0).min(rect_size.width / 2.0);
        inner_radius = inner_radius.min(minimal_threshold);
        outer_radius = outer_radius.min(minimal_threshold);

        self.arc(
            offset.x + outer_radius,
            offset.y + outer_radius,
            inner_radius,
            PI,
            -FRAC_PI_2,
        );
        self.arc(
            offset.x + rect_size.width - outer_radius,
            offset.y + outer_radius,
            inner_radius,
            -FRAC_PI_2,
            0.0,
        );
        self.arc(
            offset.x + rect_size.width - outer_radius,
            offset.y + rect_size.height - outer_radius,
            inner_radius,
            0.0,
            FRAC_PI_2,
        );
        self.arc(
            offset.x + outer_radius,
            offset.y + rect_size.height - outer_radius,
            inner_radius,
            FRAC_PI_2,
            PI,
        );
    }
}

pub(crate) trait SetSourceColor {
    fn set_source_color(
        &self,
        color: &Color,
        frame_size: RectSize<f64>,
    ) -> pangocairo::cairo::Result<()>;
}

impl SetSourceColor for Drawer {
    fn set_source_color(
        &self,
        color: &Color,
        frame_size: RectSize<f64>,
    ) -> pangocairo::cairo::Result<()> {
        match color {
            Color::LinearGradient(linear_gradient) => {
                fn dot_product(vec1: [f64; 2], vec2: [f64; 2]) -> f64 {
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

                let gradient = LinearGradient::new(
                    half_width - x_offset * factor,
                    half_height + y_offset * factor,
                    half_width + x_offset * factor,
                    half_height - y_offset * factor,
                );

                let mut offset = 0.0;
                for bgra in &linear_gradient.colors {
                    gradient
                        .add_color_stop_rgba(offset, bgra.red, bgra.green, bgra.blue, bgra.alpha);
                    offset += linear_gradient.segment_per_color;
                }

                self.context.set_source(gradient)?;
            }
            Color::Fill(bgra) => self
                .context
                .set_source_rgba(bgra.red, bgra.green, bgra.blue, bgra.alpha),
        }

        Ok(())
    }
}

impl TryFrom<Drawer> for Vec<u8> {
    type Error = pangocairo::cairo::BorrowError;

    fn try_from(value: Drawer) -> Result<Self, Self::Error> {
        let Drawer {
            surface, context, ..
        } = value;
        drop(context);
        Ok(surface.take_data()?.to_vec())
    }
}
