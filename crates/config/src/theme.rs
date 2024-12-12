use dbus::notification::Urgency;
use macros::ConfigProperty;
use serde::Deserialize;

use crate::{color::Color, public};

public! {
    #[derive(ConfigProperty, Deserialize, Default)]
    #[cfg_prop(name(Theme))]
    struct TomlTheme {
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
    #[derive(ConfigProperty, Clone, Default, Deserialize)]
    #[cfg_prop(name(Colors))]
    struct TomlColors {
        foreground: Option<Color>,
        background: Option<Color>,

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
