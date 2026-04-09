use std::collections::HashSet;

use crate::{
    types::WidgetId,
    widget::{animated_visibility::AnimatedVisibilityStyle, container::ContainerStyle, image::ImageStyle, text::TextStyle},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StyleProperty<T> {
    Explicit(T),
    FromClass(T),
    #[default]
    Default,
}

impl<T> StyleProperty<T> {
    pub(crate) fn as_ref(&self) -> StyleProperty<&T> {
        match self {
            StyleProperty::Explicit(val) => StyleProperty::Explicit(val),
            StyleProperty::FromClass(val) => StyleProperty::FromClass(val),
            StyleProperty::Default => StyleProperty::Default,
        }
    }

    pub(crate) fn map<F: FnOnce(&T) -> V, V>(&self, mapper: F) -> StyleProperty<V> {
        match self {
            StyleProperty::Explicit(val) => StyleProperty::Explicit(mapper(val)),
            StyleProperty::FromClass(val) => StyleProperty::FromClass(mapper(val)),
            StyleProperty::Default => StyleProperty::Default,
        }
    }

    pub(crate) fn override_if_higher(&mut self, other: Self) {
        match self {
            StyleProperty::Explicit(_) => (),
            StyleProperty::FromClass(_) => match &other {
                StyleProperty::Explicit(_) | StyleProperty::FromClass(_) => *self = other,
                StyleProperty::Default => (),
            },
            StyleProperty::Default => *self = other,
        }
    }

    pub(crate) fn expect(self, msg: &str) -> T {
        match self {
            StyleProperty::Explicit(v) | StyleProperty::FromClass(v) => v,
            StyleProperty::Default => panic!("{}", msg),
        }
    }

    pub(crate) fn unwrap_or(self, other: T) -> T {
        match self {
            StyleProperty::Explicit(val) | StyleProperty::FromClass(val) => val,
            StyleProperty::Default => other,
        }
    }

    pub(crate) fn unwrap_or_default(self) -> T
    where
        T: Default,
    {
        match self {
            StyleProperty::Explicit(val) | StyleProperty::FromClass(val) => val,
            StyleProperty::Default => Default::default(),
        }
    }

    pub(crate) fn as_option(&self) -> Option<&T> {
        match self {
            StyleProperty::Explicit(val) | StyleProperty::FromClass(val) => Some(val),
            StyleProperty::Default => None,
        }
    }

    pub(crate) fn is_default(&self) -> bool {
        matches!(self, StyleProperty::Default)
    }
}

#[derive(Clone)]
pub(crate) struct StyleInfo {
    /// The secondary visual or layout settings (e.g., alignment or colors).
    style: WidgetStyle,
    subscribers: HashSet<WidgetId>,
}

impl StyleInfo {
    pub(crate) fn new(style: WidgetStyle) -> Self {
        StyleInfo {
            style,
            subscribers: HashSet::new(),
        }
    }

    pub(crate) fn set_style(&mut self, style: WidgetStyle) {
        self.style = style
    }

    pub(crate) fn get_style(&self) -> &WidgetStyle {
        &self.style
    }

    pub(crate) fn add_subscriber<Id>(&mut self, subscriber: Id)
    where
        Id: Into<WidgetId>,
    {
        self.subscribers.insert(subscriber.into());
    }

    pub(crate) fn subscribers(&self) -> impl Iterator<Item = &WidgetId> {
        self.subscribers.iter()
    }
}

/// A collection of specific configuration structs for specialized widgets.
///
/// These structs (like `TextConfiguration`) typically mirror the fields
/// of their corresponding widget. They allow the caller to override
/// or set properties externally through the `CompileCtx`.
#[derive(Debug, Clone)]
pub enum WidgetStyle {
    Text(TextStyle),
    Image(ImageStyle),
    Container(ContainerStyle),
    AnimatedVisibility(AnimatedVisibilityStyle),
    Unknown,
}

/// Internal trait for updating a widget's state via a configuration struct.
///
/// This trait is typically implemented automatically by macros. It
/// allows a widget to ingest a configuration (`C`) and apply its
/// values to the widget's own fields, usually acting as a "patch"
/// that fills in missing or default values.
pub(crate) trait Configure<C> {
    fn configure(&mut self, config: C);
}

impl From<WidgetStyle> for StyleInfo {
    fn from(value: WidgetStyle) -> Self {
        StyleInfo {
            style: value,
            subscribers: HashSet::new(),
        }
    }
}

#[macro_export]
macro_rules! make_style {
    ($name:ident { $($field_name:ident: $val:expr),* $(,)? }) => {
            $name::builder()
                $(.$field_name($val))*
                .build()
    };
}

/// A syntax sugar macro for bridging Configuration structs with Widgets.
///
/// This macro defines a "Configuration" struct and automatically implements
/// the [`Configure`] and [`ToConfig`] traits for the specified target widgets.
///
/// ### Syntax
/// `make_configuration! { [Struct Definition] <<= [Target Widgets] }`
///
/// ### Behavior
/// 1. **Struct Generation:** It defines the configuration struct (e.g., `TextConfiguration`)
///    with all provided fields and derives.
/// 2. **Multi-Target Implementation:** For every widget listed after the `<<=` operator,
///    the macro generates:
///    - An implementation of `Configure<Self>` for the widget, allowing it to
///      ingest these specific fields.
///    - An implementation of `ToConfig<Self>` for the widget, allowing it to
///      export its current state back into this configuration format.
///
/// This allows a single configuration definition to be shared across multiple
/// similar widgets (e.g., both `Container` and `FlexContainer`).
#[macro_export]
macro_rules! make_configuration {
    ($(#[$($attr:tt)+])* $struct_vis:vis struct $struct_name:ident { $($(#[$($field_attr:tt)+])* $field_vis:vis $field_name:ident: $field_type:ty),* $(,)? } <<= $($origin_struct:ident),+) => {
        $(#[$($attr)+])*
        $struct_vis struct $struct_name {
            $(
                $(#[$($field_attr)+])*
                $field_vis $field_name: $field_type,
            )*
        }

        make_configuration! {
            @impl_blocks
            struct_name = $struct_name;
            fields = [ $($field_name),* ];
            origins = [ $($origin_struct),+ ]
        }
    };

    // Base recursion case: the origins is empty
    (
        @impl_blocks
        struct_name = $struct_name:ident;
        fields = [ $($field_name:ident),* ];
        origins = []
    ) => {};

    // Recursion step: take first origin_struct and handle it. For other origin_struct call
    // the same macro.
    (
        @impl_blocks
        struct_name = $struct_name:ident;
        fields = [ $($field_name:ident),* ];
        origins = [ $current_origin:ident $(, $rest_origins:ident)* ]
    ) => {

        impl Configure<$struct_name> for $current_origin {
            fn configure(&mut self, config: $struct_name) {
                $(
                    self.$field_name.override_if_higher(config.$field_name);
                )*
            }
        }

        make_configuration! {
            @impl_blocks
            struct_name = $struct_name;
            fields = [ $($field_name),* ];
            origins = [ $($rest_origins),* ]
        }
    };
}
