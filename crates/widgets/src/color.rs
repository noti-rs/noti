use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use config::color::{Color as CfgColor, LinearGradient as CfgLinearGradient, Rgba as CfgRgba};
use shared::value::TryFromValue;

#[derive(Clone)]
pub enum Color {
    LinearGradient(LinearGradient),
    Fill(Bgra<f32>),
}

impl Color {
    pub fn is_transparent(&self) -> bool {
        match self {
            Color::LinearGradient(linear_gradient) => {
                linear_gradient.colors.iter().all(Bgra::is_transparent)
            }
            Color::Fill(bgra) => bgra.is_transparent(),
        }
    }
}

impl From<LinearGradient> for Color {
    fn from(value: LinearGradient) -> Self {
        Color::LinearGradient(value)
    }
}

impl From<Bgra<f32>> for Color {
    fn from(value: Bgra<f32>) -> Self {
        Color::Fill(value)
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
        Color::Fill(Bgra::default())
    }
}

impl TryFromValue for Color {}

#[derive(Clone)]
pub struct LinearGradient {
    pub angle: f32,
    pub grad_vector: [f32; 2],
    pub colors: Vec<Bgra<f32>>,
    pub segment_per_color: f32,
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

        let segment_per_color = 1.0 / (colors.len() - 1) as f32;

        Self {
            angle,
            grad_vector,
            colors: colors.into_iter().map(Bgra::from).collect(),
            segment_per_color,
        }
    }
}

impl From<CfgLinearGradient> for LinearGradient {
    fn from(value: CfgLinearGradient) -> Self {
        LinearGradient::new(value.degree, value.colors)
    }
}

/// The RGBA color representation by ARGB format in little endian.
///
/// The struct was made in as adapter between config Rgba color and cairo's ARgb32 format because
/// the first one is very simple and the second one is complex and accepts only floats.
#[derive(Clone, Copy, Default)]
pub struct Bgra<T>
where
    T: Copy + Default,
{
    pub blue: T,
    pub green: T,
    pub red: T,
    pub alpha: T,
}

impl Bgra<f32> {
    pub fn is_transparent(&self) -> bool {
        self.alpha == 0.0
    }
}

impl From<&CfgRgba> for Bgra<f32> {
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

impl From<CfgRgba> for Bgra<f32> {
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

impl From<CfgRgba> for Bgra<u8> {
    fn from(
        CfgRgba {
            red,
            green,
            blue,
            alpha,
        }: CfgRgba,
    ) -> Self {
        Bgra {
            blue,
            green,
            red,
            alpha,
        }
    }
}

impl From<Bgra<f32>> for Bgra<u8> {
    fn from(value: Bgra<f32>) -> Self {
        Self {
            blue: (value.blue * u8::MAX as f32).round() as u8,
            green: (value.green * u8::MAX as f32).round() as u8,
            red: (value.red * u8::MAX as f32).round() as u8,
            alpha: (value.alpha * u8::MAX as f32).round() as u8,
        }
    }
}

impl TryFromValue for Bgra<f32> {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        <CfgRgba as TryFrom<_>>::try_from(value.clone())
            .map(Into::into)
            .map_err(|_| shared::error::ConversionError::InvalidValue {
                expected: "#RGB, #RRGGBB or #RRGGBBAA",
                actual: value,
            })
    }
}
