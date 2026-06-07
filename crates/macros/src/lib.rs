mod config_property;
mod general;
mod generic_builder;
mod widget_component;

use proc_macro::TokenStream;

macro_rules! propagate_err {
    ($expr:expr) => {
        match $expr {
            Ok(data) => data,
            Err(err) => return err.to_compile_error().into(),
        }
    };
}

pub(crate) use propagate_err;

#[proc_macro_derive(ConfigProperty, attributes(cfg_prop))]
pub fn config_property(item: TokenStream) -> TokenStream {
    config_property::make_derive(item)
}

#[proc_macro_derive(GenericBuilder, attributes(gbuilder))]
pub fn generic_builder(item: TokenStream) -> TokenStream {
    generic_builder::make_derive(item)
}

/// Marks a struct as a widget component.
///
/// *REQUIRES* the `bon` crate, because it heavily depends on the `#[derive(bon::Builder)]` macro!
///
/// This macro reduces a lot of boilerplate code, allowing you to declare a clean and readable struct,
/// and converts it into a functional component for the widget system.
///
/// There are three kinds of widgets: *minimal*, *standard*, and *container*. Each of them introduces
/// additional struct fields and methods to help with measurement and drawing.
///
/// * `minimal` — adds only three fields: `id`, `key`, and `class`. Automatically implements
///   the `WidgetInformation` trait.
///
/// * `standard` (default) — adds fields from the `minimal` kind + the `margin` field. Automatically implements
///   the `WidgetInformation` trait and generates the `self.outer_spacing()` method.
///
/// * `container` — adds fields from the `standard` kind + `padding`, `border`, `background_color`, and
///   `alignment` properties. Automatically implements the `WidgetInformation` trait and generates
///   three methods: `self.inner_spacing()`, `self.outer_spacing()`, and `self.total_spacing()`.
///
/// To pick the correct kind for a widget, decide how the concrete widget should behave:
///
/// 1. If it just adds a property or changes something but does nothing in measurement, then it's a
///    *minimal* widget.
///
/// 2. If the widget is a leaf in the widget tree that draws related content, then it's *standard*.
///
/// 3. If the widget groups child widgets or places them according to specific rules, then it's a
///    *container*.
///
/// Syntax: `#[widget(kind = standard)]`.
///
/// Some fields of a widget can be styles that change its look. In the standard way, you would
/// declare them like this:
///
/// ```rs
/// #[builder(with = |v: u16| StyleProperty::Explicit(v), default)]
/// font_size: StyleProperty<u16>,
/// ```
///
/// Instead of this, you can mark it as a style:
///
/// ```rs
/// #[style]
/// font_size: u16,
/// ```
///
/// The macro converts this into the boilerplate code shown above. You can verify this by hovering
/// over a field to check its actual type. This allows you to focus on the actual types without
/// cluttering the code with property wrappers.
///
/// NOTE: Style fields are optional for the builder by default due to the `default` key in `#[builder]`
/// attributes. If you want to require the user to provide a field, add the `required` key: `#[style(required)]`.
/// This will remove the `default` key.
///
/// Also, you may want to generate a separate style structure that copies all style fields, which is
/// reasonable if this style struct is related only to this specific widget. For this case, use
/// `#[make_widget_style(StructName, derive(bon::Builder))]`:
///
/// ```rs
/// #[widget]
/// #[make_widget_style(SpacerStyle, derive(bon::Builder))]
/// #[derive(bon::Builder)]
/// struct Spacer {
///     // Custom fields can be added here
/// }
/// ```
///
/// Since the default widget kind is `standard`, it automatically adds the `margin` field (which is a style field).
/// As a result, the generated code will look like this:
///
/// ```rs
/// #[derive(bon::Builder)]
/// struct Spacer {
///     // minimal fields (id, key, class)...
///     
///     // Automatically injected by the "standard" kind:
///     #[builder(with = |v: Spacing| StyleProperty::Explicit(v), default)]
///     margin: StyleProperty<Spacing>,
/// }
///
/// #[derive(bon::Builder)]
/// struct SpacerStyle {
///     // Automatically mirrored and modified for the style struct:
///     #[builder(with = |v: Spacing| StyleProperty::FromClass(v), default)]
///     margin: StyleProperty<Spacing>,
/// }
/// ```
///
/// In case of using a single style for more than one widget, explicitly declare the style struct with
/// `#[widget_style]`. See more in [widget_style].
///
/// Any widget can have callbacks. To add a callback, use the `#[callback(name: T -> R)]` attribute.
/// It offers a highly flexible declaration syntax. See the examples below:
///
/// ```rs
/// #[widget]
/// #[callback(on_finish)]                   // Callback<(), ()> — no arguments, no return type
/// #[callback(on_click: ClickAction)]       // Callback<ClickAction, ()> — input argument only
/// #[callback(on_hover: () -> Propagation)] // Callback<(), Propagation> — return type only
/// #[callback(on_something: A -> B)]        // Callback<A, B> — both input and return types
/// #[derive(bon::Builder)]
/// struct SomeWidget { /*...*/ }
///
/// // This generates the on_finish, on_click, on_hover, and on_something callback fields,
/// // which you can invoke if they are present.
/// ```
#[proc_macro_attribute]
pub fn widget(attributes: TokenStream, item: TokenStream) -> TokenStream {
    widget_component::make_widget(item, attributes)
}

/// Marks a struct as a widget style.
///
/// *REQUIRES* the `bon` crate, because it heavily depends on the `#[derive(bon::Builder)]` macro!
///
/// Like `#[widget]`, it supports the same kinds: `minimal`, `standard`, and `container`.
/// Depending on the selected kind, the corresponding fields will be automatically injected:
///
/// * `minimal` — nothing to add.
///
/// * `standard` (default) — adds the `margin` field.
///
/// * `container` — adds fields from the `standard` kind + `padding`, `border`, `background_color`, and
///   `alignment` properties.
///
/// This macro takes each field from the struct and converts it into a style property using the following template:
///
/// ```rs
/// // Original field:
/// field: T,
///
/// // Converted field under the hood:
/// #[builder(with = |v: T| StyleProperty::FromClass(v), default)]
/// field: StyleProperty<T>,
/// ```
///
/// A standalone style struct is not very useful on its own; it must be bound to target widgets.
/// For each specified target widget, this macro automatically implements the `Configure` trait,
/// allowing the widget to consume values from the style struct without any boilerplate code.
///
/// Example:
///
/// ```rs
/// #[widget_style(kind = standard, targets(SomeWidget, AnotherWidget))]
/// #[derive(bon::Builder)]
/// struct SomeWidgetStyle {
///     first_field: u16,
///     second_field: String,
/// }
///
/// // Then, inside the widget logic, you can easily use the `Configure` trait:
/// self.configure(some_widget_style); // Where `self` is an instance of `SomeWidget`
/// ```
#[proc_macro_attribute]
pub fn widget_style(attributes: TokenStream, item: TokenStream) -> TokenStream {
    widget_component::make_widget_style(item, attributes)
}
