use dbus::notification::Urgency;
use macros::ConfigProperty;
use serde::Deserialize;

use crate::{color::Color, public};

public! {
    #[derive(ConfigProperty, Deserialize, Default, Debug)]
    #[cfg_prop(name(Theme), derive(Debug))]
    struct TomlTheme {
        name: Option<String>,

        #[cfg_prop(use_type(Colors))]
        low: Option<TomlColors>,

        #[cfg_prop(use_type(Colors))]
        normal: Option<TomlColors>,

        #[cfg_prop(use_type(Colors), default(path = TomlColors::default_critical))]
        critical: Option<TomlColors>,
    }
}

impl Theme {
    pub fn by_urgency(&self, urgency: &Urgency) -> &Colors {
        match urgency {
            Urgency::Low => &self.low,
            Urgency::Normal => &self.normal,
            Urgency::Critical => &self.critical,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        TomlTheme::default().unwrap_or_default()
    }
}

public! {
    #[derive(ConfigProperty, Clone, Default, Deserialize, Debug)]
    #[cfg_prop(name(Colors), derive(Debug))]
    struct TomlColors {
        #[cfg_prop(default(path = Color::new_black))]
        foreground: Option<Color>,
        #[cfg_prop(default(path = Color::new_white))]
        background: Option<Color>,

        #[cfg_prop(default(path = Color::new_black))]
        border: Option<Color>,
    }
}

impl TomlColors {
    fn default_critical() -> TomlColors {
        TomlColors {
            background: Some(Color::new_white()),
            foreground: Some(Color::new_red()),
            border: Some(Color::new_red()),
        }
    }
}
