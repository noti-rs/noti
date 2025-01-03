use dbus::notification::Notification;

pub(crate) enum Error {
    UnrenderedNotifications(Vec<Notification>),
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
