use std::{collections::HashMap, path::PathBuf};

use log::error;

use crate::file_watcher::FileState;

pub struct CachedData<K, V>(HashMap<K, V>)
where
    K: std::cmp::Eq + std::hash::Hash,
    V: for<'a> TryFrom<&'a K, Error = CachedValueError>;

impl<K, V> CachedData<K, V>
where
    K: std::cmp::Eq + std::hash::Hash + ToOwned<Owned = K>,
    V: for<'a> TryFrom<&'a K, Error = CachedValueError>,
{
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

    pub fn extend_by_keys(&mut self, keys: Vec<&K>) {
        self.0.retain(|key, _| keys.contains(&key));

        for key in keys {
            if self.0.contains_key(key) {
                continue;
            }

            match V::try_from(key) {
                Ok(data) => {
                    self.0.insert(key.to_owned(), data);
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
        let data = iter
            .into_iter()
            .filter_map(|key| V::try_from(key).map(|value| (key, value)).ok())
            .fold(HashMap::new(), |mut acc, (key, value)| {
                acc.insert(key.to_owned(), value);
                acc
            });

        Self(data)
    }
}

pub trait CacheUpdate {
    fn check_updates(&mut self) -> FileState;
    fn update(&mut self);
}

#[derive(derive_more::Display)]
pub enum CachedValueError {
    #[display("Failed to init file watcher for file {path_buf:?}. Error: {source}")]
    FailedInitWatcher {
        path_buf: PathBuf,
        source: anyhow::Error,
    },
}
