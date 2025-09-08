use crate::error::ConversionError;

/// Represents a generic runtime value.
///
/// This type is used as the basis for dynamic type management at runtime. Unlike
/// languages with built-in runtime reflection, Rust's type system is static and
/// strict. `Value` provides a minimal set of flexible types to allow runtime
/// resolution and injection of values when building complex structures, such as
/// those constructed with a `GenericBuilder`.
#[derive(Debug)]
pub enum Value {
    UInt(usize),
    String(String),

    /// The `Any` variant is boxed to allow storing arbitrary types that cannot be
    /// enumerated statically. This enables runtime downcasting when the concrete
    /// type is not known at compile time.
    Any(Box<dyn std::any::Any>),
}

/// A trait for converting [`Value`] into concrete Rust types.
///
/// This trait provides a standardized way to extract strongly-typed values from
/// the generic [`Value`] enum. It is intended to reduce boilerplate when working
/// with runtime-resolved values in the system.
///
/// # Usage
/// Implementers can override the provided methods to define conversions from
/// specific `Value` variants into their type. By default, `try_from_string` and
/// `try_from_uint` return an error, so you must override them if your type needs
/// to support conversion from `String` or `usize`.
///
/// The trait ensures that conversions either succeed with the expected type or
/// return a [`ConversionError`] describing the failure.
pub trait TryFromValue: Sized + 'static {
    fn try_from_cloned(value: &Value) -> Result<Self, ConversionError>
    where
        Self: Clone,
    {
        match value {
            Value::String(string) => Self::try_from_string(string.clone()),
            Value::UInt(uint) => Self::try_from_uint(*uint),
            Value::Any(dyn_value) => dyn_value.try_downcast_ref().cloned(),
        }
    }

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

/// Provides safe downcasting from a generic type to a specific type.
///
/// This trait is used to attempt converting a boxed or reference type into a
/// concrete type `T` at runtime. It is mainly used with [`Value::Any`] to
/// extract the contained type safely, leveraging Rust's `Any` for downcasting.
///
/// If the downcast fails due to a type mismatch, a [`ConversionError`] is returned,
/// allowing the calling code to handle the error gracefully.
pub trait TryDowncast<T: 'static>: Sized {
    /// Attempts to downcast `self` into `T`, consuming `self`.
    fn try_downcast(self) -> Result<T, ConversionError>;

    /// Attempts to get a reference to `T` from `self`.
    fn try_downcast_ref(&self) -> Result<&T, ConversionError>;
}

impl<T: 'static> TryDowncast<T> for Box<dyn std::any::Any> {
    fn try_downcast(self) -> Result<T, ConversionError> {
        Ok(*self
            .downcast()
            .map_err(|_| ConversionError::AnyDoesntMatchType {
                concrete_typename: std::any::type_name::<T>(),
            })?)
    }

    fn try_downcast_ref(&self) -> Result<&T, ConversionError> {
        self.downcast_ref()
            .ok_or_else(|| ConversionError::AnyDoesntMatchType {
                concrete_typename: std::any::type_name::<T>(),
            })
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

impl<T: 'static + Clone> TryFromValue for Vec<T> {}

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
