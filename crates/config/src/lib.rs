use macros::ConfigProperty;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path, sync::Mutex};

pub mod colors;
pub mod sorting;
pub mod spacing;
pub mod text;
pub mod watcher;

use colors::{Color, TomlUrgencyColors, UrgencyColors};
use sorting::Sorting;
use spacing::Spacing;
use text::{TextProperty, TomlTextProperty};
use watcher::{ConfigState, ConfigWatcher};

pub static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::init()));

#[derive(Debug)]
pub struct Config {
    watcher: ConfigWatcher,
    general: GeneralConfig,
    display: DisplayConfig,

    app_configs: HashMap<String, DisplayConfig>,
}

impl Config {
    pub fn init() -> Self {
        let watcher = ConfigWatcher::init().expect("The config watcher must be initialized");

        let (general, display, app_configs) = Self::parse(watcher.get_config_path());

        Self {
            watcher,
            general,
            display,
            app_configs,
        }
    }

    pub fn general(&self) -> &GeneralConfig {
        &self.general
    }

    #[allow(unused)]
    pub fn default_display(&self) -> &DisplayConfig {
        &self.display
    }

    pub fn display_by_app(&self, name: &str) -> &DisplayConfig {
        self.app_configs.get(name).unwrap_or(&self.display)
    }

    pub fn check_updates(&mut self) -> ConfigState {
        self.watcher.check_updates()
    }

    pub fn update(&mut self) {
        let (general, display, app_configs) = Self::parse(self.watcher.get_config_path());

        self.general = general;
        self.display = display;
        self.app_configs = app_configs;
    }

    fn parse(
        path: Option<&Path>,
    ) -> (GeneralConfig, DisplayConfig, HashMap<String, DisplayConfig>) {
        let TomlConfig {
            general,
            display,
            apps,
        } = TomlConfig::parse(path);

        let mut app_configs =
            HashMap::with_capacity(apps.as_ref().map(|data| data.len()).unwrap_or(0));

        if let Some(apps) = apps {
            for mut app in apps {
                app = app.merge(display.as_ref());
                app_configs.insert(app.name, app.display.unwrap().unwrap_or_default());
            }
        }

        (
            general.unwrap_or_default().into(),
            display.unwrap_or_default().into(),
            app_configs,
        )
    }
}

#[macro_export]
macro_rules! public {
    ($(#[$($attr:tt)+])* struct $name:ident { $($(#[$($field_attr:tt)+])* $field:ident: $ftype:ty,)* }) => {
        $(#[$($attr)+])*
        pub struct $name {
            $(
                $(#[$($field_attr)+])*
                pub $field: $ftype,
            )*
        }
    };
}

#[derive(Debug, Deserialize, Default)]
pub struct TomlConfig {
    general: Option<TomlGeneralConfig>,
    display: Option<TomlDisplayConfig>,

    #[serde(rename(deserialize = "app"))]
    apps: Option<Vec<AppConfig>>,
}

impl TomlConfig {
    fn parse(path: Option<&Path>) -> Self {
        path.map(|config_path| fs::read_to_string(&config_path).unwrap())
            .map(|content| match toml::from_str(&content) {
                Ok(content) => Some(content),
                Err(error) => {
                    eprintln!("{error}");
                    None
                }
            })
            .flatten()
            .unwrap_or(Default::default())
    }
}

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
    #[serde(from = "(String, u8)")]
    struct Font {
        name: String,
        size: u8,
    }
}

impl From<(String, u8)> for Font {
    fn from((name, size): (String, u8)) -> Self {
        Font { name, size }
    }
}

impl Default for Font {
    fn default() -> Self {
        Font {
            name: "Noto Sans".to_string(),
            size: 12,
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
        match self {
            Anchor::Top | Anchor::TopLeft | Anchor::TopRight => true,
            _ => false,
        }
    }

    pub fn is_right(&self) -> bool {
        match self {
            Anchor::TopRight | Anchor::BottomRight | Anchor::Right => true,
            _ => false,
        }
    }

    pub fn is_bottom(&self) -> bool {
        match self {
            Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight => true,
            _ => false,
        }
    }

    pub fn is_left(&self) -> bool {
        match self {
            Anchor::TopLeft | Anchor::BottomLeft | Anchor::Left => true,
            _ => false,
        }
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
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(DisplayConfig), derive(Debug))]
    struct TomlDisplayConfig {
        #[cfg_prop(use_type(ImageProperty))]
        image: Option<TomlImageProperty>,

        padding: Option<Spacing>,
        #[cfg_prop(use_type(Border))]
        border: Option<TomlBorder>,

        #[cfg_prop(use_type(UrgencyColors))]
        colors: Option<TomlUrgencyColors>,

        #[cfg_prop(temporary)]
        text: Option<TomlTextProperty>,

        #[cfg_prop(inherits(field = text), use_type(TextProperty), default(TomlTextProperty::default_title()))]
        title: Option<TomlTextProperty>,

        #[cfg_prop(inherits(field = text), use_type(TextProperty))]
        body: Option<TomlTextProperty>,

        #[cfg_prop(default(true))]
        markup: Option<bool>,

        #[cfg_prop(default(0))]
        timeout: Option<u16>,
    }
}

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(ImageProperty), derive(Debug, Clone))]
    struct TomlImageProperty {
        #[cfg_prop(default(64))]
        max_size: Option<u16>,
        margin: Option<Spacing>,
        resizing_method: Option<ResizingMethod>,
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum ResizingMethod {
    #[serde(rename = "nearest")]
    Nearest,
    #[serde(rename = "triangle")]
    Triangle,
    #[serde(rename = "catmull-rom")]
    CatmullRom,
    #[default]
    #[serde(rename = "gaussian")]
    Gaussian,
    #[serde(rename = "lanczos3")]
    Lanczos3,
}

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(Border), derive(Debug))]
    struct TomlBorder {
        #[cfg_prop(default(0))]
        size: Option<u8>,
        #[cfg_prop(default(0))]
        radius: Option<u8>,
        #[cfg_prop(default(Color::new_black()))]
        color: Option<Color>,
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    pub name: String,
    pub display: Option<TomlDisplayConfig>,
}

impl AppConfig {
    fn merge(mut self, other: Option<&TomlDisplayConfig>) -> Self {
        self.display = self.display.or(other.cloned());
        self
    }
}
