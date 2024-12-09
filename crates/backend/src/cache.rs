use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use log::{error, warn};
use render::widget::Widget;
use shared::file_watcher::{FileState, FilesWatcher};

pub(super) struct CachedLayouts {
    layouts: HashMap<PathBuf, CachedLayout>,
}

impl CachedLayouts {
    pub(super) fn get(&self, path: &PathBuf) -> Option<&CachedLayout> {
        self.layouts.get(path)
    }

    pub(super) fn update(&mut self) {
        self.layouts
            .values_mut()
            .for_each(|layout| match layout.check_updates() {
                FileState::Updated => layout.update(),
                FileState::NothingChanged | FileState::NotFound => (),
            })
    }

    pub(super) fn extend_by_paths(&mut self, paths: Vec<&PathBuf>) {
        self.layouts.retain(|path, _| paths.contains(&path));

        for path_buf in paths {
            if self.layouts.contains_key(path_buf) {
                continue;
            }

            if let Some(cached_layout) = CachedLayout::from_path(path_buf) {
                self.layouts.insert(path_buf.to_owned(), cached_layout);
            }
        }
    }
}

impl<'a> FromIterator<&'a PathBuf> for CachedLayouts {
    fn from_iter<T: IntoIterator<Item = &'a PathBuf>>(iter: T) -> Self {
        let layouts = iter
            .into_iter()
            .filter_map(|path_buf| {
                CachedLayout::from_path(path_buf).map(|cached_layout| (path_buf, cached_layout))
            })
            .fold(HashMap::new(), |mut acc, (path_buf, cached_layout)| {
                acc.insert(path_buf.to_owned(), cached_layout);
                acc
            });

        Self { layouts }
    }
}

pub(super) struct CachedLayout {
    watcher: FilesWatcher,
    layout: Option<Widget>,
}

impl CachedLayout {
    fn from_path(path_buf: &PathBuf) -> Option<Self> {
        let watcher = match FilesWatcher::init(vec![path_buf]) {
            Ok(watcher) => watcher,
            Err(err) => {
                error!("Failed to init watcher for file {path_buf:?}. Error: {err}");
                return None;
            }
        };

        let layout = watcher
            .get_watching_path()
            .and_then(CachedLayout::load_layout);

        Some(CachedLayout { watcher, layout })
    }

    pub(super) fn layout(&self) -> Option<&Widget> {
        self.layout.as_ref()
    }

    fn check_updates(&mut self) -> FileState {
        self.watcher.check_updates()
    }

    fn update(&mut self) {
        self.layout = self.watcher.get_watching_path().and_then(Self::load_layout)
    }

    fn load_layout(path: &Path) -> Option<Widget> {
        match filetype::parse_layout(path) {
            Ok(widget) => Some(widget),
            Err(err) => {
                warn!("The layout by path {path:?} is not valid. Error: {err}");
                None
            }
        }
    }
}
