use std::{fmt::Display, ops::Deref};

use shared::value::TryFromValue;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Default)]
pub struct WidgetId(u64);

impl Deref for WidgetId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<u64> for WidgetId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl TryFromValue for WidgetId {
    fn try_from_uint(value: usize) -> Result<Self, shared::error::ConversionError> {
        Ok(Self(value as u64))
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Default)]
pub struct WidgetClass(String);

impl Display for WidgetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for WidgetClass {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for WidgetClass {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for WidgetClass {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl TryFromValue for WidgetClass {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(Self(value))
    }
}
