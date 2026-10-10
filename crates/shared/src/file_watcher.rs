use std::path::{Path, PathBuf};

use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use log::debug;

const DEFAULT_MASKS: WatchMask = WatchMask::MOVE_SELF
    .union(WatchMask::DELETE_SELF)
    .union(WatchMask::MODIFY);

type FilePath = PathBuf;

/// Watches a prioritized list of files for changes and provides convenient access
/// to the current file data.
///
/// `FilesWatcher` tracks a list of possible files in order of priority (left to right),
/// but internally only works with the first available file. If the currently tracked
/// file changes or a higher-priority file becomes available, the watcher considers
/// the data as updated.
///
/// This struct uses `inotify` internally to detect file changes efficiently,
/// but the user-facing API abstracts away these details.
/// Even if none of the files in the list exist, `FilesWatcher` handles it gracefully
/// and returns `None` as the current data.
#[derive(Debug)]
pub struct FilesWatcher {
    inotify: Inotify,
    paths: Vec<FilePath>,
    file_wd: Option<FileWd>,
}

impl FilesWatcher {
    /// Initializes a `FilesWatcher` from a list of file paths.
    ///
    /// The paths are treated in priority order: the first path has the highest priority,
    /// the second path has lower priority, and so on.  
    /// The watcher will track the first available file according to this order and
    /// consider the data updated if the file changes or a higher-priority file becomes available.
    pub fn init<T: AsRef<Path>>(paths: Vec<T>) -> anyhow::Result<Self> {
        assert!(
            !paths.is_empty(),
            "At least one path should be provided to FilesWatcher"
        );

        debug!("Watcher: Initializing");
        let inotify = Inotify::init()?;

        let paths: Vec<FilePath> = paths
            .iter()
            .map(<T as AsRef<Path>>::as_ref)
            .map(Path::to_path_buf)
            .collect();
        debug!(
            "Watcher: Received paths - {paths}",
            paths = paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let file_wd = paths
            .iter()
            .find(|path| path.is_file())
            .map(|path| inotify.new_wd(path));

        debug!("Watcher: Initialized");
        Ok(Self {
            inotify,
            paths,
            file_wd,
        })
    }

    /// Returns the path of the file currently being tracked by the `FilesWatcher`.
    ///
    /// `FilesWatcher` does not read the file itself; it only identifies which file
    /// from the prioritized list should be used. The caller is responsible for
    /// opening and reading the file data.
    pub fn get_watching_path(&self) -> Option<&Path> {
        self.file_wd
            .as_ref()
            .map(|file_wd| file_wd.path_buf.as_path())
    }

    /// Checks the highest-priority available file for updates.
    ///
    /// The caller is responsible for reading the file data if changes are detected.
    /// [`FilesWatcher`] only tracks which file should be used and detects changes; it
    /// does not read or cache file contents.
    pub fn check_updates(&mut self) -> FileState {
        let state = if let Some(file_path) = self.paths.iter().find(|path| path.is_file()) {
            if self
                .file_wd
                .as_ref()
                .is_some_and(|file_wd| file_path == &file_wd.path_buf)
            {
                let state = self.inotify.handle_events();

                if state.is_not_found() {
                    // INFO: The file path is available, but the watch descriptor thinks the file
                    // was moved or deleted earlier. So we don’t give a fuck and just recreate
                    // the watch descriptor with the current file path.

                    self.inotify.destroy_wd(self.file_wd.take());
                    self.file_wd = Some(self.inotify.new_wd(file_path));
                    FileState::Updated
                } else {
                    state
                }
            } else {
                // INFO: A different file from the prioritized list is now available.
                // Recreate the watch descriptor using this new file path.

                self.inotify.destroy_wd(self.file_wd.take());
                self.file_wd = Some(self.inotify.new_wd(file_path));
                FileState::Updated
            }
        } else {
            self.inotify.destroy_wd(self.file_wd.take());
            FileState::NotFound
        };

        state
    }
}

/// Represents the result of checking a file for updates in [`FilesWatcher`].
#[derive(Debug)]
pub enum FileState {
    /// None of the files in the prioritized list are currently available.
    NotFound,

    /// The file exists and its contents have changed since the last check.
    Updated,

    /// The file exists and has not changed since the last check.
    NothingChanged,
}

impl FileState {
    fn is_not_found(&self) -> bool {
        matches!(self, FileState::NotFound)
    }

    /// Returns a numeric priority for this `FileState`.
    ///
    /// Higher numbers indicate states that should take precedence over lower ones.
    fn priority(&self) -> u8 {
        match self {
            FileState::NotFound => 2,
            FileState::Updated => 1,
            FileState::NothingChanged => 0,
        }
    }
}

impl std::ops::BitOr for FileState {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self.priority().cmp(&rhs.priority()) {
            std::cmp::Ordering::Less => rhs,
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => self,
        }
    }
}

/// Represents a simple file watch descriptor tied to a specific path.
///
/// This struct simplifies managing inotify watch descriptors by pairing the
/// descriptor with its corresponding file path. It makes it easier to track
/// and remove descriptors when files are deleted or replaced.
#[derive(Debug)]
struct FileWd {
    wd: WatchDescriptor,
    path_buf: PathBuf,
}

impl FileWd {
    /// Creates a new `FileWd` from a watch descriptor and file path.
    ///
    /// This constructor ties the descriptor to the file path, allowing easy
    /// management of the watch descriptor when the file changes or is removed.
    fn from_wd(watch_descriptor: WatchDescriptor, path: PathBuf) -> Self {
        Self {
            wd: watch_descriptor,
            path_buf: path,
        }
    }
}

/// An extension trait for [`Inotify`] providing specialized file-watching logic.
///
/// This trait adds higher-level functionality for managing watch descriptors and
/// handling file changes in a prioritized and safe way. It uses [`FileWd`] to tie
/// a watch descriptor to its corresponding path and returns [`FileState`] to indicate
/// changes.
trait InotifySpecialization {
    /// Creates a new [`FileWd`] for the given path, registering it with inotify and
    /// associating it with the file path for easier management.
    fn new_wd(&self, path: &FilePath) -> FileWd;

    /// Removes the specified watch descriptor, if any, from inotify. This is useful
    /// when a file is deleted, moved, or replaced, ensuring that stale descriptors
    /// are cleaned up properly.
    fn destroy_wd(&self, file_wd: Option<FileWd>);

    /// Processes events from the inotify queue and returns a [`FileState`] representing
    /// the current status of the highest-priority available file.
    fn handle_events(&mut self) -> FileState;
}

impl InotifySpecialization for Inotify {
    fn new_wd(&self, file_path: &FilePath) -> FileWd {
        let new_wd = self
            .watches()
            .add(file_path.as_path(), DEFAULT_MASKS)
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to create watch descriptor for config path {file_path}",
                    file_path = file_path.display()
                )
            });

        FileWd::from_wd(new_wd, file_path.clone())
    }

    fn destroy_wd(&self, file_wd: Option<FileWd>) {
        if let Some(file_wd) = file_wd {
            let _ = self.watches().remove(file_wd.wd);
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
                .fold(FileState::NothingChanged, std::ops::BitOr::bitor),
            Err(_) => FileState::NothingChanged,
        }
    }
}
