use std::{fmt::Display, ops::Deref};

use shared::value::TryFromValue;

/// A unique, auto-generated identifier for the widget.
/// IDs start at 1; a value of 0 represents an "unknown" or "empty" state.
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

/// A unique access key for external control. While the system defaults to
/// the last instance found in the tree if keys are duplicated, you should
/// ensure each key is unique to avoid selection conflicts.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct WidgetKey(String);

impl Deref for WidgetKey {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for WidgetKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for WidgetKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl TryFromValue for WidgetKey {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(Self(value))
    }
}

/// A stylistic grouping similar to CSS classes. Use this
/// to apply shared styles to multiple widgets of the same
/// category simultaneously.
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
