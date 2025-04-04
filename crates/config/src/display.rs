use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

use dbus::notification::Urgency;
use macros::{ConfigProperty, GenericBuilder};
use serde::{de::Visitor, Deserialize};
use shared::{error::ConversionError, value::TryFromValue};

use crate::{
    public,
    spacing::Spacing,
    text::{TextProperty, TomlTextProperty},
};

public! {
    #[derive(ConfigProperty, Debug)]
    #[cfg_prop(name(TomlDisplayConfig), derive(Debug, Deserialize, Default, Clone))]
    struct DisplayConfig {
        layout: Layout,

        theme: String,

        #[cfg_prop(use_type(IconInfoProperty), mergeable)]
        icons: IconInfo,

        #[cfg_prop(use_type(TomlImageProperty), mergeable)]
        image: ImageProperty,

        padding: Spacing,

        #[cfg_prop(use_type(TomlBorder), mergeable)]
        border: Border,

        #[cfg_prop(
            also_from(name = text, mergeable),
            use_type(TomlTextProperty),
            default(TomlTextProperty::default_summary()),
            mergeable
        )]
        summary: TextProperty,

        #[cfg_prop(
            also_from(name = text, mergeable),
            use_type(TomlTextProperty),
            mergeable
        )]
        body: TextProperty,

        #[cfg_prop(default(true))]
        markup: bool,

        #[cfg_prop(default(Timeout::new(0)))]
        timeout: Timeout,
    }
}

impl TomlDisplayConfig {
    pub(super) fn use_relative_path(&mut self, mut prefix: PathBuf) {
        if let Some(Layout::FromPath { ref mut path_buf }) = self.layout.as_mut() {
            if path_buf.is_relative() {
                prefix.extend(&*path_buf);
                *path_buf = prefix;
            }
        };
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(from = "String")]
pub enum Layout {
    #[default]
    Default,
    FromPath {
        path_buf: PathBuf,
    },
}

impl Layout {
    pub fn is_default(&self) -> bool {
        matches!(self, Layout::Default)
    }
}

impl From<String> for Layout {
    fn from(value: String) -> Self {
        if value == "default" {
            return Layout::Default;
        }

        Layout::FromPath {
            path_buf: PathBuf::from(
                shellexpand::full(&value)
                    .map(|value| value.into_owned())
                    .unwrap_or(value),
            ),
        }
    }
}

public! {
    #[derive(ConfigProperty, Debug)]
    #[cfg_prop(name(IconInfoProperty), derive(Debug, Deserialize, Clone, Default))]
    struct IconInfo {
        #[cfg_prop(default("Adwaita".to_string()))]
        theme: String,

        #[cfg_prop(default(vec![64, 32]))]
        size: Vec<u16>,
    }
}

public! {
    #[derive(ConfigProperty, GenericBuilder, Debug, Clone)]
    #[cfg_prop(name(TomlImageProperty), derive(Debug, Clone, Default, Deserialize))]
    #[gbuilder(name(GBuilderImageProperty), derive(Clone))]
    struct ImageProperty {
        #[cfg_prop(default(64))]
        #[gbuilder(default(64))]
        max_size: u16,

        #[cfg_prop(default(0))]
        #[gbuilder(default(0))]
        rounding: u16,

        #[gbuilder(default)]
        margin: Spacing,

        #[gbuilder(default)]
        resizing_method: ResizingMethod,
    }
}

impl Default for ImageProperty {
    fn default() -> Self {
        TomlImageProperty::default().into()
    }
}

impl TryFromValue for ImageProperty {}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum ResizingMethod {
    #[serde(rename = "nearest")]
    Nearest,
    #[serde(rename = "triangle")]
    Triangle,
    #[serde(rename = "catmull-rom")]
    CatmullRom,
    #[default]
    #[serde(rename = "gaussian")]
    Gaussian,
    #[serde(rename = "lanczos3")]
    Lanczos3,
}

impl TryFromValue for ResizingMethod {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "nearest" => ResizingMethod::Nearest,
            "triangle" => ResizingMethod::Triangle,
            "catmull-rom" | "catmull_rom" => ResizingMethod::CatmullRom,
            "gaussian" => ResizingMethod::Gaussian,
            "lanczos3" => ResizingMethod::Lanczos3,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "nearest, triangle, gaussian, lanczos3, catmull-rom or catmull_rom",
                actual: value,
            })?,
        })
    }
}

public! {
    #[derive(ConfigProperty, GenericBuilder, Debug, Default, Clone)]
    #[cfg_prop(name(TomlBorder), derive(Debug, Clone, Default, Deserialize))]
    #[gbuilder(name(GBuilderBorder), derive(Clone))]
    struct Border {
        #[cfg_prop(default(0))]
        #[gbuilder(default(0))]
        size: u8,

        #[cfg_prop(default(0))]
        #[gbuilder(default(0))]
        radius: u8,
    }
}

impl TryFromValue for Border {}

#[derive(Debug, Default, Clone)]
pub struct Timeout {
    default: Option<u16>,
    low: Option<u16>,
    normal: Option<u16>,
    critical: Option<u16>,
}

impl Timeout {
    const DEFAULT: u16 = 0;

    fn new(default_value: u16) -> Self {
        Self {
            default: default_value.into(),
            ..Default::default()
        }
    }

    pub fn by_urgency(&self, urgency: &Urgency) -> u16 {
        match urgency {
            Urgency::Low => self.low,
            Urgency::Normal => self.normal,
            Urgency::Critical => self.critical,
        }
        .or(self.default)
        .unwrap_or(Self::DEFAULT)
    }
}

impl From<u16> for Timeout {
    fn from(value: u16) -> Self {
        Timeout::new(value)
    }
}

impl From<HashMap<String, u16>> for Timeout {
    fn from(value: HashMap<String, u16>) -> Self {
        Timeout {
            default: value.get("default").copied(),
            low: value.get("low").copied(),
            normal: value.get("normal").copied(),
            critical: value.get("critical").copied(),
        }
    }
}

struct TimeoutVisitor<T>(PhantomData<fn() -> T>);

impl<'de> Deserialize<'de> for Timeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(TimeoutVisitor(PhantomData))
    }
}

impl<'de, T> Visitor<'de> for TimeoutVisitor<T>
where
    T: Deserialize<'de> + From<u16> + From<HashMap<String, u16>>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            r#"Either u16 or Table value.

Example:

# In milliseconds
display.timeout = 2000

# or

[display.timeout]
low = 2000
normal = 4000
critical = 5000

# or

[display.timeout]
default = 3000 # for low and normal this value will be set
critical = 0 # but for critical the default value will be overriden
"#
        )
    }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.into())
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v < 0 {
            Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Signed(v),
                &self,
            ))
        } else {
            Ok((v.clamp(0, u16::MAX as i64) as u16).into())
        }
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut local_map = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, u16>()? {
            match key.as_str() {
                "default" | "low" | "normal" | "critical" => {
                    local_map.insert(key, value);
                }
                _ => {
                    return Err(serde::de::Error::unknown_variant(
                        &key,
                        &["default", "low", "normal", "critical"],
                    ))
                }
            }
        }

        Ok(local_map.into())
    }
}
