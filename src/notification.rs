use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub expire_timeout: Timeout,
    pub urgency: Urgency,
    pub image_data: Option<ImageData>,
    pub image_path: Option<String>,
    pub is_read: bool,
    pub created_at: u64,
}

impl std::fmt::Debug for Notification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notification")
            .field("id", &self.id)
            .field("app_name", &self.app_name)
            .field("app_icon", &self.app_icon)
            .field("summary", &self.summary)
            .field("body", &self.body)
            .field("expire_timeout", &self.expire_timeout)
            .field("urgency", &self.urgency)
            .field(
                "image_data",
                &self.image_data.as_ref().map(|_data| "Vec<u8> [ ... ]"),
            )
            .field("image_path", &self.image_path)
            .field("is_read", &self.is_read)
            .field("created_at", &self.created_at)
            .finish()
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageData {
    // Width of image in pixels
    pub width: i32,

    // Height of image in pixels
    pub height: i32,

    // Distance in bytes between row starts
    pub rowstride: i32,

    // Whether the image has an alpha channel
    pub has_alpha: bool,

    // Must always be 8
    pub bits_per_sample: i32,

    // If has_alpha is TRUE, must be 4, otherwise 3
    pub channels: i32,

    // The image data, in RGB byte order
    pub data: Vec<u8>,
}

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
