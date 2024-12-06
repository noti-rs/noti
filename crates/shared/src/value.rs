use crate::error::ConversionError;

#[derive(Debug)]
pub enum Value {
    UInt(usize),
    String(String),
    Any(Box<dyn std::any::Any>),
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
impl_from_for_value!(Box<dyn std::any::Any> => Value::Any);

impl TryFrom<Value> for usize {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::UInt(val) => Ok(val),
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(val) => Ok(val),
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

impl<T: 'static> TryFrom<Value> for Vec<T> {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

pub trait TryDowncast {
    fn try_downcast<T: 'static>(self) -> Result<T, ConversionError>;
}

impl TryDowncast for Box<dyn std::any::Any> {
    fn try_downcast<T: 'static>(self) -> Result<T, ConversionError> {
        Ok(*self
            .downcast()
            .map_err(|_| ConversionError::AnyDoesntMatchType {
                concrete_typename: std::any::type_name::<T>(),
            })?)
    }
}
