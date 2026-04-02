use shared::{error::ConversionError, value::TryFromValue};

/// Defines how children are placed inside an arbitrary container.
///
/// [`Self::horizontal`] controls alignment along the x-axis, and
/// [`Self::vertical`] along the y-axis. These values affect the
/// final placement of child widgets when there is extra free space
/// remaining after compilation.
#[derive(macros::GenericBuilder, Debug, Default, Clone)]
#[gbuilder(name(AlignmentGBuilder), derive(Clone), constructor)]
pub struct Alignment {
    #[gbuilder(aliases(diagonal))]
    pub horizontal: Position,

    #[gbuilder(aliases(diagonal))]
    pub vertical: Position,
}

impl Alignment {
    pub fn new(horizontal: Position, vertical: Position) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

impl TryFromValue for Alignment {}

/// The position strategy used by [`Alignment`] to place children.
#[derive(Debug, Default, Clone)]
pub enum Position {
    /// Aligns children at the start of the axis.
    Start,

    /// Centers children (default).
    #[default]
    Center,

    /// Aligns children at the end of the axis.
    End,

    /// Distributes children evenly, adding spacing between them to fill available space.
    SpaceBetween,
}

impl TryFromValue for Position {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "start" => Position::Start,
            "center" => Position::Center,
            "end" => Position::End,
            "space-between" | "space_between" => Position::SpaceBetween,
            _ => Err(ConversionError::InvalidValue {
                expected: "start, center, end, space-between or space_between",
                actual: value,
            })?,
        })
    }
}

impl Position {
    /// Computes the starting offset for an element of a given width
    /// relative to the available space, based on the positioning strategy.
    ///
    /// This is the core helper for placing children at `Start`, `Center`,
    /// `End`, or evenly with `SpaceBetween`.
    pub fn get_start(&self, width: f32, element_width: f32) -> f32 {
        match self {
            Position::Start | Position::SpaceBetween => 0.0,
            Position::Center => width / 2.0 - element_width / 2.0,
            Position::End => width - element_width,
        }
    }
}
