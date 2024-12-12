use display::{DisplayConfig, TomlDisplayConfig};
use general::{GeneralConfig, TomlGeneralConfig};
use log::{debug, error};
use serde::Deserialize;
use shared::{
    cached_data::{CacheUpdate, CachedData, CachedValueError},
    file_watcher::{FileState, FilesWatcher},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use theme::{Theme, TomlTheme};

pub mod color;
pub mod display;
pub mod general;
pub mod sorting;
pub mod spacing;
pub mod text;
pub mod theme;

use spacing::Spacing;

const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const HOME: &str = "HOME";
const APP_NAME: &str = env!("APP_NAME");
const CONFIG_FILE: &str = "config.toml";

pub struct Config {
    watcher: FilesWatcher,
    general: GeneralConfig,
    display: DisplayConfig,

    app_configs: HashMap<String, DisplayConfig>,

    default_theme: Theme,
    cached_themes: CachedData<String, CachedTheme>,
}

impl Config {
    pub fn init(user_config: Option<&str>) -> Self {
        let config_paths = [
            user_config.map(PathBuf::from),
            xdg_config_dir(CONFIG_FILE),
            home_config_dir(CONFIG_FILE),
        ]
        .into_iter()
        .flatten()
        .collect();

        debug!("Config: Initializing");
        let watcher =
            FilesWatcher::init(config_paths).expect("The config watcher must be initialized");

        let (general, display, app_configs) = Self::parse(watcher.get_watching_path());
        debug!("Config: Initialized");

        let mut config = Self {
            watcher,
            general,
            display,
            app_configs,

            default_theme: Theme::default(),
            cached_themes: CachedData::default(),
        };

        config.cached_themes.extend_by_keys(
            config
                .displays()
                .map(|display| display.theme.to_owned())
                .collect(),
        );

        config
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

    pub fn theme_by_app(&self, name: &str) -> &Theme {
        self.cached_themes
            .get(&self.display_by_app(name).theme)
            .and_then(|cached_theme| cached_theme.theme.as_ref())
            .unwrap_or(&self.default_theme)
    }

    pub fn check_display_updates(&mut self) -> FileState {
        self.watcher.check_updates()
    }

    pub fn update_themes(&mut self) -> bool {
        self.cached_themes.update()
    }

    pub fn update(&mut self) {
        let (general, display, app_configs) = Self::parse(self.watcher.get_watching_path());

        self.general = general;
        self.display = display;
        self.app_configs = app_configs;

        self.cached_themes.extend_by_keys(
            self.displays()
                .map(|display| display.theme.to_owned())
                .collect(),
        );

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
        path.map(|config_path| std::fs::read_to_string(config_path).unwrap())
            .and_then(|content| match toml::from_str(&content) {
                Ok(content) => Some(content),
                Err(error) => {
                    error!("{error}");
                    None
                }
            })
            .unwrap_or_default()
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

struct CachedTheme {
    watcher: FilesWatcher,
    theme: Option<Theme>,
}

impl CachedTheme {
    fn load_theme(path: &Path) -> Option<Theme> {
        let file_content = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => {
                    error!("The theme file at {path:?} is not existing!");
                    return None;
                }
                std::io::ErrorKind::PermissionDenied => {
                    error!("The theme file at {path:?} is restricted to read!");
                    return None;
                }
                other_fs_err => {
                    error!("Cannot read the theme file due unknown error. Error: {other_fs_err}");
                    return None;
                }
            },
        };

        match toml::from_str::<TomlTheme>(&file_content) {
            Ok(theme) => Some(theme.unwrap_or_default()),
            Err(err) => {
                error!("Failed to parse theme file at {path:?}. Reason: {err}");

                None
            }
        }
    }
}

impl CacheUpdate for CachedTheme {
    fn check_updates(&mut self) -> FileState {
        self.watcher.check_updates()
    }

    fn update(&mut self) {
        self.theme = self.watcher.get_watching_path().and_then(Self::load_theme)
    }
}

impl TryFrom<&String> for CachedTheme {
    type Error = CachedValueError;

    fn try_from(theme_name: &String) -> Result<Self, Self::Error> {
        let suffix = "/themes/".to_owned() + theme_name + ".toml";
        let possible_paths = vec![xdg_config_dir(&suffix), home_config_dir(&suffix)]
            .into_iter()
            .flatten()
            .collect();

        let watcher = match FilesWatcher::init(possible_paths) {
            Ok(watcher) => watcher,
            Err(err) => {
                return Err(CachedValueError::FailedInitWatcher { source: err });
            }
        };

        let theme = watcher.get_watching_path().and_then(Self::load_theme);

        Ok(Self { watcher, theme })
    }
}

fn xdg_config_dir(suffix: &str) -> Option<PathBuf> {
    std::env::var(XDG_CONFIG_HOME)
        .map(|mut path| {
            path.push('/');
            path.push_str(APP_NAME);
            path.push('/');
            path.push_str(suffix);
            path
        })
        .map(PathBuf::from)
        .ok()
}

fn home_config_dir(suffix: &str) -> Option<PathBuf> {
    std::env::var(HOME)
        .map(|mut path| {
            path.push_str("/.config/");
            path.push_str(APP_NAME);
            path.push('/');
            path.push_str(suffix);
            path
        })
        .map(PathBuf::from)
        .ok()
}
