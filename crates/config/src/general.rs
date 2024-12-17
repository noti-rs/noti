//! The module that contain the structure `GeneralConfig` which stores general config properties.
//!
//! With it the module also stores `TomlGeneralConfig` which can parse data from TOML data.

use macros::ConfigProperty;
use serde::Deserialize;

use crate::{public, sorting::Sorting};

public! {
    #[derive(ConfigProperty, Debug)]
    #[cfg_prop(name(TomlGeneralConfig), derive(Debug, Default, Deserialize, Clone))]
    struct GeneralConfig {
        font: Font,

        #[cfg_prop(default(300))]
        width: u16,
        #[cfg_prop(default(150))]
        height: u16,

        anchor: Anchor,
        offset: (u8, u8),
        #[cfg_prop(default(10))]
        gap: u8,

        sorting: Sorting,

        #[cfg_prop(default(0))]
        limit: u8,

        idle_threshold: IdleThreshold,
    }
}

public! {
    #[derive(Debug, Deserialize, Clone)]
    #[serde(from = "String")]
    struct IdleThreshold {
        duration: u32,
    }
}

impl From<String> for IdleThreshold {
    fn from(duration_str: String) -> Self {
        humantime::parse_duration(&duration_str)
            .map(|duration| IdleThreshold {
                duration: duration.as_millis() as u32,
            })
            .unwrap_or_default()
    }
}

impl Default for IdleThreshold {
    fn default() -> Self {
        IdleThreshold {
            duration: humantime::parse_duration("5 min")
                .expect("The default duration must be valid")
                .as_millis() as u32,
        }
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
