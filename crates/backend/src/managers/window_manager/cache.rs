use std::path::{Path, PathBuf};

use log::warn;
use render::widget::Widget;
use shared::{
    cached_data::{CacheUpdate, CachedValueError},
    file_watcher::{FileState, FilesWatcher},
};

pub(super) struct CachedLayout {
    watcher: FilesWatcher,
    layout: Option<Widget>,
}

impl CachedLayout {
    pub(super) fn layout(&self) -> Option<&Widget> {
        self.layout.as_ref()
    }

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
