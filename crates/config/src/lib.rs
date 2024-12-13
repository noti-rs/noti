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
    ops::Not,
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
    main_watcher: FilesWatcher,
    subwatchers: Vec<FilesWatcher>,
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
        let main_watcher =
            FilesWatcher::init(config_paths).expect("The config watcher must be initialized");

        let (subwatchers, general, display, app_configs) =
            Self::parse(main_watcher.get_watching_path());

        debug!("Config: Initialized");

        let mut config = Self {
            main_watcher,
            subwatchers,
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

    pub fn check_updates(&mut self) -> FileState {
        self.main_watcher.check_updates()
            | self
                .subwatchers
                .iter_mut()
                .map(FilesWatcher::check_updates)
                .fold(FileState::NothingChanged, |lhs, rhs| lhs | rhs)
    }

    pub fn update_themes(&mut self) -> bool {
        self.cached_themes.update()
    }

    pub fn update(&mut self) {
        let (subwatchers, general, display, app_configs) =
            Self::parse(self.main_watcher.get_watching_path());

        self.subwatchers = subwatchers;
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
    ) -> (
        Vec<FilesWatcher>,
        GeneralConfig,
        DisplayConfig,
        HashMap<String, DisplayConfig>,
    ) {
        let (subwatchers, toml_config) = match TomlConfig::parse_recursive(path) {
            Some(ParsedTomlConfig {
                subwatchers,
                toml_config,
            }) => (subwatchers, toml_config),
            None => (vec![], Default::default()),
        };

        let TomlConfig {
            general,
            display,
            apps,
            ..
        } = toml_config;

        let mut app_configs: HashMap<String, TomlDisplayConfig> = HashMap::new();

        if let Some(apps) = apps {
            for app in apps {
                let app_name = app.name.clone();
                let app_display_config = match app_configs.remove(&app_name) {
                    Some(saved_app_config) => saved_app_config.merge(app.display),
                    None => app.merge(display.as_ref()).display.unwrap(),
                };
                app_configs.insert(app_name, app_display_config);
            }
        }

        debug!("Config: Parsed from files");

        (
            subwatchers,
            general.unwrap_or_default().into(),
            display.unwrap_or_default().into(),
            app_configs
                .into_iter()
                .map(|(key, value)| (key, value.unwrap_or_default()))
                .collect(),
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
    #[serde(rename(deserialize = "use"))]
    imports: Option<Vec<String>>,

    general: Option<TomlGeneralConfig>,
    display: Option<TomlDisplayConfig>,

    #[serde(rename(deserialize = "app"))]
    apps: Option<Vec<AppConfig>>,
}

impl TomlConfig {
    fn parse_recursive(path: Option<&Path>) -> Option<ParsedTomlConfig> {
        fn expand_path(value: String) -> PathBuf {
            shellexpand::full(&value)
                .map(|value| value.into_owned())
                .unwrap_or(value)
                .into()
        }

        let mut base_toml_config = Self::parse(path)?;
        let mut watchers: Vec<FilesWatcher> = base_toml_config
            .imports
            .as_ref()
            .cloned()
            .into_iter()
            .flatten()
            .map(expand_path)
            .map(|path| {
                FilesWatcher::init(vec![path])
                    .expect("The subconfigs watcher should be initialized")
            })
            .collect();

        let subconfigs: Vec<ParsedTomlConfig> = watchers
            .iter()
            .map(FilesWatcher::get_watching_path)
            .filter_map(TomlConfig::parse_recursive)
            .collect();

        for ParsedTomlConfig {
            subwatchers,
            toml_config,
        } in subconfigs
        {
            watchers.extend(subwatchers);
            base_toml_config = base_toml_config.merge(toml_config);
        }

        Some(ParsedTomlConfig {
            subwatchers: watchers,
            toml_config: base_toml_config,
        })
    }

    fn parse(path: Option<&Path>) -> Option<Self> {
        let content = std::fs::read_to_string(path?).unwrap();
        match toml::from_str(&content) {
            Ok(content) => Some(content),
            Err(error) => {
                error!("{error}");
                None
            }
        }
    }

    fn merge(mut self, other: Self) -> Self {
        let imports: Vec<_> = self
            .imports
            .into_iter()
            .chain(other.imports)
            .flatten()
            .collect();
        self.imports = imports.is_empty().not().then_some(imports);

        self.general = self
            .general
            .map(|general| general.merge(other.general.clone()))
            .or(other.general);

        self.display = self
            .display
            .map(|display| display.merge(other.display.clone()))
            .or(other.display);

        let apps: Vec<_> = self.apps.into_iter().chain(other.apps).flatten().collect();
        self.apps = apps.is_empty().not().then_some(apps);

        self
    }
}

struct ParsedTomlConfig {
    subwatchers: Vec<FilesWatcher>,
    toml_config: TomlConfig,
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
