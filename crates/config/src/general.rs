//! The module that contain the structure `GeneralConfig` which stores general config properties.
//!
//! With it the module also stores `TomlGeneralConfig` which can parse data from TOML data.

use macros::ConfigProperty;
use serde::Deserialize;

use crate::{public, sorting::Sorting};

public! {
    #[derive(ConfigProperty, Default, Debug, Deserialize, Clone)]
    #[cfg_prop(name(GeneralConfig), derive(Debug))]
    struct TomlGeneralConfig {
        font: Option<Font>,

        #[cfg_prop(default(300))]
        width: Option<u16>,
        #[cfg_prop(default(150))]
        height: Option<u16>,

        anchor: Option<Anchor>,
        offset: Option<(u8, u8)>,
        #[cfg_prop(default(10))]
        gap: Option<u8>,

        sorting: Option<Sorting>,
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
#[serde(from = "String")]
pub enum Anchor {
    Top,
    TopLeft,
    #[default]
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
    Left,
    Right,
}

impl Anchor {
    pub fn is_top(&self) -> bool {
        matches!(self, Anchor::Top | Anchor::TopLeft | Anchor::TopRight)
    }

    pub fn is_right(&self) -> bool {
        matches!(self, Anchor::TopRight | Anchor::BottomRight | Anchor::Right)
    }

    pub fn is_bottom(&self) -> bool {
        matches!(
            self,
            Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight
        )
    }

    pub fn is_left(&self) -> bool {
        matches!(self, Anchor::TopLeft | Anchor::BottomLeft | Anchor::Left)
    }
}

impl From<String> for Anchor {
    fn from(value: String) -> Self {
        match value.as_str() {
            "top" => Anchor::Top,
            "top-left" | "top left" => Anchor::TopLeft,
            "top-right" | "top right" => Anchor::TopRight,
            "bottom" => Anchor::Bottom,
            "bottom-left" | "bottom left" => Anchor::BottomLeft,
            "bottom-right" | "bottom right" => Anchor::BottomRight,
            "left" => Anchor::Left,
            "right" => Anchor::Right,
            other => panic!(
                "Invalid anchor option! There are possible values:\n\
                - \"top\"\n\
                - \"top-right\" or \"top right\"\n\
                - \"top-left\" or \"top left\"\n\
                - bottom\n\
                - \"bottom-right\" or \"bottom right\"\n\
                - \"bottom-left\" or \"bottom left\"\n\
                - left\n\
                - right\n\
                Used: {other}"
            ),
        }
    }
}
