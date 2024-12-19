use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4, PI},
    ops::{Mul, MulAssign},
};

use config::color::{Color as CfgColor, LinearGradient as CfgLinearGradient, Rgba as CfgRgba};
use shared::value::TryFromValue;

use super::widget::Coverage;

#[derive(Clone)]
pub enum Color {
    LinearGradient(LinearGradient),
    Single(Bgra),
}

impl Color {
    pub fn is_transparent(&self) -> bool {
        match self {
            Color::LinearGradient(linear_gradient) => {
                linear_gradient.colors.iter().all(Bgra::is_transparent)
            }
            Color::Single(bgra) => bgra.is_transparent(),
        }
    }
}

impl From<LinearGradient> for Color {
    fn from(value: LinearGradient) -> Self {
        Color::LinearGradient(value)
    }
}

impl From<Bgra> for Color {
    fn from(value: Bgra) -> Self {
        Color::Single(value)
    }
}

impl From<CfgColor> for Color {
    fn from(value: CfgColor) -> Self {
        match value {
            CfgColor::Rgba(rgba) => Bgra::from(rgba).into(),
            CfgColor::LinearGradient(linear_gradient) => {
                LinearGradient::from(linear_gradient).into()
            }
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::Single(Bgra::new())
    }
}

impl TryFromValue for Color {}

#[derive(Clone)]
pub struct LinearGradient {
    angle: f32,
    grad_vector: [f32; 2],
    doubled_norm: f32,
    colors: Vec<Bgra>,
    segment_per_color: f32,
}

impl LinearGradient {
    /// 3π/4
    const FRAC_3_PI_4: f32 = FRAC_PI_2 + FRAC_PI_4;

    pub fn new(mut angle: i16, mut colors: Vec<CfgRgba>) -> Self {
        if angle < 0 {
            angle += ((angle / 360) + 1) * 360;
        }

        if angle >= 360 {
            angle = angle - (angle / 360) * 360;
        }

        if angle >= 180 {
            colors.reverse();
            angle -= 180
        }

        let angle = (angle as f32).to_radians();

        let grad_vector = match angle {
            x @ 0.0..=FRAC_PI_4 => [1.0, x.tan()],
            x @ FRAC_PI_4..FRAC_PI_2 => [1.0 / x.tan(), 1.0],
            FRAC_PI_2 => [0.0, 1.0],
            x @ FRAC_PI_2..=Self::FRAC_3_PI_4 => [1.0 / x.tan(), 1.0],
            x @ Self::FRAC_3_PI_4..=PI => [-1.0, -x.tan()],
            _ => unreachable!(),
        };

        let norm = (grad_vector[0] * grad_vector[0] + grad_vector[1] * grad_vector[1]).sqrt();
        let doubled_norm = norm * norm;
        let segment_per_color = 1.0 / (colors.len() - 1) as f32;

        Self {
            angle,
            grad_vector,
            doubled_norm,
            colors: colors.into_iter().map(Bgra::from).collect(),
            segment_per_color,
        }
    }

    /// Returns a concrete color from square color space of linear gradient considering the angle.
    ///
    /// Note that the 'x' and 'y' values which you pass into function should be in range
    /// $0.0 <= x, y <= 1.0$
    /// The other values will be cause of incorrect color!
    ///
    /// To acheive it you can simply divide x or y position by frame width or hegiht respectively.
    pub fn color_at(&self, mut x: f32, y: f32) -> Bgra {
        if self.angle > FRAC_PI_2 {
            x -= 1.0
        }

        let mut position_on_grad_line = self.grad_vector.dot_product(&[x, y]) / self.doubled_norm;

        while position_on_grad_line > 1.0 {
            position_on_grad_line = 1.0 - position_on_grad_line;
        }

        while position_on_grad_line < 0.0 {
            position_on_grad_line += 1.0;
        }

        let left_color_index = (position_on_grad_line / self.segment_per_color).floor() as usize;

        if left_color_index == self.colors.len() - 1 {
            return self.colors[left_color_index];
        }

        let difference = (position_on_grad_line
            - (left_color_index as f32) * self.segment_per_color)
            / self.segment_per_color;

        self.colors[left_color_index + 1]
            .linearly_interpolate(&self.colors[left_color_index], difference)
    }
}

impl From<CfgLinearGradient> for LinearGradient {
    fn from(value: CfgLinearGradient) -> Self {
        LinearGradient::new(value.degree, value.colors)
    }
}

trait DotProduct<T>
where
    T: std::ops::Mul<Output = T> + std::ops::Add<Output = T>,
{
    fn dot_product(&self, other: &Self) -> T;
}

impl<T> DotProduct<T> for [T; 2]
where
    T: std::ops::Mul<Output = T> + std::ops::Add<Output = T> + Copy,
{
    fn dot_product(&self, other: &Self) -> T {
        self[0] * other[0] + self[1] * other[1]
    }
}

#[derive(Clone, Copy, Default)]
pub struct Bgra {
    pub blue: f32,
    pub green: f32,
    pub red: f32,
    pub alpha: f32,
}

impl Bgra {
    pub fn new() -> Self {
        Self {
            blue: 0.0,
            green: 0.0,
            red: 0.0,
            alpha: 0.0,
        }
    }

    #[allow(unused)]
    pub fn new_black() -> Self {
        Self {
            blue: 0.0,
            green: 0.0,
            red: 0.0,
            alpha: 1.0,
        }
    }

    #[allow(unused)]
    pub fn new_white() -> Self {
        Self {
            blue: 1.0,
            green: 1.0,
            red: 1.0,
            alpha: 1.0,
        }
    }

    pub fn is_transparent(&self) -> bool {
        self.alpha == 0.0
    }

    pub fn into_rgba(self) -> Rgba {
        self.into()
    }

    pub fn into_slice(self) -> [u8; 4] {
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

impl From<&CfgRgba> for Bgra {
    fn from(
        &CfgRgba {
            red,
            green,
            blue,
            alpha,
        }: &CfgRgba,
    ) -> Self {
        Bgra {
            blue: blue as f32 / 255.0,
            green: green as f32 / 255.0,
            red: red as f32 / 255.0,
            alpha: alpha as f32 / 255.0,
        }
    }
}

impl From<CfgRgba> for Bgra {
    fn from(
        CfgRgba {
            red,
            green,
            blue,
            alpha,
        }: CfgRgba,
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

impl TryFromValue for Bgra {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        <CfgRgba as TryFrom<_>>::try_from(value.clone())
            .map(Into::into)
            .map_err(|_| shared::error::ConversionError::InvalidValue {
                expected: "#RGB, #RRGGBB or #RRGGBBAA",
                actual: value,
            })
    }
}

pub struct Rgba {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Rgba {
    pub fn new() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        }
    }

    #[allow(unused)]
    pub fn new_white() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }

    pub fn into_bgra(self) -> Bgra {
        self.into()
    }

    pub fn into_slice(self) -> [u8; 4] {
        [
            (self.red * 255.0).round() as u8,
            (self.green * 255.0).round() as u8,
            (self.blue * 255.0).round() as u8,
            (self.alpha * 255.0).round() as u8,
        ]
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Self::new()
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
            pub fn linearly_interpolate(&self, dst: &Bgra, alpha: f32) -> Bgra {
                Bgra {
                    blue: self.blue * alpha + dst.blue * (1.0 - alpha),
                    green: self.green * alpha + dst.green * (1.0 - alpha),
                    red: self.red * alpha + dst.red * (1.0 - alpha),
                    alpha: self.alpha * alpha + dst.alpha * (1.0 - alpha),
                }
            }

            #[allow(unused)]
            pub fn overlay_on(self, background: &Self) -> Self {
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
