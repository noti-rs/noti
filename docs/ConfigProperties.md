# Config Properties

The Noti application have a way to be configured by TOML config file which placed
at specific position. Here a priority of positions:

1. `$XDG_CONFIG_HOME/noti/config.toml`
2. `$HOME/.config/noti/config.toml`

Use this file as the specification of configuration properties.

> [!NOTE]
> Don't need to reload the application after changing config properties because it have
> `hot-reload` or `watch-mode`.

## Types

Before of all properties, need to understand a few primitive type. The complex types like array or table will be explained in place.

| Type     | Description                                                                                                                                                                                                          |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bool`   | A boolean value                                                                                                                                                                                                      |
| `u8`     | An unsigned integer of 8 bit                                                                                                                                                                                         |
| `u16`    | An unsigned integer of 16 bit                                                                                                                                                                                        |
| `String` | A string. Usually it used as enumeration                                                                                                                                                                             |
| `[..]`   | Array containing various type. Used as tuple                                                                                                                                                                         |
| `Color`  | The hex value which is started by hashtag (#) and wrapped by doubled quotes (defines as string). It can have three, six or eight symbols which represent RGB (including alpha-channel in 8-symboled hex as opacity). |
| `Path`   | The path to particular file or directory, represented as string (surrounded by double quotes). Currently supported expansion by environment variable and tilde. The relative paths are not yet supported.            |

## Importing

You can move part of configuration values into other files and import them in
main configuration files by keyword `use` and accepts array of `Path`s. Accepts
any valid path except relative, including path that contains environment
variables, starting with tilde.

The syntax:

```toml
use = [
    "$XDG_CONFIG_HOME/noti/other_cfg_file.toml",
    "~/.config/noti/special_cfg_file.toml",
    "/home/bebra/.config/super-sepcial-file.toml",

    # The relative path is not supported
    # "./apps/Spotify.toml"
    # "apps/Spotify.toml"
]
```

> [!TIP]
> Importing the same file more than one time is discouraged and, please, avoid
> this.

### Config priority

Main thought - current configuration file is more prioritized than imported config file.

So, if the same configuration properties declares twice - in current configuration file and
in imported file, the application will pick the value from the **current** configuration file.

And, please, avoid any ambiguity in configuration files. The application will try to merge
these configuration values, but the `Noti` developers won't guarantee that it will work in
that way as you expected. Instead of this, we recommend declare whole thing in one place
and import it. For example, you can declare [themes](#themes) in other files and import them
into main config file.

---

## Property groups

There is four groups of properties:

- [General](#general)
- [Display](#display)
- [Themes](#themes)
- [Apps](#apps)

Each of them belongs to specific idea. So the reading order is not matter. But we recommend you
to go to through from the first one to the last one.

---

## General

The 'general' word means that it applies to application or all banners together. Here a table of possible general properties and below we'll go through all properties.

| Property name | Description                                         | Type                  | Default value |
| :------------ | :-------------------------------------------------- | :-------------------- | :-----------: |
| `font`        | [See desc](#font)                                   | `String`              |  "Noto Sans"  |
| `width`       | The width of banner frame.                          | `u16`                 |      300      |
| `height`      | The height of banner frame.                         | `u16`                 |      150      |
| `anchor`      | [See desc](#anchor)                                 | `String`              |  "top right"  |
| `gap`         | The space size between two banners. Measures in px. | `u8`                  |      10       |
| `offset`      | [See desc](#offset)                                 | `[u8, u8]`            |    [0, 0]     |
| `sorting`     | [See desc](#sorting)                                | `String` or `Sorting` |   "default"   |

### Font

Accepts the font name. It can be separated or not by spaces. The application can use only the font
name which can be used as pattern in `fc-list` command. So you can't use already styled font like
"Noto Sans Bold". But the application internally can load font with needed styles if it is possible.

### Anchor

The anchor of current monitor for current window instance. It means where you want to see appearing
notification banners. The possible values:

- `"top"`
- `"top-left"` or `"top left"`
- `"top-right"` or `"top right"`
- `"left"`
- `"right"`
- `"bottom"`
- `"bottom-left"` or `"bottom left"`
- `"bottom-right"` or `"bottom right"`

### Offset

The offset from edges for window instance. The first value is the offset
by x-axis, the second value - by y-axis.

For example, you picked `"bottom-left"` anchor and `[5, 10]` offset and
it means that the window instance will be placed at bottom-left edge of a
current monitor with offsets by 5 from left edge and by 10 from bottom edge.

### Sorting

The property which set rule of banner sorting. It's very helpful when you
want to place banner with critical urgency at top or bottom.

You can define only a string and the sorting always will be ascending. But
when you want to sort in descending, you have to define a table:

| Property name | Type     | Default value |
| :------------ | :------- | :-----------: |
| by            | `String` |   "default"   |
| ordering      | `String` |  "ascending"  |

Possible values of the `by` property name:

- "default" (alias to "time")
- "time"
- "id" (means the notification id, simple to "time", but in replacement case
  it stays in old place)
- "urgency"

Possible values of the `ordering` property name:

- "ascending" (also possible short name "asc")
- "descending" (also possible short name "desc")

---

## Display

To change the visual styles of banners use `display` table. You cad define
the values of `display` table for all the applications at the same time and
use specific values per application by [app config](#apps).

If you curious about banner layout, please visit [the other document](#BannerLayout.md)
that was made specifically for it.

The display properties affects only and only for a banner, not the window entire.
The currently possible properties of `display` table:

| Property name                  | Description                                         | Type                                                                    | Default value |
| :----------------------------- | :-------------------------------------------------- | :---------------------------------------------------------------------- | :-----------: |
| [layout](./Filetype.md)        | Use the custom layout by providen path              | `Path`                                                                  |   "default"   |
| [theme](#themes)               | Use the [theme](#themes) by name                    | `String`                                                                |       -       |
| [image](#image)                | Image properties                                    | `Image`                                                                 |       -       |
| [padding](#padding-and-margin) | The spacing from the banner's edge to inner content | `u8` or `[u8, u8]` or `[u8, u8, u8]` or `[u8, u8, u8, u8]` or `Spacing` |       0       |
| [border](#border)              | Border properties                                   | `Border`                                                                |       -       |
| [text](#text)                  | Text properties (alias for `title` and `body`)      | `Text`                                                                  |       -       |
| [title](#text)                 | Title text properties                               | `Text`                                                                  |       -       |
| [body](#text)                  | Body text properties                                | `Text`                                                                  |       -       |
| [markup](#markup)              | Enables HTML style markup                           | `bool`                                                                  |     true      |
| [timeout](#timeout)            | Sets the timeout of banner                          | `u16`                                                                   |       0       |

The [layout](./Filetype.md) property should have or `"default"` value or path to file in which
describes layout for banner. You can pass path with environment variables like
`"$XDG_CONFIG_HOME/noti/File.noti"` or use tilde - `"~/.config/noti/File.noti`.

The `Spacing` table:

| Key        | Short description                                                           | Type |
| :--------- | :-------------------------------------------------------------------------- | :--- |
| top        | Spacing from top                                                            | `u8` |
| right      | Spacing from right                                                          | `u8` |
| bottom     | Spacing from bottom                                                         | `u8` |
| left       | Spacing from left                                                           | `u8` |
| vertical   | Spacing from top and bottom together (incompatible with top or bottom keys) | `u8` |
| horizontal | Spacing from left and right together (incompatible with left or right keys) | `u8` |

### Padding and margin

Within scope of this application, the padding and margin have different meaning.
The padding is the spacing from the banner edges for inner elements, it's like giving the content area smaller.
The margin is the spacing from the edges of remaining area and other inner elements.

> [!NOTE]
> If you have issue that the image or the text doesn't show in banner, it's maybe
> because of large value of padding or margins that the content can't fit into
> remaining space.

Here are two ways to declare properties for the padding and the margins:

- [CSS-like](#css-like)
- [Explicit](#explicit)

#### CSS-like

If you familiar with CSS, you know that the padding or the margin can be applied in single row:

```css
body {
  padding: 0 5; /* Applies vertical and horizontal paddings respectively */
  margin: 3 2 5; /* Applies top, horizontal and bottom paddings respectively */
}

main {
  padding: 1 2 3 4; /* Applies top, right, bottom, left paddings respectively */
  margin: 3; /* All-directional margin */
}
```

In the TOML config file you can do it using array:

```toml
# Applies vertical and horizontal paddings respectively
padding = [0, 5]

# Applies top, horizontal and bottom paddings respectively
margin = [3, 2, 5]

# Applies top, right, bottom, left paddings respectively
padding = [1, 2, 3, 4]

# All-directional margin
margin = 3
```

#### Explicit

If you don't like the CSS-like properties, here an explicit way. You can use table instead the array and write directions as keys: top, bottom, right and left.
Also if you wanna apply the same value for top and bottom (right and left) together, here the vertical (horizontal) keys.

```toml
# Sets only top padding
padding = { top = 3 }

# Sets only top and right padding
padding = { top = 5, right = 6 }

# Instead of
# padding = { top = 5, right = 6, bottom = 5 }
# Write
padding = { vertical = 5, right = 6 }

# If there are collisions of values, an error will be thrown due to ambiguity.
# padding = { top = 5, vertical = 6 }

# You can apply the same way for margin
margin = { top = 5, horizontal = 10 }

# For all-directional padding or margin, set only number as above in CSS
padding = 10
margin = 5
```

### Image

Usually the notification can contain the image or icon and it draws at the right of
banner. More about it in [banner layout](BannerLayout.md#image). The `Noti` application
can perform some actions which in result the image will look very pleasant for most users.

Here's a table of `Image` properties:

| Property name   | Description                                                                                                                                            | Type                                                                    | Default value |
| :-------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------- | :-----------: |
| max_size        | Sets the max size for image and resizes it when width or height of image exceeds `max_size` value                                                      | `u16`                                                                   |      64       |
| rounding        | It's a border-radius in CSS and used to round image corners                                                                                            | `u16`                                                                   |       0       |
| margin          | Creates a spacing around image. If there is no space for image, the image will be squished.                                                            | `u8` or `[u8, u8]` or `[u8, u8, u8]` or `[u8, u8, u8, u8]` or `Spacing` |       0       |
| resizing_method | Sets the resizing method for image when it exceeds `max_size`. Possible values: `"gaussian"`, `"nearest"`, `"triandle"`, `"catmull-rom"`, `"lanczos3"` | `String`                                                                |  "gaussian"   |

### Border

To notification banner you can apply border styles: border size and radius.

- Border size - the width of stroke which is outlines around the banner.
  It also reduces inner space of rectange.
- Border radius - the radius which will applied for rounding the corners of banner.

> [!NOTE]
> You can find that the behavior of banner rounding is different from other applications.
> Here the simple rules for it: inner radius gets from formula $radius - size$.
> It means that inner rounding won't draws if border size exceeds the radius.

The `Border` table:

| Key    | Short description                                      | Type | Default value |
| :----- | :----------------------------------------------------- | :--- | :-----------: |
| size   | The width of stroke which is outlines around the banne | `u8` |       0       |
| radius | The border radius for corner rounding                  | `u8` |       0       |

### Text

Currently the `Noti` application have only title and body, but they are interpreted as `Text` so they both
are described here. Also the `text` property was introduced which can be used for title and body at the same
time. The idiomatic way is using `text` when you want to define the same values for `title` and `body`,
otherwise define values in `title` or `body`. It means `title` and `body` **inherits** from `text` property.

Priority of properties:

1. Picks `title` or `body` properties;
2. If some values is not defined then check `text` properties and replace by it;
3. If some values still not defined then pick default values.

The `Text` table:

| Key           | Short description                                                                                                                                                                           | Type      | Default value |
| :------------ | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :-------- | :-----------: |
| wrap          | Sets possibility to line breaking when text overflows a line                                                                                                                                | `bool`    |     true      |
| ellipsize_at  | Ellipsizes a text if it's totally overflows an area. Possible values: "end" and "middle". "end" - put ellipsis at end of word, "middle" - cut word at some middle of word and puts ellipsis | `String`  |     "end"     |
| style         | Sets the style for whole text area. Possible values: "regular", "bold", "italic", "bold italic"                                                                                             | `String`  |   "regular"   |
| margin        | The text spacing from edges of remaining area                                                                                                                                               | `Spacing` |       0       |
| justification | The text justification. Possible values: "left", "right", "center", "space-between"                                                                                                         | `String`  |    "left"     |
| line_spacing  | The gap between wrapped text lines. Measures in px                                                                                                                                          | `u8`      |       0       |
| font_size     | The size of font. Measures in px                                                                                                                                                            | `u8`      |      12       |

For more explanation how the text draws, please visit [the other documentation about text](BannerLayout.md#text).

### Markup

Enables text styling using HTML tags.
For body applies the `markup` property, because the body can contain the HTML-like tags:

- \<b\> - bold style
- \<i\> - italic style
- \<u\> - underline style
- \<a href="https://google.com"&gt; - the link
- \<img src="path/to/image" alt="image description"\> - the image inside text

You can turn off the `markup` property by setting `false` value.

### Timeout

The time in milliseconds when the notification banner should be closed by expiration
since creation.

The value `0` means will never expired.

---

## Themes

This is feature of the `Noti` application. Instead of defining the color values among
config properties in main config file you can define array of tables named `theme`. You should
put name of theme, otherwise it will be skipped. And use the name of theme in `display.theme`.

> [!NOTE]
> Theme names should have **exact** match. Instead application will use the default theme.

The `Theme` table:

| Key      | Type     | Default value | Short description                        |
| :------- | :------- | :-----------: | :--------------------------------------- |
| low      | `Colors` |       -       | The colors for 'low' urgency banner      |
| normal   | `Colors` |       -       | The colors for 'normal' urgency banner   |
| critical | `Colors` |       -       | The colors for 'critical' urgency banner |

### Colors

Currently possible only three things that can be modified by colors: `foreground`, `background` and
`border`.

The `Colors` table:

| Key        | Type    |               Default value               | Short description                        |
| :--------- | :------ | :---------------------------------------: | :--------------------------------------- |
| background | `Color` |                 "#FFFFFF"                 | The background color of banner           |
| foreground | `Color` | "#000000" (but for `critical`: "#FF0000") | The foreground color which used for text |
| border     | `Color` | "#000000" (but for `critical`: "#FF0000") | The border stroke color                  |

**Example of theme usage**:

```toml
[display]
theme = "my-theme"

[[theme]]
name = "my-theme"

[theme.normal]
border = "#0F0" # green border for normal urgency

[theme.critical]
border = "#F0F" # pink border for critical urgency
```

---

## Apps

The `Noti` application have the huge feature named "app-config" in which you can redefine
`display` table specifically for particular application.

So need to introduce the rule of picking properties:

1. Check is there a defined property by app;
2. If not then redefine by general display property;
3. If is still not defined in general display property then pick default value.

The format of defining `display` config per application:

```toml
[[app]]
name = "Telegram Desktop"

[app.display.border]
size = 3

[[app]]
name = "Spotify"

[app.display]
padding = 3
image = { max_size = 86 }

# and so on..
```

The `App` table:

| Key     | Short description                                    | Type                  |
| :------ | :--------------------------------------------------- | :-------------------- |
| name    | The name of application                              | `String`              |
| display | The display configuration table for this application | [`Display`](#display) |
