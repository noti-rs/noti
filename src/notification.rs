use std::{fmt::Display, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct Notification {
    pub id: u32,
    pub name: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub expire_timeout: Option<Duration>,
    pub urgency: Urgency,
    pub is_read: bool,
    pub created_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

impl Display for Urgency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

impl From<u64> for Urgency {
    fn from(value: u64) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Normal,
            2 => Self::Critical,
            _ => Self::default(),
        }
    }
}

impl Default for Urgency {
    fn default() -> Self {
        Self::Normal
    }
}
