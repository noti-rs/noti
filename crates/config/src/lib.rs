use log::debug;
use macros::{ConfigProperty, GenericBuilder};
use serde::Deserialize;
use shared::{
    error::ConversionError,
    file_watcher::{FileState, FilesWatcher},
    value::TryDowncast,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub mod colors;
pub mod sorting;
pub mod spacing;
pub mod text;

use colors::{Color, TomlUrgencyColors, UrgencyColors};
use sorting::Sorting;
use spacing::Spacing;
use text::{TextProperty, TomlTextProperty};

const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const HOME: &str = "HOME";
const APP_NAME: &str = env!("APP_NAME");
const CONFIG_FILE: &str = "config.toml";

#[derive(Debug)]
pub struct Config {
    watcher: FilesWatcher,
    general: GeneralConfig,
    display: DisplayConfig,

    app_configs: HashMap<String, DisplayConfig>,
}

impl Config {
    pub fn init(user_config: Option<&str>) -> Self {
        fn to_config_file(prefix: String) -> PathBuf {
            let mut path_buf: PathBuf = prefix.into();
            path_buf.push(APP_NAME);
            path_buf.push(CONFIG_FILE);
            path_buf
        }

        let config_paths = [
            user_config.map(PathBuf::from),
            std::env::var(XDG_CONFIG_HOME).map(to_config_file).ok(),
            std::env::var(HOME)
                .map(|mut path| {
                    path.push_str("/.config");
                    path
                })
                .map(to_config_file)
                .ok(),
        ]
        .into_iter()
        .flatten()
        .collect();

        debug!("Config: Initializing");
        let watcher =
            FilesWatcher::init(config_paths).expect("The config watcher must be initialized");

        let (general, display, app_configs) = Self::parse(watcher.get_watching_path());
        debug!("Config: Initialized");

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

    pub fn displays(&self) -> impl Iterator<Item = &DisplayConfig> {
        vec![&self.display]
            .into_iter()
            .chain(self.app_configs.values())
    }

    pub fn check_updates(&mut self) -> FileState {
        self.watcher.check_updates()
    }

    pub fn update(&mut self) {
        let (general, display, app_configs) = Self::parse(self.watcher.get_watching_path());

        self.general = general;
        self.display = display;
        self.app_configs = app_configs;

        debug!("Config: Updated");
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

        debug!("Config: Parsed from files");

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
        path.map(|config_path| fs::read_to_string(config_path).unwrap())
            .and_then(|content| match toml::from_str(&content) {
                Ok(content) => Some(content),
                Err(error) => {
                    eprintln!("{error}");
                    None
                }
            })
            .unwrap_or_default()
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
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(DisplayConfig), derive(Debug))]
    struct TomlDisplayConfig {
        layout: Option<Layout>,

        #[cfg_prop(use_type(ImageProperty), mergeable)]
        image: Option<TomlImageProperty>,

        padding: Option<Spacing>,
        #[cfg_prop(use_type(Border), mergeable)]
        border: Option<TomlBorder>,

        #[cfg_prop(use_type(UrgencyColors), mergeable)]
        colors: Option<TomlUrgencyColors>,

        #[cfg_prop(temporary)]
        text: Option<TomlTextProperty>,

        #[cfg_prop(inherits(field = text), use_type(TextProperty), default(TomlTextProperty::default_title()), mergeable)]
        title: Option<TomlTextProperty>,

        #[cfg_prop(inherits(field = text), use_type(TextProperty), mergeable)]
        body: Option<TomlTextProperty>,

        #[cfg_prop(default(true))]
        markup: Option<bool>,

        #[cfg_prop(default(0))]
        timeout: Option<u16>,
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(from = "String")]
pub enum Layout {
    #[default]
    Default,
    FromPath {
        path_buf: PathBuf,
    },
}

impl Layout {
    pub fn is_default(&self) -> bool {
        matches!(self, Layout::Default)
    }
}

impl From<String> for Layout {
    fn from(value: String) -> Self {
        if value == "default" {
            return Layout::Default;
        }

        Layout::FromPath {
            path_buf: PathBuf::from(
                shellexpand::full(&value)
                    .map(|value| value.into_owned())
                    .unwrap_or(value),
            ),
        }
    }
}

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(
        name(ImageProperty),
        derive(GenericBuilder, Debug, Clone),
        attributes(#[gbuilder(name(GBuilderImageProperty))])
    )]
    struct TomlImageProperty {
        #[cfg_prop(
            default(64),
            attributes(#[gbuilder(default(64))])
        )]
        max_size: Option<u16>,

        #[cfg_prop(
            default(0),
            attributes(#[gbuilder(default(0))])
        )]
        rounding: Option<u16>,

        #[cfg_prop(attributes(#[gbuilder(default)]))]
        margin: Option<Spacing>,

        #[cfg_prop(attributes(#[gbuilder(default)]))]
        resizing_method: Option<ResizingMethod>,
    }
}

impl Default for ImageProperty {
    fn default() -> Self {
        TomlImageProperty::default().into()
    }
}

impl TryFrom<shared::value::Value> for ImageProperty {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
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

impl TryFrom<shared::value::Value> for ResizingMethod {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "nearest" => ResizingMethod::Nearest,
                "triangle" => ResizingMethod::Triangle,
                "catmull-rom" | "catmull_rom" => ResizingMethod::CatmullRom,
                "gaussian" => ResizingMethod::Gaussian,
                "lanczos3" => ResizingMethod::Lanczos3,
                _ => Err(shared::error::ConversionError::InvalidValue {
                    expected: "nearest, triangle, gaussian, lanczos3, catmull-rom or catmull_rom",
                    actual: str,
                })?,
            }),
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
    }
}

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(
        name(Border),
        derive(GenericBuilder, Debug, Clone, Default),
        attributes(#[gbuilder(name(GBuilderBorder))])
    )]
    struct TomlBorder {
        #[cfg_prop(
            default(0),
            attributes(#[gbuilder(default(0))])
        )]
        size: Option<u8>,

        #[cfg_prop(
            default(0),
            attributes(#[gbuilder(default(0))])
        )]
        radius: Option<u8>,

        #[cfg_prop(
            default(Color::new_black()),
            attributes(#[gbuilder(default(path = Color::new_black))])
        )]
        color: Option<Color>,
    }
}

impl TryFrom<shared::value::Value> for Border {
    type Error = ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    pub name: String,
    pub display: Option<TomlDisplayConfig>,
}

impl AppConfig {
    fn merge(mut self, other: Option<&TomlDisplayConfig>) -> Self {
        self.display = self
            .display
            .map(|display| display.merge(other.cloned()))
            .or(other.cloned());
        self
    }
}
