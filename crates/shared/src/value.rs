use crate::error::ConversionError;

#[derive(Debug)]
pub enum Value {
    UInt(usize),
    String(String),
    Any(Box<dyn std::any::Any + Send + Sync>),
}

pub trait TryFromValue: Sized + 'static {
    fn try_from(value: Value) -> Result<Self, ConversionError> {
        match value {
            Value::String(string) => Self::try_from_string(string),
            Value::UInt(uint) => Self::try_from_uint(uint),
            Value::Any(dyn_value) => dyn_value.try_downcast(),
        }
    }

    fn try_from_string(_value: String) -> Result<Self, ConversionError> {
        Err(ConversionError::CannotConvert)
    }

    fn try_from_uint(_value: usize) -> Result<Self, ConversionError> {
        Err(ConversionError::CannotConvert)
    }
}

pub trait TryDowncast {
    fn try_downcast<T: 'static>(self) -> Result<T, ConversionError>;
}

impl TryDowncast for Box<dyn std::any::Any + Send + Sync> {
    fn try_downcast<T: 'static>(self) -> Result<T, ConversionError> {
        Ok(*self
            .downcast()
            .map_err(|_| ConversionError::AnyDoesntMatchType {
                concrete_typename: std::any::type_name::<T>(),
            })?)
    }
}

macro_rules! impl_from_for_value {
    ($type:ty => $variant:path) => {
        impl From<$type> for Value {
            fn from(value: $type) -> Self {
                $variant(value)
            }
        }
    };
}

impl_from_for_value!(usize => Value::UInt);
impl_from_for_value!(String => Value::String);
impl_from_for_value!(Box<dyn std::any::Any + Send + Sync> => Value::Any);

impl TryFromValue for usize {
    fn try_from_uint(value: usize) -> Result<Self, ConversionError> {
        Ok(value)
    }
}

impl TryFromValue for String {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(value)
    }
}

impl<T: 'static> TryFromValue for Vec<T> {}

macro_rules! impl_try_from_value_for_uint {
    ($($type:path),*) => {
        $(
            impl TryFromValue for $type {
                fn try_from_uint(value: usize) -> Result<Self, ConversionError> {
                    Ok(value.clamp(<$type>::MIN as usize, <$type>::MAX as usize) as $type)
                }
            }
        )*
    };
}

impl_try_from_value_for_uint!(u8, u16, u32, i8, i16, i32);

impl TryFromValue for bool {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        value
            .parse::<bool>()
            .map_err(|_| ConversionError::InvalidValue {
                expected: "true or false",
                actual: value,
            })
    }
}
