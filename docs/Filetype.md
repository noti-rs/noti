# Filetype

The `Noti` application have the huge feature named **filetype** with which you
can declare custom layout of banners. The `Noti` team introduced the `.noti`
file extension and some logic of layout declaration.

The `Noti` team supposed that you may not like the standard banner layout and
you want to move image to right side, or swap the title and body elements, or
nail the title to upper position instead of centering.

All of these actions you can do with the special **filetype**. To use it, in
your main config file need to put:

```toml
display.layout = "path/to/your/layout/File.noti"
```

Not matter is it global `display` config or per application. Then need to write
your layout in file.

> [!WARNING]
> Since you use the layuot the other configuration of banners will be disabled.
> The borders configuration, paddings, margins, and so on ― that all will be
> disabled. There are also configuration variables cannot be disabled - general
> configuration, colors and something that cannot be related to banner styling.

## Widgets and Type values

The banner layout discusses in **widget** terms, which are powerful and easy
to use. The widget is a generic graphic object that have the single requirement ―
must be able to draw.

Currently available only three simple widgets:

- **FlexContainer**
- **Text**
- **Image**

In addition to widgets, there are also **type values**. Type values are not
renderable objects and use only for configuring the widgets. Some of them are
hidden like `horizontal` and `vertical` that converts into `Direction` type.

Available type values:

- **Alignment**
- **Spacing**
- **Border**

## Types

There is not so much types for properties - only few:

- Literal - continuous sequence of characters unsurrounded double quotes, always
  treated as string
- UInt - unsigned integer
- [Type value](#widgets-and-type-values)

The noti application have smart conversion from `Literal` type and it can be
**boolean**, **color**, **type** or something else if supported.

## Declaration

In one file must be **single** parent widget. Otherwise the `Noti` application
will refuse to further analyzing.

To declare a widget or type value use _constructor with named field_:

```noti
WidgetName(
    property_name = value,
    second_property_name = 3,
    third_property_name = true,
    fourth_property_name = TypeValueName(
        type_value_property_name = 5,
    )
)
```

In the parentheses declares properties for widgets and fields for type values.
They're always separated by comma and here is allowed to use trailing comma.
For some widgets or type value the declaration of properties (or fields) can
be omitted to use default values:

```noti
Image()
```

The parentheses is important for declaring widgets and type values, and you
cannot omit him!

The `FlexContainer` widget is very different because it have `children` in which
you can declare inner widgets. Syntax:

```noti
FlexContainer() {
    WidgetName()
    WidgetName()
}
```

The widgets always surrounded by curly braces. There is no specific separators
between widgets except whitespace.

## Widget properties

Because of the layout is separated from main config file by visual styling, the
widgets have the same options as config. It is not bad thing as you may think because
**all** important config properties you can declare in place and not need to jump
between files.

So, the `Text` widget inherits the [text](./ConfigProperties.md#text) properties,
the `Image` widget inherits the [image](./ConfigProperties.md#image) properties,
the `Border` type value inherits the [border](./ConfigProperties.md#border) properties.

Need to mention that for the `String` type use `Literal` (without double quotes!), for the
`u8`, `u16`, `u32` use `UInt`.

Here we'll describe only few properties that only specific widgets and type values have.
The `-` sign in **Default value** means that it is needed to set your own value because
there is no default value.

### FlexContainer properties

| Property name | Description                                                                                                       | Type        | Default value                                      |
| ------------- | ----------------------------------------------------------------------------------------------------------------- | ----------- | -------------------------------------------------- |
| direction     | Sets the direction of container to arrange elements in row or column. Possible values: `horizontal` or `vertical` | `Literal`   | -                                                  |
| max_width     | Sets the max width of container                                                                                   | `UInt`      | MAX                                                |
| max_height    | Sets the max height of container                                                                                  | `UInt`      | MAX                                                |
| border        | Make a border around container                                                                                    | `Border`    | Default [border](./ConfigProperties.md#border) values |
| spacing       | Treat is `padding` from `display.padding`                                                                         | `Spacing`   | Spacing's default values                           |
| alignment     | Align the container's content by horizontal and vertical                                                          | `Alignment` | -                                                  |

### Text properties

There are two text kinds in banner - for title and body. And need to define it to get correct
result.

| Property name | Description                                                          | Type      | Default value |
| ------------- | -------------------------------------------------------------------- | --------- | ------------- |
| kind          | The kind of text. Possible values: `title` (or `summary`) or `body`. | `Literal` | -             |

### Spacing

Unfortunately, currently unsupported the way to declare values as in CSS.

| Property name | Description            | Type   | Default value |
| ------------- | ---------------------- | ------ | ------------- |
| top           | Set offset from top    | `UInt` | 0             |
| right         | Set offset from right  | `UInt` | 0             |
| bottom        | Set offset from bottom | `UInt` | 0             |
| left          | Set offset from left   | `UInt` | 0             |

### Alignment

For all fields possible values: `start`, `end`, `center`, `space-between`.

| Property name | Description                           | Type      | Default value |
| ------------- | ------------------------------------- | --------- | ------------- |
| horizontal    | Sets the alignment by horizontal axis | `Literal` | -             |
| vertical      | Sets the alignment by vertical axis   | `Literal` | -             |
