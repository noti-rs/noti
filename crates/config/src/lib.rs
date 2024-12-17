use display::{DisplayConfig, TomlDisplayConfig};
use general::{GeneralConfig, TomlGeneralConfig};
use log::{debug, error, warn};
use serde::Deserialize;
use shared::file_watcher::{FileState, FilesWatcher};
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

    default_theme: Theme,
    themes: HashMap<String, Theme>,

    app_configs: HashMap<String, DisplayConfig>,
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

        let ParsedConfig {
            subwatchers,
            general,
            display,
            themes,
            app_configs,
        } = Self::parse(main_watcher.get_watching_path());

        debug!("Config: Initialized");

        Self {
            main_watcher,
            subwatchers,
            general,
            display,
            app_configs,

            default_theme: Theme::default(),
            themes,
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

    pub fn theme_by_app(&self, name: &str) -> &Theme {
        self.themes
            .get(&self.display_by_app(name).theme)
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

    pub fn update(&mut self) {
        let ParsedConfig {
            subwatchers,
            general,
            display,
            themes,
            app_configs: apps,
        } = Self::parse(self.main_watcher.get_watching_path());

        self.subwatchers = subwatchers;
        self.general = general;
        self.display = display;
        self.app_configs = apps;
        self.themes = themes;

        debug!("Config: Updated");
    }

    fn parse(path: Option<&Path>) -> ParsedConfig {
        let (subwatchers, toml_config) =
            match TomlConfig::parse_recursive(path, &mut std::collections::HashSet::new()) {
                Some(ParsedTomlConfig {
                    subwatchers,
                    toml_config,
                }) => (subwatchers, toml_config),
                None => (vec![], Default::default()),
            };

        let TomlConfig {
            general,
            display,
            themes,
            apps,
            ..
        } = toml_config;

        let mut theme_table: HashMap<String, TomlTheme> = HashMap::new();
        if let Some(themes) = themes {
            for theme in themes {
                if theme.name.as_ref().is_none_or(|name| name.is_empty()) {
                    warn!("Config: Encountered an unnamed theme. Skipped.");
                    continue;
                }

                let theme_name = theme.name.as_ref().cloned().unwrap();
                let theme_to_save = match theme_table.remove(&theme_name) {
                    Some(saved_theme) => saved_theme.merge(Some(theme)),
                    None => theme,
                };
                theme_table.insert(theme_name, theme_to_save);
            }
        }

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

        ParsedConfig {
            subwatchers,
            general: general.unwrap_or_default().into(),
            display: display.unwrap_or_default().into(),
            themes: theme_table
                .into_iter()
                .map(|(key, value)| (key, value.unwrap_or_default()))
                .collect(),
            app_configs: app_configs
                .into_iter()
                .map(|(key, value)| (key, value.unwrap_or_default()))
                .collect(),
        }
    }
}

struct ParsedConfig {
    subwatchers: Vec<FilesWatcher>,
    general: GeneralConfig,
    display: DisplayConfig,
    themes: HashMap<String, Theme>,
    app_configs: HashMap<String, DisplayConfig>,
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

    #[serde(rename(deserialize = "theme"))]
    themes: Option<Vec<TomlTheme>>,

    #[serde(rename(deserialize = "app"))]
    apps: Option<Vec<AppConfig>>,
}

impl TomlConfig {
    fn parse_recursive(
        path: Option<&Path>,
        config_tree_path: &mut std::collections::HashSet<PathBuf>,
    ) -> Option<ParsedTomlConfig> {
        let config_path = PathBuf::from(path?);
        if !config_tree_path.insert(config_path.clone()) {
            error!("Found circular imports! Check the config file {config_path:?}");
            return None;
        }

        let Some(mut base_toml_config) = Self::parse(path?) else {
            config_tree_path.remove(&config_path);
            return None;
        };

        let path_prefix = {
            let mut config_path = config_path.clone();
            config_path.pop();
            config_path
        };

        if let Some(display) = base_toml_config.display.as_mut() {
            display.use_relative_path(path_prefix.clone());
        }

        let mut watchers: Vec<FilesWatcher> = base_toml_config
            .imports
            .as_ref()
            .cloned()
            .into_iter()
            .flatten()
            .flat_map(|path| expand_path(path, &path_prefix))
            .map(|path| {
                FilesWatcher::init(vec![path])
                    .expect("The subconfigs watcher should be initialized")
            })
            .collect();

        let subconfigs: Vec<ParsedTomlConfig> = watchers
            .iter()
            .map(FilesWatcher::get_watching_path)
            .filter_map(|path| TomlConfig::parse_recursive(path, config_tree_path))
            .collect();

        for ParsedTomlConfig {
            subwatchers,
            toml_config,
        } in subconfigs
        {
            watchers.extend(subwatchers);
            base_toml_config = base_toml_config.merge(toml_config);
        }

        config_tree_path.remove(&config_path);
        Some(ParsedTomlConfig {
            subwatchers: watchers,
            toml_config: base_toml_config,
        })
    }

    fn parse(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).unwrap();
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

        let themes: Vec<_> = self
            .themes
            .into_iter()
            .chain(other.themes)
            .flatten()
            .collect();
        self.themes = themes.is_empty().not().then_some(themes);

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

fn expand_path(value: String, path_prefix: &Path) -> Vec<PathBuf> {
    let mut expanded_path: PathBuf = shellexpand::full(&value)
        .map(|value| value.into_owned())
        .unwrap_or(value)
        .into();

    if expanded_path.is_relative() {
        let mut prefix = path_prefix.to_path_buf();
        prefix.extend(&expanded_path);
        expanded_path = prefix;
    }

    let Some(expanded_path_str) = expanded_path.to_str() else {
        error!("Path {expanded_path:?} is not UTF-8 valid!");
        return vec![];
    };

    match glob::glob(expanded_path_str) {
        Ok(entries) => entries
            .into_iter()
            .inspect(|entry| {
                if let Err(err) = entry {
                    warn!(
                        "Failed to read config file at {:?}. Error: {}",
                        err.path(),
                        err.error()
                    );
                }
            })
            .flatten()
            .collect(),
        Err(err) => {
            error!("Failed to parse path glob pattern. Error: {err}");
            vec![]
        }
    }
}
