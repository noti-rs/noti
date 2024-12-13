use std::path::PathBuf;

use macros::{ConfigProperty, GenericBuilder};
use serde::Deserialize;
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

        #[cfg_prop(default(0))]
        timeout: Option<u16>,
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
