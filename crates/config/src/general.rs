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
        anchor: Anchor,
        offset: (u8, u8),
        #[cfg_prop(default(10))]
        gap: u8,

        #[cfg_prop(use_type(DebugTomlConfig), mergeable)]
        debug: DebugConfig,

        sorting: Sorting,

        #[cfg_prop(default(0))]
        limit: u8,

        #[cfg_prop(default(TimeDuration::default_idle_threshold()))]
        idle_threshold: TimeDuration,

        #[cfg_prop(default(TimeDuration::default_sound_cooldown()))]
        sound_cooldown: TimeDuration,
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(from = "String")]
pub struct TimeDuration(u32);

impl TimeDuration {
    fn default_idle_threshold() -> Self {
        Self(
            humantime::parse_duration("5 min")
                .expect("The default duration must be valid")
                .as_millis() as u32,
        )
    }

    fn default_sound_cooldown() -> Self {
        Self(
            humantime::parse_duration("300ms")
                .expect("The default sound cooldown duration must be valid")
                .as_millis() as u32,
        )
    }
}

impl From<String> for TimeDuration {
    fn from(duration_str: String) -> Self {
        if duration_str.to_lowercase() == "none" {
            return Self(0);
        }

        humantime::parse_duration(&duration_str)
            .map(|duration| Self(duration.as_millis() as u32))
            .unwrap_or_else(|_| Self(0))
    }
}

impl std::ops::Deref for TimeDuration {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
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

public! {
    #[derive(ConfigProperty, Debug)]
    #[cfg_prop(name(DebugTomlConfig), derive(Debug, Default, Deserialize, Clone))]
    struct DebugConfig {
        /// Shows bounds of each UI component. There's two outlines:
        /// + Dashed Magenta — provided extent for UI component, may be bigger than actual content
        /// + Solid Green — actual extent of UI component
        ///
        /// Dashed Magenta and Solid Green may matches at some or all sides. And for this Solid
        /// Green is slightly transparent to show the Dashed Magenta below it.
        show_layout_bounds: bool,
    }
}
