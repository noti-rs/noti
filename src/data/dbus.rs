use super::notification::Notification;

pub enum Action {
    Show(Notification),
    ShowLast,
    Close(Option<u32>),
    CloseAll,
}

pub enum Signal {
    ActionInvoked {
        notification_id: u32,
    },
    NotificationClosed {
        notification_id: u32,
        reason: ClosingReason,
    },
}

pub enum ClosingReason {
    Expired,
    DimissedByUser,
    CallCloseNotification,
    Undefined,
}

impl From<ClosingReason> for u32 {
    fn from(value: ClosingReason) -> Self {
        match value {
            ClosingReason::Expired => 1,
            ClosingReason::DimissedByUser => 2,
            ClosingReason::CallCloseNotification => 3,
            ClosingReason::Undefined => 4,
        }
    }
}
