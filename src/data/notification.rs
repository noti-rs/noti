use super::{image::ImageData, text::Text};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use zbus::zvariant::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: Text,
    pub body: Text,
    pub expire_timeout: Timeout,
    pub hints: Hints,
    pub is_read: bool,
    pub created_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Hints {
    pub urgency: Urgency,
    pub category: Category,
    pub desktop_entry: Option<String>,
    pub image_data: Option<ImageData>,
    pub image_path: Option<String>,
    pub resident: Option<bool>,
    pub sound_file: Option<String>,
    pub sound_name: Option<String>,
    pub suppress_sound: Option<bool>,
    pub transient: Option<bool>,
    pub coordinates: Option<Coordinates>,
}

impl Hints {
    fn get_hint_value<'a, T>(hints: &'a HashMap<&'a str, Value<'a>>, key: &str) -> Option<T>
    where
        T: TryFrom<&'a Value<'a>>,
    {
        hints.get(key).and_then(|val| T::try_from(val).ok())
    }
}

impl From<&HashMap<&str, Value<'_>>> for Hints {
    fn from(hints: &HashMap<&str, Value>) -> Self {
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
            .find_map(|&name| hints.get(name))
            .and_then(ImageData::from_hint);

        let image_path = Self::get_hint_value(hints, "image-path");
        let desktop_entry = Self::get_hint_value(hints, "desktop-entry");
        let sound_file = Self::get_hint_value(hints, "sound-file");
        let sound_name = Self::get_hint_value(hints, "sound-name"); // NOTE: http://0pointer.de/public/sound-naming-spec.html
        let resident = Self::get_hint_value(hints, "resident");
        let suppress_sound = Self::get_hint_value(hints, "suppress-sound");
        let transient = Self::get_hint_value(hints, "transient");
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
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Category {
    Device(CategoryEvent),
    Email(CategoryEvent),
    InstantMessage(CategoryEvent),
    Network(CategoryEvent),
    Presence(CategoryEvent),
    Transfer(CategoryEvent),
    #[default]
    None,
}

impl Category {
    pub fn from_hint(hint: &Value<'_>) -> Option<Category> {
        String::try_from(hint)
            .ok()
            .and_then(|s| Some(Self::from(s.as_str())))
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
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CategoryEvent {
    Generic,
    Added,
    Removed,
    Arrived,
    Bounced,
    Received,
    Error,
    Connected,
    Disconnected,
    Offline,
    Online,
    Complete,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub enum Timeout {
    Millis(u32),
    Never,
    #[default]
    Configurable,
}

impl Display for Timeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}"))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    pub fn from_hint(hint: &Value<'_>) -> Option<Self> {
        u32::try_from(hint)
            .ok()
            .and_then(|val| Some(Self::from(val)))
    }
}

impl Display for Urgency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

impl From<u32> for Urgency {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Normal,
            2 => Self::Critical,
            _ => Self::default(),
        }
    }
}

impl From<&str> for Urgency {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "low" => Self::Low,
            "normal" => Self::Normal,
            "critical" => Self::Critical,
            _ => Self::default(),
        }
    }
}
