# Config Properties

The documentation which describes all existing properties at this moment. You can use it as the reference to correct setting values in your `config.toml` file.

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

| Property name     | Type           |  Default value  |
| :---------------- | :------------- | :-------------: |
| [font](#font)     | `[String, u8]` | "Noto Sans", 12 |
| [width](#width)   | `u16`          |       300       |
| [height](#height) | `u16`          |       150       |
| [anchor](#anchor) | `String`       |   "top right"   |
| [gap](#gap)       | `u8`           |       10        |
| [offset](#offset) | `[u8, u8]`     |     [0, 0]      |

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

---

## Display

To change the visual styles of banners use `display` table. You cad define
the values of `display` table for all the appications at the same time and
use specific values per application by [app config](#apps).

If you curious about banner layout, please visit [the other document](#BannerLayout.md)
which is maded specifically for it.

The display properties affects only and only for a banner, not the window entire.
The currently possible properties of `display` table:

| Property name             | Type                                                                   | Default value |
| :------------------------ | :--------------------------------------------------------------------- | :-----------: |
| [image_size](#image-size) | `u16`                                                                  |      64       |
| [padding](#padding)       | `u8` or `[u8, u8]` or `[u8, u8, u8]` or `[u8, u8, u8, u8]` or `Offset` |       0       |
| [border](#border)         | `Border`                                                               |       -       |
| [colors](#colors)         | `UrgencyColors`                                                        |       -       |
| [title](#text)            | `Text`                                                                 |       -       |
| [body](#text)             | `Text`                                                                 |       -       |
| [markup](#markup)         | `bool`                                                                 |     true      |
| [timeout](#timeout)       | `u16`                                                                  |       0       |

The `Offset` table:

| Key        | Type | Short description                                                          |
| :--------- | :--- | :------------------------------------------------------------------------- |
| top        | `u8` | Offset from top                                                            |
| right      | `u8` | Offset from right                                                          |
| bottom     | `u8` | Offset from bottom                                                         |
| left       | `u8` | Offset from left                                                           |
| vertical   | `u8` | Offset from top and bottom together (incompatible with top or bottom keys) |
| horizontal | `u8` | Offset from left and right together (incompatible with left or right keys) |

### Image size

Usually the notification can contain the image or icon and it draws at the right of
banner. More about it in [banner layout](BannerLayout.md#image)

The image size defines in `px` measurement.

### Padding

The offset from outer edges of banner.
This property mostly described in [banner layout](BannerLayout.md#padding).

### Border

The notification banner's border.
This property mostly described in [banner layout](BannerLayout.md#border).

The `Border` table:

| Key    | Type    | Short description                                      |
| :----- | :------ | :----------------------------------------------------- |
| size   | `u8`    | The width of stroke which is outlines around the banne |
| radius | `u8`    | the border radius for corner rounding                  |
| color  | `Color` | The stroke color                                       |

### Colors

A notification have three urgencies: low, normal and critical. And you can define
colors for all them separately.

The `UrgencyColors` table:

| Key      | Type     | Short description                        |
| :------- | :------- | :--------------------------------------- |
| low      | `Colors` | The colors for 'low' urgency banner      |
| normal   | `Colors` | The colors for 'normal' urgency banner   |
| critical | `Colors` | The colors for 'critical' urgency banner |

The `Colors` table:

| Key        | Type    | Short description                        |
| :--------- | :------ | :--------------------------------------- |
| background | `Color` | The background color of banner           |
| foreground | `Color` | The foreground color which used for text |

### Text

The title and the body have same definifions as text-element so they combined
into `Text` section.

The `Text` table:

| Key           | Type     | Short description                            |
| :------------ | :------- | :------------------------------------------- |
| margin        | `Offset` | The text offset from edges of remaining area |
| justification | `String` | The text justification                       |
| line_spacing  | `u8`     | The gap between wrapped text lines           |

Currently available text justification values:

- `"left"`
- `"center"`
- `"right"`
- `"space between"`

For more explanation how the text draws, please visit [the other documentation about text](BannerLayout.md#text).

### Markup

Enables text styling using HTML tags.

This property mostly described in [banner layout](BannerLayout.md#body).

### Timeout

The time in milliseconds when the notification banner should be closed by expiration
since creation.

The value `0` means will never expired.

## Apps

This application have huge feature named "app-config" in which you can redefine `display`
table specifically for particular application.

Not nesseccary to redefine fully `display` table because it inerhits from general
`display` table.

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
icon_size = 86

# and so on..
```
