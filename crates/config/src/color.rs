use std::{slice::ChunksExact, str::Chars};

use anyhow::{bail, Context};
use serde::Deserialize;
use shared::value::TryFromValue;

use super::public;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Color {
    Rgba(Rgba),
    LinearGradient(LinearGradient),
}

impl Color {
    pub(super) fn new_rgba_white() -> Self {
        Color::Rgba(Rgba::new_white())
    }

    pub(super) fn new_rgba_black() -> Self {
        Color::Rgba(Rgba::new_black())
    }

    pub(super) fn new_rgba_red() -> Self {
        Color::Rgba(Rgba::new_red())
    }
}

impl From<Rgba> for Color {
    fn from(value: Rgba) -> Self {
        Color::Rgba(value)
    }
}

impl From<LinearGradient> for Color {
    fn from(value: LinearGradient) -> Self {
        Color::LinearGradient(value)
    }
}

public! {
    #[derive(Debug, Clone, Deserialize, Default)]
    #[serde(try_from = "String")]
    struct Rgba {
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
    }
}

impl Rgba {
    pub(super) fn new_white() -> Self {
        Self {
            red: 255,
            green: 255,
            blue: 255,
            alpha: 255,
        }
    }

    #[allow(unused)]
    pub(super) fn new_black() -> Self {
        Self {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255,
        }
    }

    pub(super) fn new_red() -> Self {
        Self {
            red: 255,
            green: 0,
            blue: 0,
            alpha: 255,
        }
    }

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

impl TryFrom<String> for Rgba {
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
            Ok(Rgba {
                red: next_digit(&mut chars).with_context(|| ERR_MSG)?,
                green: next_digit(&mut chars).with_context(|| ERR_MSG)?,
                blue: next_digit(&mut chars).with_context(|| ERR_MSG)?,
                alpha: 255,
            })
        } else {
            let mut data = value[1..].as_bytes().chunks_exact(2);

            fn next_slice<'a>(data: &'a mut ChunksExact<u8>) -> Result<&'a str, anyhow::Error> {
                data.next()
                    .with_context(|| "Expected valid pair of HEX digits")
                    .and_then(|slice| {
                        std::str::from_utf8(slice).with_context(|| "Failed to parse color value")
                    })
            }

            Ok(Rgba {
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

impl TryFromValue for Rgba {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        value
            .clone()
            .try_into()
            .map_err(|_| shared::error::ConversionError::InvalidValue {
                expected: "#RGB, #RRGGBB or #RRGGBBAA",
                actual: value,
            })
    }
}

public! {
    #[derive(Debug, Clone, Deserialize)]
    #[serde(try_from = "Vec<String>")]
    struct LinearGradient {
        angle: u16,
        colors: Vec<Rgba>,
    }
}

impl TryFrom<Vec<String>> for LinearGradient {
    type Error = anyhow::Error;

    fn try_from(mut value: Vec<String>) -> Result<Self, Self::Error> {
        if value.len() < 2 {
            bail!("The length of array must be greater than 2");
        }

        let degree_str = value.remove(0);
        if !degree_str.len() < 4 || &degree_str[degree_str.len() - 3..] != "deg" {
            bail!("The first element of array should be a degree. For example, '15deg'")
        }

        let mut degree = degree_str[..degree_str.len() - 3].parse::<i16>()?;
        if degree < 0 {
            degree += ((degree / 360) + 1) * 360;
        }

        Ok(Self {
            angle: degree as u16,
            colors: value
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<Rgba>, _>>()?,
        })
    }
}
