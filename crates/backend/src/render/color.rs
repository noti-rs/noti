use std::ops::{Mul, MulAssign};

use config::colors::Color;

use super::widget::Coverage;

#[derive(Clone, Default)]
pub(crate) struct Bgra {
    pub(crate) blue: f32,
    pub(crate) green: f32,
    pub(crate) red: f32,
    pub(crate) alpha: f32,
}

impl Bgra {
    pub(crate) fn new() -> Self {
        Self {
            blue: 0.0,
            green: 0.0,
            red: 0.0,
            alpha: 0.0,
        }
    }

    #[allow(unused)]
    pub(crate) fn new_black() -> Self {
        Self {
            blue: 0.0,
            green: 0.0,
            red: 0.0,
            alpha: 1.0,
        }
    }

    #[allow(unused)]
    pub(crate) fn new_white() -> Self {
        Self {
            blue: 1.0,
            green: 1.0,
            red: 1.0,
            alpha: 1.0,
        }
    }

    pub(crate) fn into_rgba(self) -> Rgba {
        self.into()
    }

    pub(crate) fn into_slice(self) -> [u8; 4] {
        [
            (self.blue * 255.0).round() as u8,
            (self.green * 255.0).round() as u8,
            (self.red * 255.0).round() as u8,
            (self.alpha * 255.0).round() as u8,
        ]
    }
}

impl From<&[u8; 4]> for Bgra {
    fn from(value: &[u8; 4]) -> Self {
        Self {
            blue: value[0] as f32 / 255.0,
            green: value[1] as f32 / 255.0,
            red: value[2] as f32 / 255.0,
            alpha: value[3] as f32 / 255.0,
        }
    }
}

impl From<&[f32; 4]> for Bgra {
    fn from(value: &[f32; 4]) -> Self {
        Self {
            blue: value[0],
            green: value[1],
            red: value[2],
            alpha: value[3],
        }
    }
}

impl From<&Color> for Bgra {
    fn from(
        &Color {
            red,
            green,
            blue,
            alpha,
        }: &Color,
    ) -> Self {
        Bgra {
            blue: blue as f32 / 255.0,
            green: green as f32 / 255.0,
            red: red as f32 / 255.0,
            alpha: alpha as f32 / 255.0,
        }
    }
}

impl From<Bgra> for [u8; 4] {
    fn from(value: Bgra) -> Self {
        value.into_slice()
    }
}

impl From<Bgra> for Rgba {
    fn from(value: Bgra) -> Self {
        let Bgra {
            blue,
            green,
            red,
            alpha,
        } = value;
        Rgba {
            red,
            green,
            blue,
            alpha,
        }
    }
}

impl Mul<f32> for Bgra {
    type Output = Bgra;

    fn mul(mut self, rhs: f32) -> Self::Output {
        self.blue *= rhs;
        self.green *= rhs;
        self.red *= rhs;
        self.alpha *= rhs;
        self
    }
}

impl Mul<Coverage> for Bgra {
    type Output = Bgra;

    fn mul(self, Coverage(val): Coverage) -> Self::Output {
        self * val
    }
}

impl MulAssign<f32> for Bgra {
    fn mul_assign(&mut self, rhs: f32) {
        self.blue *= rhs;
        self.green *= rhs;
        self.red *= rhs;
        self.alpha *= rhs;
    }
}

impl MulAssign<Coverage> for Bgra {
    fn mul_assign(&mut self, Coverage(val): Coverage) {
        *self *= val
    }
}

pub(crate) struct Rgba {
    pub(crate) red: f32,
    pub(crate) green: f32,
    pub(crate) blue: f32,
    pub(crate) alpha: f32,
}

impl Rgba {
    pub(crate) fn new() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        }
    }

    #[allow(unused)]
    pub(crate) fn new_white() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }

    pub(crate) fn into_bgra(self) -> Bgra {
        self.into()
    }

    pub(crate) fn into_slice(self) -> [u8; 4] {
        [
            (self.red * 255.0).round() as u8,
            (self.green * 255.0).round() as u8,
            (self.blue * 255.0).round() as u8,
            (self.alpha * 255.0).round() as u8,
        ]
    }
}

impl From<&[u8; 3]> for Rgba {
    fn from(value: &[u8; 3]) -> Self {
        Self {
            red: value[0] as f32 / 255.0,
            green: value[1] as f32 / 255.0,
            blue: value[2] as f32 / 255.0,
            alpha: 1.0,
        }
    }
}

impl From<&[u8; 4]> for Rgba {
    fn from(value: &[u8; 4]) -> Self {
        Self {
            red: value[0] as f32 / 255.0,
            green: value[1] as f32 / 255.0,
            blue: value[2] as f32 / 255.0,
            alpha: value[3] as f32 / 255.0,
        }
    }
}

impl From<&[f32; 4]> for Rgba {
    fn from(value: &[f32; 4]) -> Self {
        Self {
            red: value[0],
            green: value[1],
            blue: value[2],
            alpha: value[3],
        }
    }
}

impl From<Rgba> for Bgra {
    fn from(value: Rgba) -> Self {
        let Rgba {
            red,
            green,
            blue,
            alpha,
        } = value;
        Bgra {
            blue,
            green,
            red,
            alpha,
        }
    }
}

impl From<Rgba> for [u8; 4] {
    fn from(value: Rgba) -> Self {
        value.into_slice()
    }
}

impl Mul<f32> for Rgba {
    type Output = Rgba;

    fn mul(mut self, rhs: f32) -> Self::Output {
        self.blue *= rhs;
        self.green *= rhs;
        self.red *= rhs;
        self.alpha *= rhs;
        self
    }
}

impl MulAssign<f32> for Rgba {
    fn mul_assign(&mut self, rhs: f32) {
        self.blue *= rhs;
        self.green *= rhs;
        self.red *= rhs;
        self.alpha *= rhs;
    }
}

// SOURCE: https://stackoverflow.com/questions/726549/algorithm-for-additive-color-mixing-for-rgb-values
// r.A = 1 - (1 - fg.A) * (1 - bg.A);
// if (r.A < 1.0e-6) return r; // Fully transparent -- R,G,B not important
// r.R = fg.R * fg.A / r.A + bg.R * bg.A * (1 - fg.A) / r.A;
// r.G = fg.G * fg.A / r.A + bg.G * bg.A * (1 - fg.A) / r.A;
// r.B = fg.B * fg.A / r.A + bg.B * bg.A * (1 - fg.A) / r.A;
macro_rules! overlay_on {
    ($($type:path),+) => {
        $(impl $type {
            #[allow(unused)]
            pub(crate) fn linearly_interpolate(&self, dst: &Bgra, alpha: f32) -> Bgra {
                Bgra {
                    blue: self.blue * alpha + dst.blue * (1.0 - alpha),
                    green: self.green * alpha + dst.green * (1.0 - alpha),
                    red: self.red * alpha + dst.red * (1.0 - alpha),
                    alpha: self.alpha * alpha + dst.alpha * (1.0 - alpha),
                }
            }

            #[allow(unused)]
            pub(crate) fn overlay_on(self, background: &Self) -> Self {
                let mut new_color = Self::new();
                new_color.alpha = 1.0 - (1.0 - self.alpha) * (1.0 - background.alpha);
                if new_color.alpha < f32::EPSILON {
                    return new_color;
                }

                new_color.red = self.red * self.alpha / new_color.alpha
                    + background.red * background.alpha * (1.0 - self.alpha) / new_color.alpha;
                new_color.green = self.green * self.alpha / new_color.alpha
                    + background.green * background.alpha * (1.0 - self.alpha) / new_color.alpha;
                new_color.blue = self.blue * self.alpha / new_color.alpha
                    + background.blue * background.alpha * (1.0 - self.alpha) / new_color.alpha;
                new_color

            }
        })+
    };
}

overlay_on!(Bgra, Rgba);
