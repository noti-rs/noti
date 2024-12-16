use macros::{ConfigProperty, GenericBuilder};
use serde::Deserialize;
use shared::value::TryDowncast;

use super::{public, Spacing};

public! {
    #[derive(ConfigProperty, GenericBuilder, Debug, Clone)]
    #[cfg_prop(name(TomlTextProperty),derive(Debug, Clone, Default, Deserialize))]
    #[gbuilder(name(GBuilderTextProperty))]
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

impl TryFrom<shared::value::Value> for TextProperty {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
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

impl TryFrom<shared::value::Value> for TextStyle {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "regular" => TextStyle::Regular,
                "bold" => TextStyle::Bold,
                "italic" => TextStyle::Italic,
                "bold-italic" | "bold_italic" => TextStyle::BoldItalic,
                _ => Err(shared::error::ConversionError::InvalidValue {
                    expected: "regular, bold, italic, bold-italic or bold_italic",
                    actual: str,
                })?,
            }),
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
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

impl TryFrom<shared::value::Value> for TextJustification {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "center" => TextJustification::Center,
                "left" => TextJustification::Left,
                "right" => TextJustification::Right,
                "space-between" | "space_between" => TextJustification::SpaceBetween,
                _ => Err(shared::error::ConversionError::InvalidValue {
                    expected: "center, left, right, space-between or space_between",
                    actual: str,
                })?,
            }),
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
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

impl TryFrom<shared::value::Value> for EllipsizeAt {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "middle" => EllipsizeAt::Middle,
                "end" => EllipsizeAt::End,
                _ => Err(shared::error::ConversionError::InvalidValue {
                    expected: "middle or end",
                    actual: str,
                })?,
            }),
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
    }
}
