# Noti macro

The crate that provides macros to `noti` application for various cases.
Currently available only two macro:

- ConfigProperty
- GenericBuilder

Below I described for what purposes these macro were written and how use them
in `noti` application code.

## ConfigProperty

During developing the `noti` application the `Noti` team found that increasing
config variables attracts high code complexity. And this can easily be casue of
leading to typic bugs due careless.

To avoid this, `Noti` team decided to write a macro that do dirty job and provides
simple and useful function which can help with config variables mess.

The `ConfigProperty` is powerful, allowing having temporary config variables,
inherit them into another variable, use correct type, set default values and
add the way to merge. Here below we listed possible actions with a bit explanation:

- `also_from` - adds another temporary field from which the value can be acheived.
  It can be helpful if you want to provide another way to set value for several fields
  like temporary 'text' property for 'title' and 'body' together.

- `mergeable` - marks that field can be merged with the same field from other
  instance of the same structure.

- `default` - marks that field can use default value by Default trait or use other ways set.

- And the last, `use_type`. It was added because of the way by which macro works.

To use them, you should just put into attribute `cfg_prop` keywords that described
above (e.g. `#[cfg_prop(also_from(name = field_name), default)]`). Here is the format of
attribute for each action:

- `#[cfg_prop(also_from(name = field_name))]` where `field_name` should be distinct
  from existing field names. And the field type should be same if you want set the same
  `field_name` for some fields. Also you can enable `mergeable` option for this field,
  like `source_field.merge(field_name)`, using the addition:
  `#[cfg_prop(also_from(name = field_name, mergeable))]`

- `#[cfg_prop(mergeable)]`

- For `default` there are 3 ways to do it:

  - `#[cfg_prop(default)]` - uses `Default` trait.
  - `#[cfg_prop(default(path = path::to::function))]` - uses path to function (and
    don't call the function).
  - `#[cfg_prop(default(expression))]` where expression is any expression that you
    can write. Useful when need to set simple value like `400` for integer or
    `"Hell".to_string()` for String type.

- `#[cfg_prop(use_type(SpecificType))]` where `SpecificType` should be valid and
  have the trait `From<OriginFieldType> for SpecificType`.

> [!NOTE]
> The ConfigProperty macro will try to use default values, even if you didn't attach the 
> `#[cfg_prop(default)]` attribute. It is because need to unwrap `Option<T>` types by
> default values if there is no stored value.

### Usage

Choose or write a struct to which you want to attach the `ConfigProperty` macro.
This struct shuold be as clear as possible. If it can be default value instead of
`Option<T>` type, use it. Let's lookup an example below:

```rust
#[derive(ConfigProperty)]
#[cfg_prop(name(DirtyConfig))]
struct Config {
    #[cfg_prop(also_from(name = temporary_value), default)]
    helpful_value: String,

    // temporary_value: Option<String> will be created in new struct

    #[cfg_prop(use_type(DirtySubconfig), mergeable)]
    complex_value: Subconfig,
}

#[derive(ConfigProperty)]
#[cfg_prop(name(DirtySubconfig))]
struct Subconfig {
    #[cfg_prop(default(path = crate::path::to::default_simple_value))]
    simple_value: i32,

    #[cfg_prop(default(EnumType::Variant))]
    enum_value: EnumType
}

// This function should be reachable
fn default_simple_value() -> i32 { 10 }
```

As you can see there are a lot of thing which are associated to `ConfigProperty`
macro. Firstly, you see `cfg_prop` macro attribute that uses to define properties.
The `#[cfg_prop(name(StructName))]` is important and you always should to set it.
It will produce new struct which have a big differences.

For better understanding imagine that new created structs are mirrored to original
structs but with a lot of boilerplate code. And these new structs are more convenient
to use for deserializing data that can be in various form and it should be sustainable.
For new struct creates `impl`s:

- `fn merge(self, other: Option<Self>) -> Self` - merges the current structure with
  other structures by filling empty config values. It's very helpful if provides the
  same structure but from various sources, and need to fill up with secondary or
  default values.

- `fn unwrap_or_default(self) -> OriginalType` where `OriginalType` is original type
  to which the derive macro was attached. Converts the derived struct which contains
  field types that wrapped by `Option<T>` to original type by unwrapping field values
  and filling default values into them.

Also creates the single `From<DerivedStruct> for OriginalStruct` trait in which just
calls `unwrap_or_default` method that described above.

## GenericBuilder

The `Noti` team figured out that need to use something similar to reflection in
Rust but the programming language doesn't provides ways to do this. So instead
of changing existing struct we decided create a builder that provides way to
set values to fields by string. It's very helpful for future parsing and analyzing
where need to build a bunch of structs from plain text.

By description of issue the `GenericBuilder` derive macro was written (below we'll
call GBuilder instead of GenericBuilder for brevity). Currently this macro have 3
attributes:

For struct:

- `#[gbuilder(name(GBuilderStruct))]` - it's neccessary attribute from which the macro
  can get name for the new builder struct (in this example it will be `GBuilderStruct`).
  Places only before struct definintion.

For fields:

- `#[gbuilder(hidden)]` - hides the fields from users but still uses for building new
  struct. For fields which have `hidden` attribute the methods `contains_field` and
  `set_value` will return `false` and error respectively. Usually sets with default
  attribute.

- There are three ways to set default value and it's same as `#[cfg_prop(default)]`
  that described in [ConfigProperty](#configproperty) section:
  - `#[gbuilder(default)]` - tries to use Default trait.
  - `#[gbuilder(default(path = path::to::function))]` - calls the function by provided path
    (don't call the function!).
  - `#[gbuilder(default(expression))]` - evaluates the provided expression.

The generated GBuilder struct will have 4 methods:

- `GBuilder::new()` - creates new GBuilder instance.
- `GBuilder::contains_field(&self, field_name: &str) -> bool` - checks whether contains
  finding field or not.
- `GBuilder::set_value(&mut self, field_name: &str, value: Value) -> Result<(), Box<dyn Error>>` -
  tries to set value for mentioned field.
- `GBuilder::try_build(self) -> Result<OgirinStruct, String>` - tries to build an OriginStruct
  at which was attached macro of this builder.

At this moment we should tell you that the `Value` type you should implement by yourself.
And this type must implement `TryFrom` trait for unhidden field types. You won't have any
issue when you still use primitive types like integer or `String`. But when you reach the
state when need to implement `TryFrom` for various custom types, you'll understand that
there is a big issue with flexibility. So here you can use `std::any::Any`. Especially if
you use this builder not so frequently and it will be ok.

The example of `Value` type you can see in tests.
