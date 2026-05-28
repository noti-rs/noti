use derive_more::derive::Display;

use crate::notification::ScheduledNotification;

use super::notification::Notification;

/// Represents an action to perform on notifications, following the freedesktop notification specification.
pub enum Action {
    /// Shows a new notification by sending it to the notification server.
    Show(Box<Notification>),

    /// Schedules a notification to be shown at a later time.
    Schedule(ScheduledNotification),

    /// Closes a specific notification by its ID.
    ///
    /// If `None` is provided, the server may ignore the request.
    Close(Option<u32>),

    /// Closes all currently visible notifications.
    CloseAll,
}

/// Represents a D-Bus signal sent from the notification server back to the client.
///
/// These signals are used to inform the application about user interactions or notification state changes.
#[derive(Display)]
#[display("{_variant}")]
pub enum Signal {
    /// Indicates that the user invoked an action on a notification.
    ///
    /// This is sent when a button or action is activated.
    #[display("notification_id: {notification_id}, action_key: {action_key}")]
    ActionInvoked {
        /// The ID of the notification that triggered the action.
        notification_id: u32,

        /// The key identifying which action was invoked.
        action_key: String,
    },

    /// Indicates that a notification was closed.
    ///
    /// This is sent when the notification disappears for any reason
    /// (e.g., timeout, user dismissal, application request).
    #[display("notification_id: {notification_id}, action_key: {reason}")]
    NotificationClosed {
        /// The ID of the notification that was closed.
        notification_id: u32,

        /// The reason why the notification was closed.
        reason: ClosingReason,
    },
}

/// Represents the reason why a notification was closed, as defined by the freedesktop specification.
#[derive(Display)]
pub enum ClosingReason {
    /// The notification expired (timeout reached).
    Expired,

    /// The notification was dismissed by the user.
    DismissedByUser,

    /// The notification was closed by a call to `CloseNotification`.
    CallCloseNotification,

    /// The reason is unknown or not provided by the server.
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
