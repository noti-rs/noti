use std::path::{Path, PathBuf};

use log::warn;
use shared::{
    cached_data::{CacheUpdate, CachedValueError},
    file_watcher::{FileState, FilesWatcher},
};
use widgets::widget::Widget;

/// Represents a custom widget layout loaded into memory. This wrapper allows detecting
/// changes to the layout file and reloading it on demand.
pub(super) struct CachedLayout {
    watcher: FilesWatcher,
    layout: Option<Widget>,
}

impl CachedLayout {
    /// Returns a custom widget layout if it has been successfully loaded.
    ///
    /// The layout may not exist if the file path is invalid, the file is missing, or any other
    /// error occurs while loading.
    #[allow(unused)]
    pub(super) fn layout(&self) -> Option<&Widget> {
        self.layout.as_ref()
    }

    /// Attempts to load a custom widget layout from the specified path.
    ///
    /// Returns `None` if the layout is invalid or the file is missing.
    fn load_layout(path: &Path) -> Option<Widget> {
        match filetype::parse_layout(path) {
            Ok(widget) => Some(widget),
            Err(err) => {
                warn!(
                    "The layout by path {path} is not valid. Error: {err}",
                    path = path.display()
                );
                None
            }
        }
    }
}

impl CacheUpdate for CachedLayout {
    fn check_updates(&mut self) -> FileState {
        self.watcher.check_updates()
    }

    fn update(&mut self) {
        self.layout = self.watcher.get_watching_path().and_then(Self::load_layout)
    }
}

impl<'a> TryFrom<&'a PathBuf> for CachedLayout {
    type Error = CachedValueError;

    fn try_from(path_buf: &'a PathBuf) -> Result<Self, Self::Error> {
        let watcher = match FilesWatcher::init(vec![path_buf]) {
            Ok(watcher) => watcher,
            Err(err) => {
                return Err(CachedValueError::FailedInitWatcher { source: err });
            }
        };

        let layout = watcher
            .get_watching_path()
            .and_then(CachedLayout::load_layout);

        Ok(CachedLayout { watcher, layout })
    }
}
