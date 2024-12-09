use std::path::{Path, PathBuf};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use log::debug;

const DEFAULT_MASKS: WatchMask = WatchMask::MOVE_SELF
    .union(WatchMask::DELETE_SELF)
    .union(WatchMask::MODIFY);

#[derive(Debug)]
pub struct FilesWatcher {
    inotify: Inotify,
    paths: Vec<FilePath>,
    config_wd: Option<FileWd>,
}

/// The struct that can detect file changes using inotify.
impl FilesWatcher {
    /// Creates a `FilesWatcher` struct from provided paths.
    ///
    /// The paths are **arranged**. It's means that first path is more prioritized than second and
    /// second is more prioritized than third and so on.
    pub fn init<T: AsRef<Path>>(paths: Vec<T>) -> anyhow::Result<Self> {
        debug!("Watcher: Initializing");
        let inotify = Inotify::init()?;

        let paths: Vec<FilePath> = paths.into_iter().map(From::from).collect();
        debug!("Watcher: Received paths - {paths:?}");

        let config_wd = paths
            .iter()
            .find(|path| path.is_file())
            .map(|path| inotify.new_wd(path));

        debug!("Watcher: Initialized");
        Ok(Self {
            inotify,
            paths,
            config_wd,
        })
    }

    pub fn get_watching_path(&self) -> Option<&Path> {
        self.config_wd
            .as_ref()
            .map(|config_wd| config_wd.path_buf.as_path())
    }

    pub fn check_updates(&mut self) -> FileState {
        let state = if let Some(file_path) = self.paths.iter().find(|path| path.is_file()) {
            if self
                .config_wd
                .as_ref()
                .is_some_and(|config_wd| file_path.path_buf == config_wd.path_buf)
            {
                let state = self.inotify.handle_events();

                if state.is_not_found() {
                    self.inotify.destroy_wd(self.config_wd.take());
                    // INFO: the file is found but the watch descriptor says that the files is
                    // moved or deleted. As I understand right, we don't give a fuck what is
                    // earlier and use the path above as 'last thing that checked' and create new
                    // WatchDescriptor.
                    self.config_wd = Some(self.inotify.new_wd(file_path));
                    FileState::Updated
                } else {
                    state
                }
            } else {
                self.inotify.destroy_wd(self.config_wd.take());
                // INFO: same as above
                self.config_wd = Some(self.inotify.new_wd(file_path));
                FileState::Updated
            }
        } else {
            self.inotify.destroy_wd(self.config_wd.take());
            FileState::NotFound
        };

        state
    }
}

#[derive(Debug)]
pub enum FileState {
    NotFound,
    Updated,
    NothingChanged,
}

impl FileState {
    fn is_not_found(&self) -> bool {
        matches!(self, FileState::NotFound)
    }

    fn priority(&self) -> u8 {
        match self {
            FileState::NotFound => 2,
            FileState::Updated => 1,
            FileState::NothingChanged => 0,
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
struct FileWd {
    wd: WatchDescriptor,
    path_buf: PathBuf,
}

impl FileWd {
    fn from_wd(watch_descriptor: WatchDescriptor, path: PathBuf) -> Self {
        Self {
            wd: watch_descriptor,
            path_buf: path,
        }
    }
}

#[derive(Debug)]
struct FilePath {
    path_buf: PathBuf,
}

impl FilePath {
    fn is_file(&self) -> bool {
        self.path_buf.is_file()
    }

    fn as_path(&self) -> &Path {
        self.path_buf.as_path()
    }
}

impl<T: AsRef<Path>> From<T> for FilePath {
    fn from(value: T) -> Self {
        FilePath {
            path_buf: value.as_ref().to_path_buf(),
        }
    }
}

trait InotifySpecialization {
    fn new_wd(&self, path: &FilePath) -> FileWd;
    fn destroy_wd(&self, config_wd: Option<FileWd>);

    fn handle_events(&mut self) -> FileState;
}

impl InotifySpecialization for Inotify {
    fn new_wd(&self, file_path: &FilePath) -> FileWd {
        let new_wd = self
            .watches()
            .add(file_path.as_path(), DEFAULT_MASKS)
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to create watch descriptor for config path {:?}",
                    file_path
                )
            });

        FileWd::from_wd(new_wd, file_path.path_buf.clone())
    }

    fn destroy_wd(&self, config_wd: Option<FileWd>) {
        if let Some(config_wd) = config_wd {
            let _ = self.watches().remove(config_wd.wd);
        }
    }

    fn handle_events(&mut self) -> FileState {
        let mut buffer = [0; 4096];

        match self.read_events(&mut buffer) {
            Ok(events) => events
                .into_iter()
                .map(|event| {
                    if event.mask.contains(EventMask::MODIFY) {
                        FileState::Updated
                    } else if event.mask.contains(EventMask::DELETE_SELF)
                        || event.mask.contains(EventMask::MOVE_SELF)
                    {
                        FileState::NotFound
                    } else {
                        FileState::NothingChanged
                    }
                })
                .fold(FileState::NothingChanged, FileState::more_prioritized),
            Err(_) => FileState::NothingChanged,
        }
    }
}
