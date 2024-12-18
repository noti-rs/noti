use dbus::notification::Urgency;
use macros::ConfigProperty;
use serde::Deserialize;

use crate::{
    color::{Color, Rgba},
    public,
};

public! {
    #[derive(ConfigProperty, Debug)]
    #[cfg_prop(name(TomlTheme), derive(Debug, Deserialize, Default))]
    struct Theme {
        name: String,

        #[cfg_prop(use_type(TomlColors))]
        low: Colors,

        #[cfg_prop(use_type(TomlColors))]
        normal: Colors,

        #[cfg_prop(use_type(TomlColors), default(path = TomlColors::default_critical))]
        critical: Colors,
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
    #[derive(ConfigProperty, Debug)]
    #[cfg_prop(name(TomlColors), derive(Debug, Clone, Deserialize, Default))]
    struct Colors {
        #[cfg_prop(default(path = Rgba::new_black))]
        foreground: Rgba,
        #[cfg_prop(default(path = Color::new_rgba_white))]
        background: Color,

        #[cfg_prop(default(path = Color::new_rgba_black))]
        border: Color,
    }
}

impl TomlColors {
    fn default_critical() -> TomlColors {
        TomlColors {
            background: Some(Color::new_rgba_white()),
            foreground: Some(Rgba::new_red()),
            border: Some(Color::new_rgba_red()),
        }
    }
}
