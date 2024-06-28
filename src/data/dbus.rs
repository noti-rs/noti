use super::notification::Notification;

pub enum Action {
    Show(Notification),
    ShowLast,
    Close(Option<u32>),
    CloseAll,
}

pub enum Signal {
    ActionInvoked { notification_id: u32 },
    NotificationClosed { notification_id: u32, reason: u32 },
}
