use std::{
    fs::File,
    mem::ManuallyDrop,
    os::fd::{FromRawFd, IntoRawFd},
    sync::Arc,
};

/// A shareable and cloneable wrapper around a file descriptor.
///
/// Unlike [`std::fs::File`], this type allows multiple parts of the code to
/// hold references to the same open file safely. The file remains open as long
/// as at least one reference exists.
///
/// Internally, it uses `Arc` to track references. Dropping the last reference
/// automatically closes the underlying file descriptor, ensuring safe cleanup
/// without accidental premature closure.
#[derive(Debug, Clone)]
pub struct FileDescriptor(Arc<i32>);

impl FileDescriptor {
    /// Returns a [`FileGuard`] for this file descriptor.
    ///
    /// This allows controlled access to the file for reading or writing,
    /// while guaranteeing that the file remains open until all `FileGuard`s
    /// are dropped.
    pub fn get_file(&self) -> FileGuard {
        FileGuard::from_fd(self)
    }
}

impl From<File> for FileDescriptor {
    fn from(value: File) -> Self {
        FileDescriptor(Arc::new(value.into_raw_fd()))
    }
}

impl Drop for FileDescriptor {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == 1 {
            // SAFETY: the struct guarantees that there will not be a close for file if the count
            // of handlers more than one. So at last drop file will be closed.
            unsafe {
                File::from_raw_fd(*self.0);
            }
        }
    }
}

/// A safe handle to a [`FileDescriptor`] that allows file operations without
/// risking accidental closure.
///
/// `FileGuard` provides access to permitted methods like `read` and `write`,
/// while ensuring that the underlying file remains open. Even if the original
/// `FileDescriptor` is dropped, the file will stay open as long as a `FileGuard` exists.
pub struct FileGuard {
    file: ManuallyDrop<File>,
    _file_descriptor: FileDescriptor,
}

impl FileGuard {
    /// Constructs a new `FileGuard` from a given [`FileDescriptor`].
    ///
    /// This is the **only constructor** for `FileGuard`, ensuring that every
    /// guard is tied to a valid, shareable file descriptor.
    fn from_fd(file_descriptor: &FileDescriptor) -> Self {
        Self {
            file: ManuallyDrop::new(unsafe { File::from_raw_fd(*file_descriptor.0) }),
            _file_descriptor: file_descriptor.clone(),
        }
    }
}

impl std::ops::Deref for FileGuard {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl std::ops::DerefMut for FileGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        let file = unsafe { ManuallyDrop::take(&mut self.file) };
        let _fd = file.into_raw_fd();
    }
}
