use macros::ConfigProperty;
use serde::Deserialize;

use super::{public, Spacing};

public! {
    #[derive(ConfigProperty, Debug, Clone)]
    #[cfg_prop(name(TomlTextProperty), derive(Debug, Clone, Default, Deserialize))]
    struct TextProperty {
        font: Font,

        #[cfg_prop(default(true))]
        wrap: bool,

        style: TextStyle,

        margin: Spacing,

        alignment: TextAlignment,

        #[cfg_prop(default(14))]
        font_size: u8,

        #[cfg_prop(default(0))]
        line_spacing: u8,
    }
}

impl Default for TextProperty {
    fn default() -> Self {
        TomlTextProperty::default().into()
    }
}

public! {
    #[derive(Debug, Deserialize, Clone)]
    #[serde(from = "String")]
    struct Font {
        name: String,
    }
}

impl From<String> for Font {
    fn from(name: String) -> Self {
        Font { name }
    }
}

impl Default for Font {
    fn default() -> Self {
        Font {
            name: "Noto Sans".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum TextStyle {
    #[default]
    #[serde(rename = "regular")]
    Regular,
    #[serde(rename = "bold")]
    Bold,
    #[serde(rename = "italic")]
    Italic,
    #[serde(rename = "bold italic")]
    BoldItalic,
}

impl From<TextStyle> for widgets::widget::FontStyle {
    fn from(value: TextStyle) -> Self {
        match value {
            TextStyle::Regular => Self::Regular,
            TextStyle::Bold => Self::Bold,
            TextStyle::Italic => Self::Italic,
            TextStyle::BoldItalic => Self::BoldItalic,
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum TextAlignment {
    #[serde(rename = "center")]
    Center,
    #[default]
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "justify")]
    Justify,
}

impl From<TextAlignment> for widgets::widget::TextAlignment {
    fn from(value: TextAlignment) -> Self {
        match value {
            TextAlignment::Center => Self::Center,
            TextAlignment::Left => Self::Left,
            TextAlignment::Right => Self::Right,
            TextAlignment::Justify => Self::Justify,
        }
    }
}

impl TomlTextProperty {
    pub(super) fn default_summary() -> Self {
        Self {
            style: Some(TextStyle::Bold),
            alignment: Some(TextAlignment::Center),
            ..Default::default()
        }
    }
}
