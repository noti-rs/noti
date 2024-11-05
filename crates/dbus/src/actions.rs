use derive_more::derive::Display;

use super::notification::Notification;

pub enum Action {
    Show(Box<Notification>),
    ShowLast, // NOTE: consider removing this
    Close(Option<u32>),
    CloseAll,
    PlaySound,
}

#[derive(Display)]
#[display("{_variant}")]
pub enum Signal {
    #[display("notification_id: {notification_id}, action_key: {action_key}")]
    ActionInvoked {
        notification_id: u32,
        action_key: String,
    },
    #[display("notification_id: {notification_id}, action_key: {reason}")]
    NotificationClosed {
        notification_id: u32,
        reason: ClosingReason,
    },
}

#[derive(Display)]
pub enum ClosingReason {
    Expired,
    DismissedByUser,
    CallCloseNotification,
    Undefined,
}

impl From<ClosingReason> for u32 {
    fn from(value: ClosingReason) -> Self {
        match value {
            ClosingReason::Expired => 1,
            ClosingReason::DismissedByUser => 2,
            ClosingReason::CallCloseNotification => 3,
            ClosingReason::Undefined => 4,
        }
    }
}
