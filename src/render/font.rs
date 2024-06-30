use derive_more::Display;
use std::{collections::HashMap, ops::BitOr, sync::Arc};

use ab_glyph::FontArc;

struct FontCollection {
    map: HashMap<FontStyle, Font>,
}

struct Font {
    style: FontStyle,
    path: Arc<str>,
    data: FontArc,
}

#[derive(Hash, Display)]
enum FontStyle {
    #[display(fmt = "Bold")]
    Bold,
    #[display(fmt = "Itali")]
    Italic,
    #[display(fmt = "BoldItalic")]
    BoldItalic,
    #[display(fmt = "ExtraBold")]
    ExtraBold,
    #[display(fmt = "ExtraLight")]
    ExtraLight,
    #[display(fmt = "ExtraLightItalic")]
    ExtraLightItalic,
    #[display(fmt = "ExtraBoldItalic")]
    ExtraBoldItalic,
    #[display(fmt = "LightItalic")]
    LightItalic,
    #[display(fmt = "Thin")]
    Thin,
    #[display(fmt = "ThinItalic")]
    ThinItalic,
    #[display(fmt = "SemiBold")]
    SemiBold,
    #[display(fmt = "SemiBoldItalic")]
    SemiBoldItalic,
    #[display(fmt = "Medium")]
    Medium,
    #[display(fmt = "Regular")]
    Regular,
    #[display(fmt = "Light")]
    Light,
    #[display(fmt = "MediumItalic")]
    MediumItalic,
}

impl BitOr for FontStyle {
    type Output = FontStyle;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            Self::Bold => match rhs {
                Self::Regular | Self::Bold => Self::Bold,
                Self::Italic | Self::BoldItalic => Self::BoldItalic,
                Self::ExtraBold => Self::ExtraBold,
                Self::ExtraBoldItalic => Self::ExtraBoldItalic,
                Self::SemiBold => Self::SemiBold,
                Self::SemiBoldItalic => Self::SemiBoldItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::Italic => match rhs {
                Self::Regular | Self::Italic => Self::Italic,
                Self::Bold | Self::BoldItalic => Self::BoldItalic,
                Self::ExtraLight | Self::ExtraLightItalic => Self::ExtraLightItalic,
                Self::ExtraBold | Self::ExtraBoldItalic => Self::ExtraBoldItalic,
                Self::Light | Self::LightItalic => Self::LightItalic,
                Self::Thin | Self::ThinItalic => Self::ThinItalic,
                Self::SemiBold | Self::SemiBoldItalic => Self::SemiBoldItalic,
                Self::Medium | Self::MediumItalic => Self::MediumItalic,
            },
            Self::BoldItalic => match rhs {
                Self::Regular | Self::Bold | Self::Italic | Self::BoldItalic => Self::BoldItalic,
                Self::ExtraBold | Self::ExtraBoldItalic => Self::ExtraBoldItalic,
                Self::SemiBold | Self::SemiBoldItalic => Self::SemiBoldItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::ExtraBold => match rhs {
                Self::Regular | Self::Bold | Self::ExtraBold => Self::ExtraBold,
                Self::Italic | Self::BoldItalic | Self::ExtraBoldItalic => Self::ExtraBoldItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::ExtraLight => match rhs {
                Self::Regular | Self::ExtraLight => Self::ExtraLight,
                Self::Italic | Self::ExtraLightItalic | Self::LightItalic => Self::ExtraLightItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::ExtraLightItalic => match rhs {
                Self::Regular
                | Self::Italic
                | Self::ExtraLight
                | Self::ExtraLightItalic
                | Self::LightItalic
                | Self::Light => Self::ExtraLightItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::ExtraBoldItalic => match rhs {
                Self::Regular
                | Self::Bold
                | Self::Italic
                | Self::BoldItalic
                | Self::ExtraBold
                | Self::ExtraBoldItalic => Self::ExtraBoldItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::LightItalic => match rhs {
                Self::Regular | Self::Italic | Self::Light | Self::LightItalic => Self::LightItalic,
                Self::ExtraLight | Self::ExtraLightItalic => Self::ExtraLightItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::Thin => match rhs {
                Self::Regular | Self::Thin => Self::Thin,
                Self::Italic | Self::ThinItalic => Self::ThinItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::ThinItalic => match rhs {
                Self::Regular | Self::Thin | Self::Italic | Self::ThinItalic => Self::ThinItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::SemiBold => match rhs {
                Self::Regular | Self::Bold | Self::SemiBold => Self::SemiBold,
                Self::Italic | Self::BoldItalic | Self::SemiBoldItalic => Self::SemiBoldItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::SemiBoldItalic => match rhs {
                Self::Regular
                | Self::Bold
                | Self::SemiBold
                | Self::Italic
                | Self::BoldItalic
                | Self::SemiBoldItalic => Self::SemiBoldItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::Medium => match rhs {
                Self::Regular | Self::Medium => Self::Medium,
                Self::MediumItalic => Self::MediumItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::Regular => rhs,
            Self::Light => match rhs {
                Self::Regular | Self::Light => Self::Light,
                Self::Italic | Self::LightItalic => Self::LightItalic,
                Self::ExtraLight => Self::ExtraLight,
                Self::ExtraLightItalic => Self::ExtraLightItalic,
                other => panic!("Incorrect combination of {} and {other}", self),
            },
            Self::MediumItalic => match rhs {
                Self::Regular | Self::Medium | Self::Italic | Self::MediumItalic => {
                    Self::MediumItalic
                }
                other => panic!("Incorrect combination of {} and {other}", self),
            },
        }
    }
}
