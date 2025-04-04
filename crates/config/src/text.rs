use macros::{ConfigProperty, GenericBuilder};
use serde::Deserialize;
use shared::value::TryFromValue;

use super::{public, Spacing};

public! {
    #[derive(ConfigProperty, GenericBuilder, Debug, Clone)]
    #[cfg_prop(name(TomlTextProperty), derive(Debug, Clone, Default, Deserialize))]
    #[gbuilder(name(GBuilderTextProperty), derive(Clone))]
    struct TextProperty {
        #[cfg_prop(default(true))]
        #[gbuilder(default(true))]
        wrap: bool,

        #[gbuilder(default)]
        wrap_mode: WrapMode,

        #[gbuilder(default)]
        ellipsize: Ellipsize,

        #[gbuilder(default)]
        style: TextStyle,

        #[gbuilder(default)]
        margin: Spacing,

        #[gbuilder(default)]
        alignment: TextAlignment,

        #[gbuilder(default(false))]
        justify: bool,

        #[cfg_prop(default(12))]
        font_size: u8,

        #[cfg_prop(default(0))]
        #[gbuilder(default(0))]
        line_spacing: u8,
    }
}

impl Default for TextProperty {
    fn default() -> Self {
        TomlTextProperty::default().into()
    }
}

impl TryFromValue for TextProperty {}

#[derive(Debug, Clone, Default, Deserialize)]
pub enum WrapMode {
    #[serde(rename = "word")]
    Word,
    #[serde(rename = "word-char")]
    #[default]
    WordChar,
    #[serde(rename = "char")]
    Char,
}

impl TryFromValue for WrapMode {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "word" => WrapMode::Word,
            "char" => WrapMode::Char,
            "word-char" | "word_char" => WrapMode::WordChar,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "word, char, word-char or word_char",
                actual: value,
            })?,
        })
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

impl TryFromValue for TextStyle {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "regular" => TextStyle::Regular,
            "bold" => TextStyle::Bold,
            "italic" => TextStyle::Italic,
            "bold-italic" | "bold_italic" => TextStyle::BoldItalic,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "regular, bold, italic, bold-italic or bold_italic",
                actual: value,
            })?,
        })
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
}

impl TryFromValue for TextAlignment {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "center" => TextAlignment::Center,
            "left" => TextAlignment::Left,
            "right" => TextAlignment::Right,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "center, left or right",
                actual: value,
            })?,
        })
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

#[derive(Debug, Deserialize, Default, Clone)]
pub enum Ellipsize {
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "middle")]
    Middle,
    #[default]
    #[serde(rename = "end")]
    End,
    #[serde(rename = "none")]
    None,
}

impl TryFromValue for Ellipsize {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "middle" => Ellipsize::Middle,
            "end" => Ellipsize::End,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "middle or end",
                actual: value,
            })?,
        })
    }
}
