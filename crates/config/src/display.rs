use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

use dbus::notification::Urgency;
use macros::{ConfigProperty, GenericBuilder};
use serde::{de::Visitor, Deserialize};
use shared::{error::ConversionError, value::TryDowncast};

use crate::{
    public,
    spacing::Spacing,
    text::{TextProperty, TomlTextProperty},
};

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(name(DisplayConfig), derive(Debug))]
    struct TomlDisplayConfig {
        layout: Option<Layout>,

        theme: Option<String>,

        #[cfg_prop(use_type(ImageProperty), mergeable)]
        image: Option<TomlImageProperty>,

        padding: Option<Spacing>,
        #[cfg_prop(use_type(Border), mergeable)]
        border: Option<TomlBorder>,

        #[cfg_prop(temporary, mergeable)]
        text: Option<TomlTextProperty>,

        #[cfg_prop(inherits(field = text), use_type(TextProperty), default(TomlTextProperty::default_title()), mergeable)]
        title: Option<TomlTextProperty>,

        #[cfg_prop(inherits(field = text), use_type(TextProperty), mergeable)]
        body: Option<TomlTextProperty>,

        #[cfg_prop(default(true))]
        markup: Option<bool>,

        #[cfg_prop(default(Timeout::new(0)))]
        timeout: Option<Timeout>,
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
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(
        name(ImageProperty),
        derive(GenericBuilder, Debug, Clone),
        attributes(#[gbuilder(name(GBuilderImageProperty))])
    )]
    struct TomlImageProperty {
        #[cfg_prop(
            default(64),
            attributes(#[gbuilder(default(64))])
        )]
        max_size: Option<u16>,

        #[cfg_prop(
            default(0),
            attributes(#[gbuilder(default(0))])
        )]
        rounding: Option<u16>,

        #[cfg_prop(attributes(#[gbuilder(default)]))]
        margin: Option<Spacing>,

        #[cfg_prop(attributes(#[gbuilder(default)]))]
        resizing_method: Option<ResizingMethod>,
    }
}

impl Default for ImageProperty {
    fn default() -> Self {
        TomlImageProperty::default().into()
    }
}

impl TryFrom<shared::value::Value> for ImageProperty {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
    }
}

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

impl TryFrom<shared::value::Value> for ResizingMethod {
    type Error = shared::error::ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "nearest" => ResizingMethod::Nearest,
                "triangle" => ResizingMethod::Triangle,
                "catmull-rom" | "catmull_rom" => ResizingMethod::CatmullRom,
                "gaussian" => ResizingMethod::Gaussian,
                "lanczos3" => ResizingMethod::Lanczos3,
                _ => Err(shared::error::ConversionError::InvalidValue {
                    expected: "nearest, triangle, gaussian, lanczos3, catmull-rom or catmull_rom",
                    actual: str,
                })?,
            }),
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(shared::error::ConversionError::CannotConvert),
        }
    }
}

public! {
    #[derive(ConfigProperty, Debug, Deserialize, Default, Clone)]
    #[cfg_prop(
        name(Border),
        derive(GenericBuilder, Debug, Clone, Default),
        attributes(#[gbuilder(name(GBuilderBorder))])
    )]
    struct TomlBorder {
        #[cfg_prop(
            default(0),
            attributes(#[gbuilder(default(0))])
        )]
        size: Option<u8>,

        #[cfg_prop(
            default(0),
            attributes(#[gbuilder(default(0))])
        )]
        radius: Option<u8>,
    }
}

impl TryFrom<shared::value::Value> for Border {
    type Error = ConversionError;

    fn try_from(value: shared::value::Value) -> Result<Self, Self::Error> {
        match value {
            shared::value::Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

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
