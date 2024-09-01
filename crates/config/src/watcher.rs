use std::{
    env,
    path::{Path, PathBuf},
};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use log::{debug, info};

const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
const HOME: &str = "HOME";
const APP_NAME: &str = env!("APP_NAME");
const CONFIG_FILE: &str = "config.toml";

const DEFAULT_MASKS: WatchMask = WatchMask::MOVE_SELF
    .union(WatchMask::DELETE_SELF)
    .union(WatchMask::MODIFY);

#[derive(Debug)]
pub(super) struct ConfigWatcher {
    inotify: Inotify,
    paths: Vec<ConfigPath>,
    config_wd: Option<ConfigWd>,
}

impl ConfigWatcher {
    pub(super) fn init(user_config: Option<&str>) -> anyhow::Result<Self> {
        debug!("Config Watcher: Initializing");
        let inotify = Inotify::init()?;

        let mut paths = vec![];
        if let Some(user_config_path) = ConfigPath::new_user_config(user_config) {
            paths.push(user_config_path);
            info!("Config Watcher: Detected user config path");
        }

        if let Some(xdg_path) = ConfigPath::new_xdg() {
            paths.push(xdg_path);
            info!("Config Watcher: Detected xdg config path");
        }

        if let Some(home_path) = ConfigPath::new_home() {
            paths.push(home_path);
            info!("Config Watcher: Detected home config path");
        }

        let config_wd = paths
            .iter()
            .find(|path| path.is_file())
            .map(|path| inotify.new_wd(path));

        debug!("Config Watcher: Initialized");
        Ok(Self {
            inotify,
            paths,
            config_wd,
        })
    }

    pub(super) fn get_config_path(&self) -> Option<&Path> {
        self.config_wd.as_ref().and_then(|config_wd| {
            self.paths
                .iter()
                .find(|path| path.destination == config_wd.destination)
                .map(|path| path.as_path())
        })
    }

    pub(super) fn check_updates(&mut self) -> ConfigState {
        let state = if let Some(path) = self.paths.iter().find(|path| path.is_file()) {
            if self
                .config_wd
                .as_ref()
                .is_some_and(|config_wd| path.destination == config_wd.destination)
            {
                let state = self.inotify.handle_events();

                if state.is_not_found() {
                    self.inotify.destroy_wd(self.config_wd.take());
                    self.config_wd = Some(self.inotify.new_wd(path));
                    ConfigState::Updated
                } else {
                    state
                }
            } else {
                self.inotify.destroy_wd(self.config_wd.take());
                self.config_wd = Some(self.inotify.new_wd(path));
                ConfigState::Updated
            }
        } else {
            self.inotify.destroy_wd(self.config_wd.take());
            ConfigState::NotFound
        };

        state
    }
}

#[derive(Debug)]
pub enum ConfigState {
    NotFound,
    Updated,
    NothingChanged,
}

impl ConfigState {
    fn is_not_found(&self) -> bool {
        matches!(self, ConfigState::NotFound)
    }

    fn priority(&self) -> u8 {
        match self {
            ConfigState::NotFound => 2,
            ConfigState::Updated => 1,
            ConfigState::NothingChanged => 0,
        }
    }

    fn more_prioritized(self, other: Self) -> Self {
        match self.priority().cmp(&other.priority()) {
            std::cmp::Ordering::Less => other,
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => self,
        }
    }
}

/// The config watch descriptor
#[derive(Debug)]
struct ConfigWd {
    wd: WatchDescriptor,
    destination: ConfigDestination,
}

impl ConfigWd {
    fn from_wd(watch_descriptor: WatchDescriptor, destination: ConfigDestination) -> Self {
        Self {
            wd: watch_descriptor,
            destination,
        }
    }
}

#[derive(Debug)]
struct ConfigPath {
    path_buf: PathBuf,
    destination: ConfigDestination,
}

impl ConfigPath {
    fn new_user_config(user_config: Option<&str>) -> Option<Self> {
        user_config.map(|path| Self {
            path_buf: PathBuf::from(path),
            destination: ConfigDestination::UserConfig,
        })
    }

    fn new_xdg() -> Option<Self> {
        Some(Self {
            path_buf: Self::to_config_file(PathBuf::from(env::var(XDG_CONFIG_HOME).ok()?)),
            destination: ConfigDestination::Xdg,
        })
    }

    fn new_home() -> Option<Self> {
        let mut home_path = PathBuf::from(env::var(HOME).ok()?);
        home_path.push(".config");

        Some(Self {
            path_buf: Self::to_config_file(home_path),
            destination: ConfigDestination::Home,
        })
    }

    fn to_config_file(mut path: PathBuf) -> PathBuf {
        path.push(APP_NAME);
        path.push(CONFIG_FILE);
        path
    }

    fn is_file(&self) -> bool {
        self.path_buf.is_file()
    }

    fn as_path(&self) -> &Path {
        self.path_buf.as_path()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigDestination {
    UserConfig,
    Xdg,
    Home,
}

trait InotifyConfigSpecialization {
    fn new_wd(&self, path: &ConfigPath) -> ConfigWd;
    fn destroy_wd(&self, config_wd: Option<ConfigWd>);

    fn handle_events(&mut self) -> ConfigState;
}

impl InotifyConfigSpecialization for Inotify {
    fn new_wd(&self, path: &ConfigPath) -> ConfigWd {
        let new_wd = self
            .watches()
            .add(path.as_path(), DEFAULT_MASKS)
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to create watch descriptor for config path {:?}",
                    path
                )
            });

        ConfigWd::from_wd(new_wd, path.destination.clone())
    }

    fn destroy_wd(&self, config_wd: Option<ConfigWd>) {
        if let Some(config_wd) = config_wd {
            let _ = self.watches().remove(config_wd.wd);
        }
    }

    fn handle_events(&mut self) -> ConfigState {
        let mut buffer = [0; 4096];

        match self.read_events(&mut buffer) {
            Ok(events) => events
                .into_iter()
                .map(|event| {
                    if event.mask.contains(EventMask::MODIFY) {
                        ConfigState::Updated
                    } else if event.mask.contains(EventMask::DELETE_SELF)
                        || event.mask.contains(EventMask::MOVE_SELF)
                    {
                        ConfigState::NotFound
                    } else {
                        ConfigState::NothingChanged
                    }
                })
                .fold(ConfigState::NothingChanged, ConfigState::more_prioritized),
            Err(_) => ConfigState::NothingChanged,
        }
    }
}
