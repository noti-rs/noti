use std::str::Chars;

use serde::Deserialize;
use shared::value::TryFromValue;

use super::public;

public! {
    #[derive(Debug, Clone, Deserialize, Default)]
    #[serde(from = "String")]
    struct Color {
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
    }
}

impl Color {
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

impl From<String> for Color {
    fn from(value: String) -> Self {
        const BASE: u32 = 16;

        if value.len() == 4 {
            let mut chars = value.chars();
            chars.next(); // Skip the hashtag
            let next_digit = |chars: &mut Chars| {
                let digit = chars.next().unwrap().to_digit(BASE).unwrap() as u8;
                digit * BASE as u8 + digit
            };

            Color {
                red: next_digit(&mut chars),
                green: next_digit(&mut chars),
                blue: next_digit(&mut chars),
                alpha: 255,
            }
        } else {
            let data = &value[1..];
            Color {
                red: u8::from_str_radix(&data[0..2], BASE).unwrap(),
                green: u8::from_str_radix(&data[2..4], BASE).unwrap(),
                blue: u8::from_str_radix(&data[4..6], BASE).unwrap(),
                alpha: if data.len() == 8 {
                    u8::from_str_radix(&data[6..8], BASE).unwrap()
                } else {
                    255
                },
            }
            .pre_mul_alpha()
        }
    }
}

impl TryFromValue for Color {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(value.into())
    }
}
