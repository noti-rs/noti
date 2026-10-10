use serde::Deserialize;

use super::public;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum Color {
    LinearGradient(LinearGradient),
    #[serde(untagged)]
    Rgba(Rgba),
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
    #[derive(Debug, Clone, Copy, Deserialize, Default)]
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
}

impl TryFrom<String> for Rgba {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        <widgets::types::Bgra<u8> as TryFrom<_>>::try_from(value).map(Into::into)
    }
}

public! {
    #[derive(Debug, Clone, Deserialize)]
    struct LinearGradient {
        degree: i16,
        colors: Vec<Rgba>,
    }
}

impl From<Color> for widgets::types::Color {
    fn from(value: Color) -> Self {
        match value {
            Color::Rgba(rgba) => widgets::types::Bgra::from(rgba).into(),
            Color::LinearGradient(linear_gradient) => {
                widgets::types::LinearGradient::from(linear_gradient).into()
            }
        }
    }
}

impl From<LinearGradient> for widgets::types::LinearGradient {
    fn from(value: LinearGradient) -> Self {
        widgets::types::LinearGradient::new(
            value.degree,
            value
                .colors
                .into_iter()
                .map(widgets::types::Bgra::from)
                .collect(),
        )
    }
}

impl From<widgets::types::Bgra<u8>> for Rgba {
    fn from(value: widgets::types::Bgra<u8>) -> Self {
        Self {
            red: value.red,
            blue: value.blue,
            green: value.green,
            alpha: value.alpha,
        }
    }
}

impl From<Rgba> for widgets::types::Bgra<u8> {
    fn from(value: Rgba) -> Self {
        Self {
            red: value.red,
            blue: value.blue,
            green: value.green,
            alpha: value.alpha,
        }
    }
}

impl From<Rgba> for widgets::types::Bgra<f32> {
    fn from(value: Rgba) -> Self {
        Self {
            red: value.red as f32 / 255.0,
            blue: value.blue as f32 / 255.0,
            green: value.green as f32 / 255.0,
            alpha: value.alpha as f32 / 255.0,
        }
    }
}

impl From<&Rgba> for widgets::types::Bgra<u8> {
    fn from(value: &Rgba) -> Self {
        Self {
            red: value.red,
            blue: value.blue,
            green: value.green,
            alpha: value.alpha,
        }
    }
}

impl From<&Rgba> for widgets::types::Bgra<f32> {
    fn from(value: &Rgba) -> Self {
        Self {
            red: value.red as f32 / 255.0,
            blue: value.blue as f32 / 255.0,
            green: value.green as f32 / 255.0,
            alpha: value.alpha as f32 / 255.0,
        }
    }
}
