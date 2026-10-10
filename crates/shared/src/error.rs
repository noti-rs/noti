use crate::value::Value;

/// Represents errors that can occur during conversion from a generic [`Value`] type
/// into a concrete Rust type.
///
/// This type is used throughout the runtime type system to handle failures when
/// converting, downcasting, or interpreting a [`Value`]. Each variant provides
/// context for why the conversion failed, which helps in debugging or handling
/// errors gracefully.
///
/// # Usage
/// Conversion functions such as [`TryFromValue::try_from`] and
/// [`TryDowncast::try_downcast`] return this type when a conversion cannot be
/// performed. Callers can match on the variants to handle specific error cases.
#[derive(Debug, derive_more::Display)]
pub enum ConversionError {
    /// Field name was unknown in the conversion context.
    #[display("The '{field_name}' field is unknown")]
    UnknownField { field_name: String, value: Value },

    /// The constructor used for conversion is unsupported.
    #[display("Unsupported constructor")]
    UnsupportedConstructor,

    /// The value provided does not match the expected type or format.
    #[display("Provided invalid value. Expected [{expected}], but given [{actual}]")]
    InvalidValue {
        expected: &'static str,
        actual: String,
    },

    /// Conversion is not possible for the given type.
    #[display("Cannot convert the current type into specific")]
    CannotConvert,

    /// Downcasting from a boxed [`Any`] failed due to type mismatch.
    #[display("The boxed Any type cannot be downcasted into concrete type: {concrete_typename}")]
    AnyDoesntMatchType { concrete_typename: &'static str },
}

impl std::error::Error for ConversionError {}
