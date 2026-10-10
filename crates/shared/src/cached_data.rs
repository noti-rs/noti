use std::collections::HashMap;

use log::error;

use crate::file_watcher::FileState;

/// A helper struct for managing cached data with automatic updates and modifications.
///
/// `CachedData` provides a convenient API for storing and accessing key-value data,
/// while handling changes and extensions efficiently.
///
/// The caller only needs to implement a [CacheUpdate] to notify about external changes,
/// allowing `CachedData` to keep the cache up-to-date automatically.
///
/// This abstraction simplifies cache management and ensures consistency without
/// exposing the underlying storage details.
pub struct CachedData<K, V>(HashMap<K, V>)
where
    K: std::cmp::Eq + std::hash::Hash,
    V: for<'a> TryFrom<&'a K, Error = CachedValueError>;

impl<K, V> CachedData<K, V>
where
    K: std::cmp::Eq + std::hash::Hash + ToOwned<Owned = K>,
    V: for<'a> TryFrom<&'a K, Error = CachedValueError>,
{
    pub fn new() -> Self {
        CachedData(HashMap::new())
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }

    pub fn update(&mut self) -> bool
    where
        V: CacheUpdate,
    {
        let mut updated = false;
        self.0
            .values_mut()
            .for_each(|value| match value.check_updates() {
                FileState::Updated => {
                    value.update();
                    updated = true
                }
                FileState::NothingChanged | FileState::NotFound => (),
            });
        updated
    }

    pub fn extend_by_keys(&mut self, keys: Vec<K>) {
        self.0.retain(|key, _| keys.contains(key));

        for key in keys {
            if self.0.contains_key(&key) {
                continue;
            }

            match V::try_from(&key) {
                Ok(data) => {
                    self.0.insert(key, data);
                }
                Err(err) => {
                    error!("{err}")
                }
            }
        }
    }
}

impl<'a, K, V> FromIterator<&'a K> for CachedData<K, V>
where
    K: std::cmp::Eq + std::hash::Hash + ToOwned<Owned = K>,
    V: for<'b> TryFrom<&'b K, Error = CachedValueError>,
{
    fn from_iter<T: IntoIterator<Item = &'a K>>(iter: T) -> Self {
        Self(
            iter.into_iter()
                .filter_map(|key| V::try_from(key).map(|value| (key.to_owned(), value)).ok())
                .collect(),
        )
    }
}

impl<K, V> Default for CachedData<K, V>
where
    K: std::cmp::Eq + std::hash::Hash + ToOwned<Owned = K>,
    V: for<'a> TryFrom<&'a K, Error = CachedValueError>,
{
    fn default() -> Self {
        Self::new()
    }
}

/// A trait for checking and updating cached data.
///
/// Types implementing `CacheUpdate` provide a way for `CachedData` to detect
/// whether the underlying data has changed, allowing the cache to be updated
/// accordingly.
///
/// While `CacheUpdate` itself is generic, it often interacts with [`crate::file_watcher::FilesWatcher`]
/// indirectly: one of its methods returns a [`FileState`], which relies on `FileWatcher`
/// to detect changes in files and update the cache accordingly.
///
/// Implementing this trait allows `CachedData` to stay synchronized with external
/// data sources automatically.
pub trait CacheUpdate {
    fn check_updates(&mut self) -> FileState;
    fn update(&mut self);
}

#[derive(derive_more::Display)]
pub enum CachedValueError {
    #[display("Failed to init file watcher for file. Error: {source}")]
    FailedInitWatcher { source: anyhow::Error },
}
