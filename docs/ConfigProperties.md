# Config Properties

The documentation which describes all existing properties at this moment. You can use it as the reference to correct setting values in your `config.toml` file.

Before of all poperties, need to understand a few primitive type. The complex types like array or table will be explained in place.

| Type   | Explanation                                                                                                                                                         |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| u8     | An unsigned integer of 8 bit                                                                                                                                        |
| u16    | An unsigned integer of 16 bit                                                                                                                                       |
| String | A string. Usually it used as enumeration                                                                                                                            |
| [..]   | Array containing various type. Used as tuple                                                                                                                        |
| Hex    | The hex value which is started by hashtag (#). Used for color definition. It can have three, six or eigth symbols which represent RGBA (alpha-channel is optional). |

The documentation have three main sections of application which is showed below as list.

- [General](#general)
- [Display](#display)
- [Apps](#apps)

You can pick any section for begin. We recommend you to go to through from the first one to the last.
In definitions we may put the link to other property or other definition when we assuming that it is need.

## General

The 'general' word means that it applies to application or all banners together. Here a table of possible general properties and below we'll go through all properties.

<div align = "center">

| Property name     | Type         | Default value   |
| ----------------- | ------------ | --------------- |
| [font](#font)     | [String, u8] | "Noto Sans", 12 |
| [width](#width)   | u16          | 300             |
| [height](#height) | u16          | 150             |
| [anchor](#anchor) | String       | "top right"     |
| [gap](#gap)       | u8           | 10              |
| [offset](#offset) | [u8, u8]     | [0, 0]          |

</div>

### Font

### Width

### Height

### Anchor

### Offset
