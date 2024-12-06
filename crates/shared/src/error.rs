#[derive(Debug, derive_more::Display)]
pub enum ConversionError {
    #[display("The '{field_name}' field is unknown")]
    UnknownField { field_name: String },
    #[display("Provided invalid value. Expected [{expected}], but given [{actual}]")]
    InvalidValue {
        expected: &'static str,
        actual: String,
    },
    #[display("Cannot convert the current type into specific")]
    CannotConvert,
    #[display("The boxed Any type cannot be downcasted into concrete type: {concrete_typename}")]
    AnyDoesntMatchType { concrete_typename: &'static str },
}

impl std::error::Error for ConversionError {}
