use shared::{error::ConversionError, value::TryFromValue};

/// Determines whether an arbirtrary container arranges its children
/// horizontally or vertically.
#[derive(Clone, Copy)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    #[allow(unused)]
    /// Returns the direction orthogonal to the current one.
    ///
    /// * Horizontal → Vertical  
    /// * Vertical → Horizontal
    ///
    /// Useful when computing cross-axis alignment or spacing.
    pub(crate) fn orthogonalize(&self) -> Direction {
        match self {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        }
    }
}

impl TryFromValue for Direction {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "horizontal" => Direction::Horizontal,
            "vertical" => Direction::Vertical,
            _ => Err(ConversionError::InvalidValue {
                expected: "horizontal or vertical",
                actual: value,
            })?,
        })
    }
}
