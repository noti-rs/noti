use std::{collections::HashMap, marker::PhantomData, path::PathBuf, time::Duration};

use dbus::notification::Urgency;
use macros::{ConfigProperty, GenericBuilder};
use serde::{de::Visitor, Deserialize};

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

        #[cfg_prop(use_type(AnimationProperty), mergeable)]
        animation: Animation,

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
    #[cfg_prop(name(AnimationProperty), derive(Debug, Deserialize, Clone, Default))]
    struct Animation {
        #[cfg_prop(
            use_type(SpacerAnimationDefinitionProperty),
            mergeable,
            default
        )]
        allocation: SpacerAnimationDefinition,

        #[cfg_prop(
            use_type(AnimationDefinitionProperty),
            mergeable,
            default(path = AnimationDefinitionProperty::default_enter)
        )]
        enter: AnimationDefinition,

        #[cfg_prop(
            use_type(AnimationDefinitionProperty),
            mergeable,
            default(path = AnimationDefinitionProperty::default_exit)
        )]
        exit: AnimationDefinition,

        #[cfg_prop(
            use_type(SpacerAnimationDefinitionProperty),
            mergeable,
            default
        )]
        free: SpacerAnimationDefinition,
    }
}

public! {
    #[derive(ConfigProperty,Debug)]
    #[cfg_prop(name(SpacerAnimationDefinitionProperty), derive(Debug, Deserialize, Clone))]
    struct SpacerAnimationDefinition {
        duration: AnimationDuration,
    }
}

impl Default for SpacerAnimationDefinitionProperty {
    fn default() -> Self {
        Self {
            duration: Some(AnimationDuration(Duration::from_secs_f32(0.15))),
        }
    }
}

public! {
    #[derive(ConfigProperty,Debug)]
    #[cfg_prop(name(AnimationDefinitionProperty), derive(Debug, Deserialize, Clone, Default))]
    struct AnimationDefinition {
        style:  AnimationStyle,
        easing: EasingType,
        duration: AnimationDuration,
    }
}

impl AnimationDefinitionProperty {
    fn default_enter() -> Self {
        Self {
            style: Some(AnimationStyle::FadeIn),
            easing: Some(EasingType::EaseInOut),
            duration: Some(AnimationDuration(Duration::from_secs_f32(0.5))),
        }
    }

    fn default_exit() -> Self {
        Self {
            style: Some(AnimationStyle::FadeOut),
            easing: Some(EasingType::EaseInOut),
            duration: Some(AnimationDuration(Duration::from_secs_f32(0.5))),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AnimationDuration(#[serde(with = "humantime_serde")] Duration);

impl From<AnimationDuration> for Duration {
    fn from(value: AnimationDuration) -> Self {
        value.0
    }
}

impl Default for AnimationDuration {
    fn default() -> Self {
        Self(Duration::from_secs_f32(0.5))
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub enum AnimationStyle {
    #[default]
    #[serde(rename = "fade-in")]
    FadeIn,
    #[serde(rename = "fade-out")]
    FadeOut,
    #[serde(rename = "pop-in")]
    PopIn,
    #[serde(rename = "pop-out")]
    PopOut,
    #[serde(rename = "slide-in")]
    SlideIn,
    #[serde(rename = "slide-out")]
    SlideOut,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub enum EasingType {
    #[serde(rename = "linear")]
    Linear,
    #[serde(rename = "ease-out")]
    EaseOut,
    #[default]
    #[serde(rename = "ease-in-out")]
    EaseInOut,
}

impl From<EasingType> for widgets::animations::Easing {
    fn from(value: EasingType) -> Self {
        match value {
            EasingType::Linear => widgets::animations::Easing::Linear,
            EasingType::EaseOut => widgets::animations::Easing::EaseOut,
            EasingType::EaseInOut => widgets::animations::Easing::EaseInOut,
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
    #[derive(ConfigProperty, Debug, Clone)]
    #[cfg_prop(name(TomlImageProperty), derive(Debug, Clone, Default, Deserialize))]
    struct ImageProperty {
        #[cfg_prop(default(64))]
        max_size: u16,

        #[cfg_prop(default(0))]
        rounding: u16,

        margin: Spacing,

        resizing_method: ResizingMethod,
        mipmap_mode: MipmapMode,
        fit_mode: FitMode,
    }
}

impl Default for ImageProperty {
    fn default() -> Self {
        TomlImageProperty::default().into()
    }
}

impl From<ImageProperty> for widgets::widget::ImageStyle {
    fn from(value: ImageProperty) -> Self {
        Self {
            max_size: value.max_size,
            rounding: value.rounding,
            margin: value.margin.into(),
            resizing_method: value.resizing_method.into(),
            mipmap_mode: value.mipmap_mode.into(),
            fit_mode: value.fit_mode.into()
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum ResizingMethod {
    #[serde(rename = "nearest")]
    Nearest,
    #[default]
    #[serde(rename = "linear")]
    Linear,
}

impl From<ResizingMethod> for widgets::widget::ResizingMethod {
    fn from(value: ResizingMethod) -> Self {
        match value {
            ResizingMethod::Nearest => widgets::widget::ResizingMethod::Nearest,
            ResizingMethod::Linear => widgets::widget::ResizingMethod::Linear,
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum MipmapMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "nearest")]
    Nearest,
    #[default]
    #[serde(rename = "linear")]
    Linear,
}

impl From<MipmapMode> for widgets::widget::MipmapMode {
    fn from(value: MipmapMode) -> Self {
        match value {
            MipmapMode::None => widgets::widget::MipmapMode::None,
            MipmapMode::Nearest => widgets::widget::MipmapMode::Nearest,
            MipmapMode::Linear => widgets::widget::MipmapMode::Linear,
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum FitMode {
    #[default]
    #[serde(rename = "contain")]
    Contain,
    #[serde(rename = "cover")]
    Cover,
    #[serde(rename = "fill")]
    Fill,
    #[serde(rename = "scale-down")]
    ScaleDown,
}

impl From<FitMode> for widgets::widget::FitMode {
    fn from(value: FitMode) -> Self {
        match value {
            FitMode::Contain => widgets::widget::FitMode::Contain,
            FitMode::Cover => widgets::widget::FitMode::Cover,
            FitMode::Fill => widgets::widget::FitMode::Fill,
            FitMode::ScaleDown => widgets::widget::FitMode::ScaleDown,
        }
    }
}

public! {
    #[derive(ConfigProperty, GenericBuilder, Debug, Default, Clone)]
    #[cfg_prop(name(TomlBorder), derive(Debug, Clone, Default, Deserialize))]
    #[gbuilder(name(GBuilderBorder), derive(Clone))]
    struct Border {
        #[cfg_prop(default(0))]
        #[gbuilder(default(0))]
        size: u32,

        #[cfg_prop(default(0))]
        #[gbuilder(default(0))]
        radius: u32,
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
