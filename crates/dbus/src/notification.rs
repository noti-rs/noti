use super::image::ImageData;
use derive_more::Display;
use shared::text::Text;
use std::{cmp::Ordering, collections::HashMap};
use zbus::zvariant::Value;

/// Represents a notification sent through D-Bus.
#[derive(Debug)]
pub struct Notification {
    /// Unique identifier for the notification, assigned by the D-Bus notification server.
    pub id: u32,

    /// Name of the application sending the notification.
    pub app_name: String,

    /// Icon associated with the application (can be empty).
    pub app_icon: String,

    /// Short summary or title of the notification.
    pub summary: Text,

    /// Main content of the notification.
    pub body: Text,

    /// Time (in milliseconds) after which the notification expires.
    pub expire_timeout: Timeout,

    /// Optional hints that provide additional information for the notification (e.g., urgency, category, image data).
    pub hints: Hints,

    /// List of actions associated with the notification (buttons or commands the user can trigger).
    pub actions: Vec<NotificationAction>,

    /// Indicates whether the notification has been marked as read.
    pub is_read: bool,

    /// Timestamp (Unix epoch in milliseconds) when the notification was created.
    pub created_at: u64,
}

#[derive(Debug)]
pub struct ScheduledNotification {
    pub time: String,
    pub data: Box<Notification>,
}

impl Ord for ScheduledNotification {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for ScheduledNotification {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScheduledNotification {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for ScheduledNotification {}

#[derive(Debug, Clone)]
pub struct Hints {
    /// The urgency level.
    pub urgency: Urgency,

    /// The type of notification this is.
    pub category: Category,

    /// This specifies the name of the desktop filename representing the calling program.
    /// This should be the same as the prefix used for the application's .desktop file.
    /// An example would be "rhythmbox" from "rhythmbox.desktop".
    /// This can be used by the daemon to retrieve the correct icon for the application, for logging purposes, etc.
    pub desktop_entry: Option<String>,

    /// Raw data image format.
    pub image_data: Option<ImageData>,

    /// Alternative way to define the notification image
    pub image_path: Option<String>,

    /// When set the server will not automatically remove the notification when an action has been invoked.
    /// The notification will remain resident in the server until it is explicitly removed by the user or by the sender.
    /// This hint is likely only useful when the server has the "persistence" capability.
    pub resident: Option<bool>,

    /// The path to a sound file to play when the notification pops up.
    pub sound_file: Option<String>,

    /// A themeable named sound from the freedesktop.org sound naming specification to play when the notification pops up.
    /// Similar to icon-name, only for sounds. An example would be "message-new-instant".
    pub sound_name: Option<String>,

    /// Causes the server to suppress playing any sounds, if it has that ability.
    /// This is usually set when the client itself is going to play its own sound.
    pub suppress_sound: Option<bool>,

    /// When set the server will treat the notification as transient and by-pass the server's persistence capability, if it should exist.
    pub transient: Option<bool>,

    /// Specifies the X and Y location on the screen that the notification should point to.
    pub coordinates: Option<Coordinates>,

    /// When set, a server that has the "action-icons" capability will attempt to interpret any action identifier as a named icon.
    /// The localized display name will be used to annotate the icon for accessibility purposes.
    /// The icon name should be compliant with the Freedesktop.org Icon Naming Specification.
    pub action_icons: Option<bool>,

    /// Specifies the time to schedule the notification to be shown.
    pub schedule: Option<String>,
}

impl Hints {
    fn get_hint_value<'a, T>(hints: &'a HashMap<&'a str, Value<'a>>, key: &str) -> Option<T>
    where
        T: TryFrom<&'a Value<'a>>,
    {
        hints.get(key).and_then(|val| T::try_from(val).ok())
    }
}

impl From<HashMap<&str, Value<'_>>> for Hints {
    fn from(mut hints: HashMap<&str, Value>) -> Self {
        let urgency = hints
            .get("urgency")
            .and_then(Urgency::from_hint)
            .unwrap_or_default();

        let category = hints
            .get("category")
            .and_then(Category::from_hint)
            .unwrap_or_default();

        let image_data = ["image-data", "image_data", "icon-data", "icon_data"]
            .iter()
            .find_map(|&name| hints.remove(name))
            .and_then(ImageData::from_hint);

        let image_path = Self::get_hint_value(&hints, "image-path");
        let desktop_entry = Self::get_hint_value(&hints, "desktop-entry");
        let sound_file = Self::get_hint_value(&hints, "sound-file");
        let sound_name = Self::get_hint_value(&hints, "sound-name"); // NOTE: http://0pointer.de/public/sound-naming-spec.html
        let resident = Self::get_hint_value(&hints, "resident");
        let suppress_sound = Self::get_hint_value(&hints, "suppress-sound");
        let transient = Self::get_hint_value(&hints, "transient");
        let action_icons = Self::get_hint_value(&hints, "action_icons");
        let schedule = Self::get_hint_value(&hints, "schedule");
        let coordinates = Coordinates::from_hints(&hints);

        Hints {
            urgency,
            category,
            image_data,
            image_path,
            desktop_entry,
            resident,
            sound_file,
            sound_name,
            suppress_sound,
            transient,
            coordinates,
            action_icons,
            schedule,
        }
    }
}

#[derive(Debug)]
/// Represents a single action attached to a notification, following the freedesktop specification.
pub struct NotificationAction {
    /// A unique key that identifies the action.
    ///
    /// This key is sent back to the notification server when the user activates the action.
    #[allow(unused)]
    action_key: String,

    /// A human-readable, localized label for the action.
    ///
    /// This is shown to the user as the button or menu item text.
    #[allow(unused)]
    localized_string: String,
}

impl NotificationAction {
    pub fn from_vec(vec: &[&str]) -> Vec<Self> {
        let mut actions: Vec<Self> = Vec::new();

        if vec.len() >= 2 {
            for chunk in vec.chunks(2) {
                actions.push(Self {
                    action_key: chunk[0].into(),
                    localized_string: chunk[1].into(),
                });
            }
        }

        actions
    }
}

#[derive(Debug, Clone)]
pub struct Coordinates {
    pub x: i32,
    pub y: i32,
}

impl Coordinates {
    fn from_hints(hints: &HashMap<&str, Value>) -> Option<Self> {
        let x = hints.get("x").and_then(|val| i32::try_from(val).ok());
        let y = hints.get("y").and_then(|val| i32::try_from(val).ok());

        match (x, y) {
            (Some(x), Some(y)) => Some(Self { x, y }),
            _ => None,
        }
    }
}

/// Represents the category of a notification, following the freedesktop notification specification.
///
/// Categories help classify notifications so that the server or compositor can handle
/// them appropriately (e.g., show with different priority or style).
#[derive(Debug, Clone, Default)]
pub enum Category {
    /// Notifications related to device events (e.g., battery low, hardware change).
    Device(CategoryEvent),

    /// Notifications related to email events (e.g., new mail).
    Email(CategoryEvent),

    /// Notifications for instant messaging (e.g., chat messages).
    InstantMessage(CategoryEvent),

    /// Notifications about network events (e.g., connectivity changes).
    Network(CategoryEvent),

    /// Notifications indicating user presence changes (e.g., online/offline status).
    Presence(CategoryEvent),

    /// Notifications about file or data transfer events.
    Transfer(CategoryEvent),

    /// Fallback when the category is unknown or unspecified.
    #[default]
    Unknown,
}

impl Category {
    pub fn from_hint(hint: &Value<'_>) -> Option<Category> {
        String::try_from(hint).ok().map(|s| Self::from(s.as_str()))
    }
}

impl From<&str> for Category {
    fn from(value: &str) -> Self {
        match value {
            "device" => Self::Device(CategoryEvent::Generic),
            "device.added" => Self::Device(CategoryEvent::Added),
            "device.removed" => Self::Device(CategoryEvent::Removed),
            "device.error" => Self::Device(CategoryEvent::Error),
            "email" => Self::Email(CategoryEvent::Generic),
            "email.arrived" => Self::Email(CategoryEvent::Arrived),
            "email.bounced" => Self::Email(CategoryEvent::Bounced),
            "im" => Self::InstantMessage(CategoryEvent::Generic),
            "im.received" => Self::InstantMessage(CategoryEvent::Received),
            "im.error" => Self::InstantMessage(CategoryEvent::Error),
            "network" => Self::Network(CategoryEvent::Generic),
            "network.connected" => Self::Network(CategoryEvent::Connected),
            "network.disconnected" => Self::Network(CategoryEvent::Disconnected),
            "network.error" => Self::Network(CategoryEvent::Error),
            "presence" => Self::Presence(CategoryEvent::Generic),
            "presence.online" => Self::Presence(CategoryEvent::Online),
            "presence.offline" => Self::Presence(CategoryEvent::Offline),
            "transfer" => Self::Transfer(CategoryEvent::Generic),
            "transfer.complete" => Self::Transfer(CategoryEvent::Complete),
            "transfer.error" => Self::Transfer(CategoryEvent::Error),
            _ => Self::Unknown,
        }
    }
}

/// Represents a specific event within a notification category, following the freedesktop specification.
///
/// These events allow for more fine-grained classification of notifications,
/// helping the server decide how to present them (e.g., different urgency or icon).
#[derive(Debug, Clone)]
pub enum CategoryEvent {
    /// A generic event that does not fit into any of the more specific types.
    Generic,

    /// Indicates that something was added (e.g., a new device).
    Added,

    /// Indicates that something was removed (e.g., a device disconnected).
    Removed,

    /// Indicates that something has arrived (e.g., new message, new mail).
    Arrived,

    /// Indicates a bounce event (e.g., message failed to send).
    Bounced,

    /// Indicates that something has been received (e.g., file transfer completed).
    Received,

    /// Indicates an error occurred.
    Error,

    /// Indicates a connection was established.
    Connected,

    /// Indicates a connection was lost or closed.
    Disconnected,

    /// Indicates that something went offline.
    Offline,

    /// Indicates that something came online.
    Online,

    /// Indicates that a process or transfer has completed.
    Complete,
}

/// Represents the timeout of a notification, following the freedesktop notification specification.
///
/// This value determines how long the notification should be displayed.
#[derive(Default, Debug, Clone, Display)]
pub enum Timeout {
    /// A custom timeout in milliseconds.
    ///
    /// The notification server will attempt to close the notification after the specified time.
    /// The server may ignore this value if it enforces its own policy.
    Millis(u32),

    /// Indicates that the notification should never automatically expire.
    ///
    /// The notification will remain visible until the user dismisses it or the application withdraws it.
    Never,

    /// Uses the notification server's default timeout behavior.
    ///
    /// This is usually preferred unless a specific behavior is required.
    #[default]
    Configurable,
}

impl From<i32> for Timeout {
    fn from(value: i32) -> Self {
        match value {
            t if t < -1 => todo!(),
            0 => Self::Never,
            -1 => Self::Configurable,
            t => Self::Millis(t as u32),
        }
    }
}

/// Represents the urgency level of a notification, following the freedesktop notification specification.
///
/// Urgency levels are used by the notification server to determine how the notification
/// should be presented (e.g., style, priority, or persistence).
#[derive(Debug, Clone, Copy, Default, Display, PartialEq, Eq)]
pub enum Urgency {
    /// Low urgency — the notification is not time-sensitive and may be shown less prominently.
    Low,

    /// Normal urgency — the default level for most notifications.
    #[default]
    Normal,

    /// Critical urgency — indicates the notification is very important.
    ///
    /// Such notifications may remain on screen until the user explicitly dismisses them,
    /// depending on the server's implementation.
    Critical,
}

impl Urgency {
    pub fn from_hint(hint: &Value<'_>) -> Option<Self> {
        fn to_urgency<T: Into<Urgency>>(val: T) -> Urgency {
            val.into()
        }

        u8::try_from(hint)
            .map(to_urgency)
            .ok()
            .or_else(|| String::try_from(hint).map(to_urgency).ok())
    }
}

impl From<u8> for Urgency {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Normal,
            2 => Self::Critical,
            _ => Default::default(),
        }
    }
}

impl From<&Urgency> for u8 {
    fn from(value: &Urgency) -> Self {
        match value {
            Urgency::Low => 0,
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        }
    }
}

impl From<&str> for Urgency {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "low" => Self::Low,
            "normal" => Self::Normal,
            "critical" => Self::Critical,
            _ => Default::default(),
        }
    }
}

impl From<String> for Urgency {
    fn from(value: String) -> Self {
        <Self as From<&str>>::from(&value)
    }
}

impl Ord for Urgency {
    fn cmp(&self, other: &Self) -> Ordering {
        Into::<u8>::into(self).cmp(&other.into())
    }
}

impl PartialOrd for Urgency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
