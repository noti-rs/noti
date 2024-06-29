use super::image::ImageData;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use zbus::zvariant::Str;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub expire_timeout: Timeout,
    pub urgency: Urgency,
    pub category: Category,
    pub image_data: Option<ImageData>,
    pub image_path: Option<String>,
    pub is_read: bool,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Category {
    Device(Option<CategoryEvent>),
    Email(Option<CategoryEvent>),
    Im(Option<CategoryEvent>),
    Network(Option<CategoryEvent>),
    Presence(Option<CategoryEvent>),
    Transfer(Option<CategoryEvent>),
    #[default]
    None,
}

impl From<Str<'_>> for Category {
    fn from(value: Str<'_>) -> Self {
        match value.as_str() {
            "device" => Self::Device(None),
            "device.added" => Self::Device(Some(CategoryEvent::Added)),
            "device.removed" => Self::Device(Some(CategoryEvent::Removed)),
            "device.error" => Self::Device(Some(CategoryEvent::Error)),
            "email" => Self::Device(None),
            "email.arrived" => Self::Device(Some(CategoryEvent::Arrived)),
            "email.bounced" => Self::Device(Some(CategoryEvent::Bounced)),
            "im" => Self::Device(None),
            "im.received" => Self::Device(Some(CategoryEvent::Received)),
            "im.error" => Self::Device(Some(CategoryEvent::Error)),
            "network" => Self::Device(None),
            "network.connected" => Self::Device(Some(CategoryEvent::Connected)),
            "network.disconnected" => Self::Device(Some(CategoryEvent::Disconnected)),
            "network.error" => Self::Device(Some(CategoryEvent::Error)),
            "presence" => Self::Device(None),
            "presence.online" => Self::Device(Some(CategoryEvent::Online)),
            "presence.offline" => Self::Device(Some(CategoryEvent::Offline)),
            "transfer" => Self::Device(None),
            "transfer.complete" => Self::Device(Some(CategoryEvent::Complete)),
            "transfer.error" => Self::Device(Some(CategoryEvent::Error)),
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CategoryEvent {
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
