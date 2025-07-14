use std::{
    fs::File,
    mem::ManuallyDrop,
    os::fd::{FromRawFd, IntoRawFd},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct FileDescriptor(Arc<i32>);

impl FileDescriptor {
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

/// The file guard of file descriptor that guarantees not closing file.
///
/// The main purpose of this guard for providing API for client code which should not close the
/// file but able to read and write file.
pub struct FileGuard {
    file: ManuallyDrop<File>,
    _file_descriptor: FileDescriptor,
}

impl FileGuard {
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
