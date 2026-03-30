use std::path::PathBuf;

use shared::text::Text;

use crate::{
    image::ImageData,
    widget::{
        container::ContainerConfiguration, image::ImageConfiguration, text::TextConfiguration,
    },
};

/// A container for the external state and settings tied to a specific widget.
///
/// This struct acts as a "mailbox" for a widget. During the compilation
/// phase, a widget uses its ID to look up this struct. It may contain
/// the raw content to display (`data`), the visual settings (`config`),
/// or both.
pub struct AssociatedData {
    /// The primary content for the widget (e.g., text strings or image paths).
    pub data: Option<WidgetData>,
    /// The secondary visual or layout settings (e.g., alignment or colors).
    pub config: Option<WidgetConfig>,
}

/// A strictly defined set of content types that widgets can display.
///
/// By using an enum instead of an open-ended trait, the engine ensures
/// type safety and prevents "obscure" data injection. Each variant
/// maps to a specific widget's needs—for example, a `Text` widget
/// will only ever look for the `Text` variant.
#[derive(Debug, Clone)]
pub enum WidgetData {
    /// Content for text-based widgets.
    Text(Text),
    /// Raw byte data for an image.
    ImageData(ImageData),
    /// A filesystem path to an image file.
    ImagePath(PathBuf),
    /// Instructions for looking up a system icon.
    Icon {
        name: String,
        theme: String,
        sizes: Vec<u16>,
    },
}

/// A collection of specific configuration structs for specialized widgets.
///
/// These structs (like `TextConfiguration`) typically mirror the fields
/// of their corresponding widget. They allow the caller to override
/// or set properties externally through the `CompileCtx`.
#[derive(Debug, Clone)]
pub enum WidgetConfig {
    Text(TextConfiguration),
    Image(ImageConfiguration),
    Container(ContainerConfiguration),
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

/// Internal trait for extracting a configuration snapshot from a widget.
///
/// This allows a widget to "export" its current state into a
/// configuration struct (`C`). This is useful for saving a widget's
/// current setup or passing its parameters to other parts of the
/// layout engine.
pub(crate) trait ToConfig<C> {
    fn to_config(&self) -> C;
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
                    self.$field_name.get_or_insert_with(|| config.$field_name);
                )*
            }
        }

        impl ToConfig<$struct_name> for $current_origin {
            fn to_config(&self) -> $struct_name {
                $struct_name {
                    $(
                        $field_name: self.$field_name.clone().unwrap_or_default(),
                    )*
                }
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
