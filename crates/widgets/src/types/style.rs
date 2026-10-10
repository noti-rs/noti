#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StyleProperty<T>
where
    T: PartialEq,
{
    Explicit(T),
    #[default]
    Default,
}

impl<T> StyleProperty<T>
where
    T: PartialEq,
{
    pub(crate) fn as_ref(&self) -> StyleProperty<&T> {
        match self {
            StyleProperty::Explicit(val) => StyleProperty::Explicit(val),
            StyleProperty::Default => StyleProperty::Default,
        }
    }

    #[allow(unused)]
    pub(crate) fn map<F: FnOnce(&T) -> V, V>(&self, mapper: F) -> StyleProperty<V>
    where
        V: PartialEq,
    {
        match self {
            StyleProperty::Explicit(val) => StyleProperty::Explicit(mapper(val)),
            StyleProperty::Default => StyleProperty::Default,
        }
    }

    pub(crate) fn expect(self, msg: &str) -> T {
        match self {
            StyleProperty::Explicit(v) => v,
            StyleProperty::Default => panic!("{}", msg),
        }
    }

    pub(crate) fn unwrap_or(self, other: T) -> T {
        match self {
            StyleProperty::Explicit(val) => val,
            StyleProperty::Default => other,
        }
    }

    pub(crate) fn unwrap_or_default(self) -> T
    where
        T: Default,
    {
        match self {
            StyleProperty::Explicit(val) => val,
            StyleProperty::Default => Default::default(),
        }
    }

    pub(crate) fn as_option(&self) -> Option<&T> {
        match self {
            StyleProperty::Explicit(val) => Some(val),
            StyleProperty::Default => None,
        }
    }

    #[allow(unused)]
    pub(crate) fn is_default(&self) -> bool {
        matches!(self, StyleProperty::Default)
    }
}
