use std::{fmt::Display, ops::Deref};

use shared::value::TryFromValue;

/// A unique, string-based identifier used to address and track a specific
/// widget within the layout tree.
///
/// While the system automatically generates unique identifiers during
/// compilation to ensure every widget is accounted for, providing a
/// manual ID allows you to:
///
/// 1. **Targeted Configuration**: Map external [`WidgetData`] or specific [`WidgetConfig`]
///    structs to a widget without traversing the tree.
/// 2. **Readability & Debugging**: Identify specific widgets easily in
///    logs or tree dumps, as the IDs are human-readable strings.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Default)]
pub struct WidgetId(String);

impl Display for WidgetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl Deref for WidgetId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for WidgetId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for WidgetId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl TryFromValue for WidgetId {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(value.into())
    }
}
