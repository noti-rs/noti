use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4, PI},
    slice::ChunksExact,
    str::Chars,
};

use anyhow::Context;
use shared::value::TryFromValue;

/// Represents a drawable color or paint that can be applied to widgets.
///
/// Unlike a simple RGBA value, `Color` can represent complex paint
/// types such as linear gradients. This allows flexible and visually
/// rich rendering while keeping a single generic type for all widgets.
#[derive(Debug, Clone)]
pub enum Color {
    /// A linear gradient with angle, vector, and computed color stops.
    LinearGradient(LinearGradient),

    /// A solid fill color represented as BGRA floating-point values.
    Fill(Bgra<f32>),
}

impl Color {
    /// Returns `true` if the color is effectively transparent.
    ///
    /// Used by containers and widgets to skip unnecessary drawing passes
    /// when no visible paint would be produced (e.g., transparent fill).
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

impl Default for Color {
    fn default() -> Self {
        Color::Fill(Bgra::default())
    }
}

// TODO: implement correct algorithm of parsing the color
impl TryFromValue for Color {}

/// Describes a linear gradient, including direction and computed
/// color-stop information for rendering.
///
/// This is a fully resolved gradient — all additional parameters
/// needed for drawing (like gradient vector and per-color segment
/// size) are already precomputed from user configuration.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    /// The gradient’s angle in degrees, used to determine orientation.
    pub angle: f32,

    /// A normalized direction vector derived from `angle`, used to
    /// compute color transitions along the gradient.
    pub grad_vector: [f32; 2],

    /// The sequence of colors forming the gradient stops.
    pub colors: Vec<Bgra<f32>>,

    /// The length of each gradient segment, used to space colors
    /// evenly or proportionally along the gradient line.
    pub segment_per_color: f32,
}

impl LinearGradient {
    /// 3π/4
    const FRAC_3_PI_4: f32 = FRAC_PI_2 + FRAC_PI_4;

    pub fn new(mut angle: i16, mut colors: Vec<Bgra<u8>>) -> Self {
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

/// Represents a color in BGRA channel order, using a generic component type.
///
/// This type serves as an adapter between configuration-level RGBA
/// colors (typically simple integers) and rendering backends like
/// Cairo or Skia, which expect normalized floating-point channels.
///
/// The BGRA order is chosen because many graphics backends (including
/// Cairo’s `ARgb32` format) and GPU pipelines store colors in this
/// channel order on little-endian systems, allowing direct memory
/// mapping without conversion overhead.
#[derive(Debug, Clone, Copy, Default)]
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

impl Bgra<u8> {
    fn pre_mul_alpha(self) -> Self {
        if self.alpha == 255 {
            return self;
        }

        let alpha = self.alpha as f32 / 255.0;
        Self {
            red: (self.red as f32 * alpha) as u8,
            green: (self.green as f32 * alpha) as u8,
            blue: (self.blue as f32 * alpha) as u8,
            alpha: self.alpha,
        }
    }
}

impl From<Bgra<u8>> for Bgra<f32> {
    fn from(value: Bgra<u8>) -> Self {
        Self {
            blue: value.blue as f32 / 255.0,
            green: value.green as f32 / 255.0,
            red: value.red as f32 / 255.0,
            alpha: value.alpha as f32 / 255.0,
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
        <Bgra<u8> as TryFrom<_>>::try_from(value.clone())
            .map(Into::into)
            .map_err(|_| shared::error::ConversionError::InvalidValue {
                expected: "#RGB, #RRGGBB or #RRGGBBAA",
                actual: value,
            })
    }
}

impl TryFromValue for Bgra<u8> {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        <Self as TryFrom<_>>::try_from(value.clone()).map_err(|_| {
            shared::error::ConversionError::InvalidValue {
                expected: "#RGB, #RRGGBB or #RRGGBBAA",
                actual: value,
            }
        })
    }
}
impl TryFrom<String> for Bgra<u8> {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        const BASE: u32 = 16;

        if value.len() == 4 {
            let mut chars = value.chars();
            chars.next(); // Skip the hashtag
            let next_digit = |chars: &mut Chars| -> Option<u8> {
                let digit = chars.next()?.to_digit(BASE)? as u8;
                Some(digit * BASE as u8 + digit)
            };

            const ERR_MSG: &str = "Expected valid HEX digit";
            Ok(Bgra {
                red: next_digit(&mut chars).with_context(|| ERR_MSG)?,
                green: next_digit(&mut chars).with_context(|| ERR_MSG)?,
                blue: next_digit(&mut chars).with_context(|| ERR_MSG)?,
                alpha: 255,
            })
        } else {
            let mut data = value.as_bytes()[1..].chunks_exact(2);

            fn next_slice<'a>(data: &'a mut ChunksExact<u8>) -> Result<&'a str, anyhow::Error> {
                data.next()
                    .with_context(|| "Expected valid pair of HEX digits")
                    .and_then(|slice| {
                        std::str::from_utf8(slice).with_context(|| "Failed to parse color value")
                    })
            }

            Ok(Bgra {
                red: u8::from_str_radix(next_slice(&mut data)?, BASE)?,
                green: u8::from_str_radix(next_slice(&mut data)?, BASE)?,
                blue: u8::from_str_radix(next_slice(&mut data)?, BASE)?,
                alpha: if value[1..].len() == 8 {
                    u8::from_str_radix(next_slice(&mut data)?, BASE)?
                } else {
                    255
                },
            }
            .pre_mul_alpha())
        }
    }
}
