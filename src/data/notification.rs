use serde::{Deserialize, Serialize};
use std::fmt::Display;
use zbus::zvariant::{Array, Structure, Value};

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
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

impl ImageData {
    pub fn from_hint(hint: &Value<'_>) -> Option<Self> {
        Structure::try_from(hint)
            .ok()
            .and_then(Self::from_structure)
    }

    fn from_structure(image_structure: Structure) -> Option<Self> {
        let fields = image_structure.fields();
        if fields.len() < 7 {
            return None;
        }

        let image_raw = match Array::try_from(&fields[6]) {
            Ok(array) => array,
            Err(_) => return None,
        };

        let width = i32::try_from(&fields[0]).ok()?;
        let height = i32::try_from(&fields[1]).ok()?;
        let rowstride = i32::try_from(&fields[2]).ok()?;
        let has_alpha = bool::try_from(&fields[3]).ok()?;
        let bits_per_sample = i32::try_from(&fields[4]).ok()?;
        let channels = i32::try_from(&fields[5]).ok()?;

        let data = image_raw
            .iter()
            .map(|value| u8::try_from(value).expect("expected u8"))
            .collect::<Vec<_>>();

        Some(ImageData {
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        })
    }
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rowstride", &self.rowstride)
            .field("has_alpha", &self.has_alpha)
            .field("bits_per_sample", &self.bits_per_sample)
            .field("channels", &self.channels)
            .field("data", &"Vec<u8> [...]")
            .finish()
    }
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
