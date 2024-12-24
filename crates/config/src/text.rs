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
        ellipsize_at: EllipsizeAt,

        #[gbuilder(default)]
        style: TextStyle,

        #[gbuilder(default)]
        margin: Spacing,

        #[gbuilder(default)]
        justification: TextJustification,

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
pub enum TextJustification {
    #[serde(rename = "center")]
    Center,
    #[default]
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "space-between")]
    SpaceBetween,
}

impl TryFromValue for TextJustification {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "center" => TextJustification::Center,
            "left" => TextJustification::Left,
            "right" => TextJustification::Right,
            "space-between" | "space_between" => TextJustification::SpaceBetween,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "center, left, right, space-between or space_between",
                actual: value,
            })?,
        })
    }
}

impl TomlTextProperty {
    pub(super) fn default_title() -> Self {
        Self {
            style: Some(TextStyle::Bold),
            justification: Some(TextJustification::Center),
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum EllipsizeAt {
    #[serde(rename = "middle")]
    Middle,
    #[default]
    #[serde(rename = "end")]
    End,
}

impl TryFromValue for EllipsizeAt {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "middle" => EllipsizeAt::Middle,
            "end" => EllipsizeAt::End,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "middle or end",
                actual: value,
            })?,
        })
    }
}
