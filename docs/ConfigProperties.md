# Config Properties

The Noti application have a way to configure notification banners by TOML config file which placed
at specific position. Here a priority of positions:

1. `$XDG_CONFIG_HOME/noti/config.toml`
2. `$HOME/.config/noti/config.toml`

Use this file as documentation for all properties.

> [!NOTE]
> Don't need to reload the application after changing config properties because it have
> `hot-reload` or `watch-mode`.

Before of all poperties, need to understand a few primitive type. The complex types like array or table will be explained in place.

| Type     | Explanation                                                                                                                                                                                                          |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bool`   | A boolean value                                                                                                                                                                                                      |
| `u8`     | An unsigned integer of 8 bit                                                                                                                                                                                         |
| `u16`    | An unsigned integer of 16 bit                                                                                                                                                                                        |
| `String` | A string. Usually it used as enumeration                                                                                                                                                                             |
| `[..]`   | Array containing various type. Used as tuple                                                                                                                                                                         |
| `Color`  | The hex value which is started by hashtag (#) and wrapped by doubled quotes (defines as string). It can have three, six or eight symbols which represent RGB (including alpha-channel in 8-symboled hex as opacity). |

The documentation have three main sections of application which is showed below as list.

- [General](#general)
- [Display](#display)
- [Apps](#apps)

You can pick any section for begin. We recommend you to go to through from the first one to the last.
In definitions we may put the link to other property or other definition when we assuming that it is need.

## General

The 'general' word means that it applies to application or all banners together. Here a table of possible general properties and below we'll go through all properties.

| Property name       | Type                  |  Default value  |
| :------------------ | :-------------------- | :-------------: |
| [font](#font)       | `[String, u8]`        | "Noto Sans", 12 |
| [width](#width)     | `u16`                 |       300       |
| [height](#height)   | `u16`                 |       150       |
| [anchor](#anchor)   | `String`              |   "top right"   |
| [gap](#gap)         | `u8`                  |       10        |
| [offset](#offset)   | `[u8, u8]`            |     [0, 0]      |
| [sorting](#sorting) | `String` or `Sorting` |    "default"    |

### Font

The first value of array is the font name. You can write it with or without spaces.
The application can use only the font name which can be used as pattern in `fc-list` command.
So you can't use already styled font like "Noto Sans Bold". But the application internally
can load font with needed styles if it is possible.

The second value of array is the font size in `px`.

### Width

The banner frame width. Currently it is fixed for all banners.

### Height

The banner frame height. Currently it is fixed for all banners.

### Anchor

The anchor of current monitor for current window instance.
It means where you want to see appearing notification banners.
The possible values:

- `"top"`
- `"top-left"` or `"top left"`
- `"top-right"` or `"top right"`
- `"left"`
- `"right"`
- `"bottom"`
- `"bottom-left"` or `"bottom left"`
- `"bottom-right"` or `"bottom right"`

### Offset

The offset from edges for window instance.
The first value is the offset by x-axis, the second value about y-axis.

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
- "id" (means the notification id, simple to "time", but in replacement it stays in old place)
- "urgency"

Possible values of the `ordering` property name:

- "ascending" (also possible short name "asc")
- "descending" (also possible short name "desc")

---

## Display

To change the visual styles of banners use `display` table. You cad define
the values of `display` table for all the appications at the same time and
use specific values per application by [app config](#apps).

If you curious about banner layout, please visit [the other document](#BannerLayout.md)
which is maded specifically for it.

The display properties affects only and only for a banner, not the window entire.
The currently possible properties of `display` table:

| Property name                  | Type                                                                    | Default value |
| :----------------------------- | :---------------------------------------------------------------------- | :-----------: |
| [image](#image)                | `Image`                                                                 |       -       |
| [padding](#padding-and-margin) | `u8` or `[u8, u8]` or `[u8, u8, u8]` or `[u8, u8, u8, u8]` or `Spacing` |       0       |
| [border](#border)              | `Border`                                                                |       -       |
| [colors](#colors)              | `UrgencyColors`                                                         |       -       |
| [text](#text)                  | `Text`                                                                  |       -       |
| [title](#text)                 | `Text`                                                                  |       -       |
| [body](#text)                  | `Text`                                                                  |       -       |
| [markup](#markup)              | `bool`                                                                  |     true      |
| [timeout](#timeout)            | `u16`                                                                   |       0       |

The `Spacing` table:

| Key        | Type | Short description                                                           |
| :--------- | :--- | :-------------------------------------------------------------------------- |
| top        | `u8` | Spacing from top                                                            |
| right      | `u8` | Spacing from right                                                          |
| bottom     | `u8` | Spacing from bottom                                                         |
| left       | `u8` | Spacing from left                                                           |
| vertical   | `u8` | Spacing from top and bottom together (incompatible with top or bottom keys) |
| horizontal | `u8` | Spacing from left and right together (incompatible with left or right keys) |

### Padding and margin

Within scope of this application, the padding and margin have different meaning.
The padding is the offset from the banner edges for inner elements, it's like giving the content area smaller.
The margin is the offset from the edges of remaining area and other inner elements.

> [!NOTE]
> If you have issue that the image or the text doesn't show in banner, it's maybe because of large value of padding or margins that the content can't fit into remaining space.

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

# Insead of
# padding = { top = 5, right = 6, bottom = 5 }
# Write
padding = { vertical = 5, right = 6 }

# If gots collision of values the error will throws because of ambuguity
# padding = { top = 5, vertical = 6 }

# You can apply the same way for margin
margin = { top = 5, horizontal = 10 }

# For all-directional padding or margin, set only nubmer as above in CSS
padding = 10
margin = 5
```

### Image

Usually the notification can contain the image or icon and it draws at the right of
banner. More about it in [banner layout](BannerLayout.md#image)

Our application can perform some actions which in result the image will look very
pleasant for most users.

Here a table of `Image` properties:

| Property name   | Type                                                                    | Default value | Description                                                                                                                                  |
| :-------------- | :---------------------------------------------------------------------- | :-----------: | :------------------------------------------------------------------------------------------------------------------------------------------- |
| max_size        | `u16`                                                                   |      64       | Sets the max size for image and resizes it when width or height exceeds `max_size`                                                           |
| rounding        | `u16`                                                                   |       0       | It's a border-radius in CSS and used to round image corners                                                                                  |
| margin          | `u8` or `[u8, u8]` or `[u8, u8, u8]` or `[u8, u8, u8, u8]` or `Spacing` |       0       | Creates a spacing around image. **NOTE: It may be suppressed when an available space is not enough**                                         |
| resizing_method | `String`                                                                |  "gaussian"   | Sets the resizing method for image when it exceeds `max_size`. Possible values: "gaussian", "nearest", "triandle", "catmull-rom", "lanczos3" |

### Border

To notification banner you can apply border styles: border size, radius and color.

- Border size - the width of stroke which is outlines around the banne. It draws in the rectangle, so the inner elements can overlay the border if you pick margin smaller than border size.
- Border radius - the radius which will applied for rounding the corners of banner.
- Border color - the color of stroke.

> [!NOTE]
> You can find that the behavior of banner rounding is different from other applications.
> Here the simple rules for it: inner radius gets from formula $radius - size$.
> It means that inner rounding won't draws if border size exceeds the radius.

The `Border` table:

| Key    | Type    | Default value | Short description                                      |
| :----- | :------ | :-----------: | :----------------------------------------------------- |
| size   | `u8`    |       0       | The width of stroke which is outlines around the banne |
| radius | `u8`    |       0       | The border radius for corner rounding                  |
| color  | `Color` |   "#000000"   | The stroke color                                       |

### Colors

A notification have three urgencies: low, normal and critical. And you can define
colors for all them separately.

The `UrgencyColors` table:

| Key      | Type     |                   Default value                    | Short description                        |
| :------- | :------- | :------------------------------------------------: | :--------------------------------------- |
| low      | `Colors` | { background = "#FFFFFF", foreground = "#000000" } | The colors for 'low' urgency banner      |
| normal   | `Colors` | { background = "#FFFFFF", foreground = "#000000" } | The colors for 'normal' urgency banner   |
| critical | `Colors` | { background = "#FFFFFF", foreground = "#FF0000" } | The colors for 'critical' urgency banner |

The `Colors` table:

| Key        | Type    | Default value | Short description                        |
| :--------- | :------ | :-----------: | :--------------------------------------- |
| background | `Color` |   "#FFFFFF"   | The background color of banner           |
| foreground | `Color` |   "#000000"   | The foreground color which used for text |

### Text

Currently our application have only title and body, but they are interpreted as `Text` so they both described here.
Also introduced a property `text` which can be used for title and body. The idiomatic way tells use `text` when
you want to define the same values for `title` and `body`, otherwise define values in `title` or `body`. It means
`title` and `body` **inherits** from `text` property.

Priority of properties:

1. Picks `title` or `body` properties;
2. If some values is not defined then check `text` properties and replace by it;
3. If some values still not defined then pick default values.

The `Text` table:

| Key           | Type      | Default value | Short description                                                                                                                                                                           |
| :------------ | :-------- | :-----------: | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| wrap          | `bool`    |     true      | Sets possibility to line breaking when text overflows a line                                                                                                                                |
| ellipsize_at  | `String`  |     "end"     | Ellipsizes a text if it's totally overflows an area. Possible values: "end" and "middle". "end" - put ellipsis at end of word, "middle" - cut word at some middle of word and puts ellipsis |
| style         | `String`  |   "regular"   | Sets the style for whole text area. Possible values: "regular", "bold", "italic", "bold italic"                                                                                             |
| margin        | `Spacing` |       0       | The text spacing from edges of remaining area                                                                                                                                               |
| justification | `String`  |    "left"     | The text justification. Possible values: "left", "right", "center", "space-between"                                                                                                         |
| line_spacing  | `u8`      |       0       | The gap between wrapped text lines                                                                                                                                                          |

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

## Apps

This application have the huge feature named "app-config" in which you can redefine `display`
table specifically for particular application.

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
