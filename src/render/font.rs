use derive_more::Display;
use std::{
    collections::HashMap,
    ops::{Add, Sub},
    sync::Arc,
};

use ab_glyph::FontArc;

struct FontCollection {
    map: HashMap<FontStyle, Font>,
}

struct Font {
    style: FontStyle,
    path: Arc<str>,
    data: FontArc,
}

#[derive(Debug, Display, Hash, PartialEq, Eq)]
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

impl Add for FontStyle {
    type Output = FontStyle;

    fn add(self, rhs: Self) -> Self::Output {
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

impl Sub for FontStyle {
    type Output = FontStyle;

    fn sub(self, rhs: Self) -> Self::Output {
        match self {
            FontStyle::Bold => match rhs {
                FontStyle::Regular => Self::Bold,
                FontStyle::Bold => Self::Regular,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::Italic => match rhs {
                FontStyle::Regular => Self::Italic,
                FontStyle::Italic => Self::Regular,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::BoldItalic => match rhs {
                FontStyle::Bold => Self::Italic,
                FontStyle::Italic => Self::Bold,
                FontStyle::BoldItalic => Self::Regular,
                FontStyle::Regular => Self::BoldItalic,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::ExtraBold => match rhs {
                FontStyle::Bold => Self::Bold,
                FontStyle::ExtraBold => Self::Regular,
                FontStyle::Regular => Self::ExtraBold,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::ExtraLight => match rhs {
                FontStyle::Regular => Self::ExtraLight,
                FontStyle::ExtraLight => Self::Regular,
                FontStyle::Light => Self::Light,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::ExtraLightItalic => match rhs {
                FontStyle::ExtraLight => Self::Italic,
                FontStyle::ExtraLightItalic => Self::Regular,
                FontStyle::LightItalic => Self::Light,
                FontStyle::Regular => Self::ExtraLightItalic,
                FontStyle::Light => Self::LightItalic,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::ExtraBoldItalic => match rhs {
                FontStyle::Bold => Self::BoldItalic,
                FontStyle::Italic => Self::ExtraBold,
                FontStyle::BoldItalic => Self::Bold,
                FontStyle::ExtraBold => Self::Italic,
                FontStyle::ExtraBoldItalic => Self::Regular,
                FontStyle::Regular => Self::ExtraBoldItalic,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::LightItalic => match rhs {
                FontStyle::Italic => Self::Light,
                FontStyle::LightItalic => Self::Regular,
                FontStyle::Regular => Self::LightItalic,
                FontStyle::Light => Self::Italic,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::Thin => match rhs {
                FontStyle::Thin => Self::Regular,
                FontStyle::Regular => Self::Thin,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::ThinItalic => match rhs {
                FontStyle::Thin => Self::Italic,
                FontStyle::ThinItalic => Self::Regular,
                FontStyle::Regular => Self::ThinItalic,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::SemiBold => match rhs {
                FontStyle::SemiBold => Self::Regular,
                FontStyle::Regular => Self::SemiBold,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::SemiBoldItalic => match rhs {
                FontStyle::SemiBold => Self::Italic,
                FontStyle::SemiBoldItalic => Self::Regular,
                FontStyle::Regular => Self::SemiBoldItalic,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::Medium => match rhs {
                FontStyle::Medium => Self::Regular,
                FontStyle::Regular => Self::Medium,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::Regular => match rhs {
                FontStyle::Regular => Self::Regular,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::Light => match rhs {
                FontStyle::Regular => Self::Light,
                FontStyle::Light => Self::Regular,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
            FontStyle::MediumItalic => match rhs {
                FontStyle::Regular => Self::MediumItalic,
                FontStyle::Medium => Self::Italic,
                FontStyle::MediumItalic => Self::Regular,
                other => panic!("Incorrect substraction from {} by {other}", self),
            },
        }
    }
}

#[test]
fn add_font_styles() {
    assert_eq!(FontStyle::Bold + FontStyle::Italic, FontStyle::BoldItalic);
    assert_eq!(
        FontStyle::Bold + FontStyle::Italic + FontStyle::Regular,
        FontStyle::BoldItalic
    );
    assert_eq!(
        FontStyle::Bold + FontStyle::Italic + FontStyle::ExtraBoldItalic,
        FontStyle::ExtraBoldItalic
    );
    assert_eq!(FontStyle::Light + FontStyle::Italic, FontStyle::LightItalic);
    assert_eq!(
        FontStyle::Light + FontStyle::Italic + FontStyle::ExtraLight,
        FontStyle::ExtraLightItalic
    );
    assert_eq!(
        FontStyle::Light + FontStyle::ExtraLight,
        FontStyle::ExtraLight
    );
    assert_eq!(FontStyle::Medium + FontStyle::Regular, FontStyle::Medium);
    assert_eq!(
        FontStyle::Medium + FontStyle::Italic,
        FontStyle::MediumItalic
    );
    assert_eq!(
        FontStyle::ExtraBoldItalic + FontStyle::Italic + FontStyle::Regular + FontStyle::Bold,
        FontStyle::ExtraBoldItalic
    );
    assert_eq!(FontStyle::Thin + FontStyle::Italic, FontStyle::ThinItalic);
    assert_eq!(FontStyle::Thin + FontStyle::Regular, FontStyle::Thin);
    assert_eq!(FontStyle::Regular + FontStyle::Regular, FontStyle::Regular);
    assert_eq!(
        FontStyle::ExtraLightItalic + FontStyle::ExtraLightItalic,
        FontStyle::ExtraLightItalic
    );
    assert_eq!(
        FontStyle::BoldItalic + FontStyle::Bold,
        FontStyle::BoldItalic
    );
    assert_eq!(FontStyle::SemiBold + FontStyle::Bold, FontStyle::SemiBold);
}

#[test]
#[should_panic]
fn panicky_add_font_style() {
    let _ = FontStyle::Bold + FontStyle::Thin;
}

#[test]
fn sub_font_styles() {
    assert_eq!(FontStyle::BoldItalic - FontStyle::Italic, FontStyle::Bold);
    assert_eq!(
        FontStyle::BoldItalic - FontStyle::Italic - FontStyle::Regular,
        FontStyle::Bold
    );
    assert_eq!(
        FontStyle::ExtraBoldItalic - FontStyle::Italic - FontStyle::ExtraBold,
        FontStyle::Regular
    );
    assert_eq!(FontStyle::LightItalic - FontStyle::Italic, FontStyle::Light);
    assert_eq!(
        FontStyle::ExtraLightItalic - FontStyle::Italic - FontStyle::ExtraLight,
        FontStyle::Regular
    );
    assert_eq!(
        FontStyle::ExtraLight - FontStyle::Light,
        FontStyle::Light
    );
    assert_eq!(FontStyle::Medium - FontStyle::Regular, FontStyle::Medium);
    assert_eq!(
        FontStyle::MediumItalic - FontStyle::Italic,
        FontStyle::Medium
    );
    assert_eq!(
        FontStyle::ExtraBoldItalic - FontStyle::Italic - FontStyle::Regular - FontStyle::Bold,
        FontStyle::Bold
    );
    assert_eq!(FontStyle::ThinItalic - FontStyle::Italic, FontStyle::Thin);
    assert_eq!(FontStyle::Thin - FontStyle::Regular, FontStyle::Thin);
    assert_eq!(FontStyle::Regular - FontStyle::Regular, FontStyle::Regular);
    assert_eq!(
        FontStyle::ExtraLightItalic - FontStyle::ExtraLightItalic,
        FontStyle::Regular
    );
    assert_eq!(
        FontStyle::BoldItalic - FontStyle::Bold,
        FontStyle::Italic
    );
}

#[test]
#[should_panic]
fn panicky_sub_font_style() {
    let _ = FontStyle::Bold - FontStyle::Thin;
}
