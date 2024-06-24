
use serde::{Deserialize, Serialize};
use std::{fmt::Display, time::Duration};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub expire_timeout: Option<Duration>,
    pub urgency: Urgency,
    pub is_read: bool,
    pub created_at: u64,
}

#[derive(Clone)]
pub enum Action {
    Show(Notification),
    ShowLast,
    Close(Option<u32>),
    CloseAll,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
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

impl Default for Urgency {
    fn default() -> Self {
        Self::Normal
    }
}
