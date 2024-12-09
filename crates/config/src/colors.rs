use std::str::Chars;

use dbus::notification::Urgency;
use macros::ConfigProperty;
use serde::Deserialize;
use shared::value::TryDowncast;

use super::public;

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(UrgencyColors), derive(Debug))]
    struct TomlUrgencyColors {
        #[cfg_prop(use_type(Colors), mergeable)]
        low: Option<TomlColors>,

        #[cfg_prop(use_type(Colors), mergeable)]
        normal: Option<TomlColors>,

        #[cfg_prop(use_type(Colors), mergeable, default(TomlColors::default_critical()))]
        critical: Option<TomlColors>,
    }
}

impl UrgencyColors {
    pub fn by_urgency(&self, urgency: &Urgency) -> &Colors {
        match urgency {
            Urgency::Low => &self.low,
            Urgency::Normal => &self.normal,
            Urgency::Critical => &self.critical,
        }
    }
}

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(Colors), derive(Debug))]
    struct TomlColors {
        #[cfg_prop(default(Color::new_white()))]
        background: Option<Color>,
        #[cfg_prop(default(Color::new_black()))]
        foreground: Option<Color>,
    }
}

impl TomlColors {
    fn default_critical() -> TomlColors {
        TomlColors {
            background: Some(Color::new_white()),
            foreground: Some(Color::new_red()),
        }
    }
}

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

impl TryFrom<shared::value::Value> for Color {
    type Error = shared::error::ConversionError;
    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::String(str) => Ok(str.into()),
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
    }
}
