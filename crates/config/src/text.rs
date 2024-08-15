use macros::ConfigProperty;
use serde::Deserialize;

use super::{public, Spacing};

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(TextProperty), derive(Debug, Clone))]
    struct TomlTextProperty {
        #[cfg_prop(default(true))]
        wrap: Option<bool>,
        ellipsize_at: Option<EllipsizeAt>,

        style: Option<TextStyle>,

        margin: Option<Spacing>,
        justification: Option<TextJustification>,
        #[cfg_prop(default(0))]
        line_spacing: Option<u8>,
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
