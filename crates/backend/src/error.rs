use dbus::notification::Notification;

/// A generic error type that represents all possible errors in the backend.
pub(crate) enum Error {
    /// Contains a list of notifications that were not rendered for some reason.
    UnrenderedNotifications(Vec<Notification>),
    /// Represents any other kind of error that the application cannot handle.
    /// Such errors usually cause the application to exit.
    Fatal(anyhow::Error),
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Self::Fatal(value)
    }
}

impl From<Vec<Notification>> for Error {
    fn from(value: Vec<Notification>) -> Self {
        Self::UnrenderedNotifications(value)
    }
}
