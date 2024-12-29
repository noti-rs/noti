use std::f64::consts::{FRAC_PI_2, PI};

use pangocairo::cairo::{Context, ImageSurface, LinearGradient};

use crate::{
    color::Color,
    types::{Offset, RectSize},
};

pub struct Drawer {
    surface: ImageSurface,
    pub context: Context,
    _size: RectSize,
}

impl Drawer {
    pub fn create(size: RectSize) -> anyhow::Result<Self> {
        let surface = ImageSurface::create(
            pangocairo::cairo::Format::ARgb32,
            size.width as i32,
            size.height as i32,
        )?;

        let context = Context::new(&surface)?;

        Ok(Self {
            surface,
            context,
            _size: size,
        })
    }
}

pub(crate) trait MakeRounding {
    fn make_rounding(
        &self,
        offset: Offset,
        rect_size: RectSize,
        outer_radius: f64,
        inner_radius: f64,
    );
}

impl MakeRounding for Drawer {
    fn make_rounding(
        &self,
        offset: Offset,
        rect_size: RectSize,
        outer_radius: f64,
        inner_radius: f64,
    ) {
        self.context.arc(
            offset.x as f64 + outer_radius,
            offset.y as f64 + outer_radius,
            inner_radius,
            PI,
            -FRAC_PI_2,
        );
        self.context.arc(
            offset.x as f64 + rect_size.width as f64 - outer_radius,
            offset.y as f64 + outer_radius,
            inner_radius,
            -FRAC_PI_2,
            0.0,
        );
        self.context.arc(
            offset.x as f64 + rect_size.width as f64 - outer_radius,
            offset.y as f64 + rect_size.height as f64 - outer_radius,
            inner_radius,
            0.0,
            FRAC_PI_2,
        );
        self.context.arc(
            offset.x as f64 + outer_radius,
            offset.y as f64 + rect_size.height as f64 - outer_radius,
            inner_radius,
            FRAC_PI_2,
            PI,
        );
    }
}

pub(crate) trait SetSourceColor {
    fn set_source_color(&self, color: &Color, frame_size: RectSize);
}

impl SetSourceColor for Drawer {
    fn set_source_color(&self, color: &Color, frame_size: RectSize) {
        match color {
            Color::LinearGradient(linear_gradient) => {
                let (half_width, half_height) = (
                    frame_size.width as f64 / 2.0,
                    frame_size.height as f64 / 2.0,
                );

                let aspect_ratio = half_width / half_height;
                let half_width_ratio = linear_gradient.grad_vector[0] as f64 * half_width
                    / if linear_gradient.grad_vector[0] < 1.0 {
                        aspect_ratio
                    } else {
                        1.0
                    };

                let half_height_ratio = linear_gradient.grad_vector[1] as f64
                    * half_height
                    * if linear_gradient.grad_vector[1] < 1.0 {
                        aspect_ratio
                    } else {
                        1.0
                    };

                let gradient = LinearGradient::new(
                    half_width - half_width_ratio,
                    half_height + half_height_ratio,
                    half_width + half_width_ratio,
                    half_height - half_height_ratio,
                );

                let mut offset = 0.0;
                let incrementor = 1.0 / (linear_gradient.colors.len() - 1) as f64;
                for bgra in &linear_gradient.colors {
                    gradient.add_color_stop_rgba(
                        offset,
                        bgra.red as f64,
                        bgra.green as f64,
                        bgra.blue as f64,
                        bgra.alpha as f64,
                    );
                    offset += incrementor;
                }

                self.context.set_source(gradient).unwrap();
            }
            Color::Fill(bgra) => self.context.set_source_rgba(
                bgra.red as f64,
                bgra.green as f64,
                bgra.blue as f64,
                bgra.alpha as f64,
            ),
        }
    }
}

impl From<Drawer> for Vec<u8> {
    fn from(value: Drawer) -> Self {
        let Drawer {
            surface, context, ..
        } = value;
        drop(context);
        surface.take_data().unwrap().to_vec()
    }
}
