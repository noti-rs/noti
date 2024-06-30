use derive_more::Display;
use std::{
    collections::HashMap,
    ops::{Add, Sub},
    process::Command,
    sync::Arc,
};

use ab_glyph::FontArc;

use crate::data::aliases::Result;

struct FontCollection {
    map: HashMap<FontStyle, Font>,
}

impl FontCollection {
    fn load_by_font_name(font_name: String) -> Result<Self> {
        let process_result = Command::new(format!(
            "fc-list {font_name} --format \"%{{file}}:%{{style}}\""
        ))
        .spawn()?
        .wait_with_output()?;

        let output: String = process_result
            .stdout
            .into_iter()
            .map(|data| data as char)
            .collect();

        let map = output
            .split("\n")
            .map(|line| {
                let (filepath, styles) = line
                    .split_once(":")
                    .expect("Must be the colon delimiter in --format when calling fc-list");
                Font::try_read(filepath, styles).expect("Can't read the font file")
            })
            .fold(HashMap::new(), |mut acc, font| {
                acc.insert(font.style.clone(), font);
                acc
            });

        Ok(Self { map })
    }
}

struct Font {
    style: FontStyle,
    path: Arc<str>,
    data: FontArc,
}

impl Font {
    fn try_read(filepath: &str, styles: &str) -> Result<Self> {
        let style = FontStyle::from(
            styles
                .split_once(",")
                .map(|(important_style, _)| important_style)
                .unwrap_or(styles),
        );
        let bytes = std::fs::read(filepath)?;
        let data = FontArc::try_from_vec(bytes)?;
        Ok(Self {
            style,
            data,
            path: Arc::from(filepath),
        })
    }
}

#[derive(Debug, Display, Hash, PartialEq, Eq, Clone)]
enum FontStyle {
    #[display(fmt = "Regular")]
    Regular,
    #[display(fmt = "Bold")]
    Bold,
    #[display(fmt = "Italic")]
    Italic,
    #[display(fmt = "BoldItalic")]
    BoldItalic,
    #[display(fmt = "Medium")]
    Medium,
    #[display(fmt = "MediumItalic")]
    MediumItalic,
    #[display(fmt = "Light")]
    Light,
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
    #[display(fmt = "ExtraBold")]
    ExtraBold,
    #[display(fmt = "ExtraLight")]
    ExtraLight,
    #[display(fmt = "ExtraBoldItalic")]
    ExtraBoldItalic,
    #[display(fmt = "ExtraLightItalic")]
    ExtraLightItalic,
}

impl From<&str> for FontStyle {
    fn from(value: &str) -> Self {
        match value {
            "Regular" => Self::Regular,
            "Bold" => Self::Bold,
            "Italic" => Self::Italic,
            "Bold Italic" => Self::BoldItalic,
            "Medium" => Self::Medium,
            "Medium Italic" => Self::MediumItalic,
            "SemiBold" => Self::SemiBold,
            "SemiBold Italic" => Self::SemiBoldItalic,
            "Light" => Self::Light,
            "Light Italic" => Self::LightItalic,
            "Thin" => Self::Thin,
            "Thin Italic" => Self::ThinItalic,
            "ExtraBold" => Self::ExtraBold,
            "ExtraBold Italic" => Self::ExtraBoldItalic,
            "ExtraLight" => Self::ExtraLight,
            "ExtraLight Italic" => Self::ExtraLightItalic,
            other => panic!("Invalid style: {other}"),
        }
    }
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
                Self::Regular | Self::Light | Self::ExtraLight => Self::ExtraLight,
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
                Self::Italic | Self::MediumItalic => Self::MediumItalic,
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
                FontStyle::Italic => Self::ExtraLight,
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
                FontStyle::Italic => Self::Thin,
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
                FontStyle::Italic => Self::SemiBold,
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
                FontStyle::Italic => Self::Medium,
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
    assert_eq!(
        FontStyle::ExtraBoldItalic + FontStyle::ExtraBoldItalic,
        FontStyle::ExtraBoldItalic
    );
    assert_eq!(
        FontStyle::ExtraBold + FontStyle::ExtraBoldItalic,
        FontStyle::ExtraBoldItalic
    );
    assert_eq!(FontStyle::Light + FontStyle::Italic, FontStyle::LightItalic);
    assert_eq!(
        FontStyle::Light + FontStyle::Italic + FontStyle::ExtraLight,
        FontStyle::ExtraLightItalic
    );
    assert_eq!(
        FontStyle::ExtraLightItalic + FontStyle::ExtraLight,
        FontStyle::ExtraLightItalic
    );
    assert_eq!(
        FontStyle::LightItalic + FontStyle::ExtraLight,
        FontStyle::ExtraLightItalic
    );
    assert_eq!(
        FontStyle::Italic + FontStyle::ExtraLight,
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
    assert_eq!(FontStyle::Thin + FontStyle::Thin, FontStyle::Thin);
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
    assert_eq!(
        FontStyle::ExtraBoldItalic - FontStyle::Italic,
        FontStyle::ExtraBold,
    );
    assert_eq!(FontStyle::LightItalic - FontStyle::Italic, FontStyle::Light);
    assert_eq!(
        FontStyle::ExtraLightItalic - FontStyle::Italic - FontStyle::ExtraLight,
        FontStyle::Regular
    );
    assert_eq!(FontStyle::ExtraLight - FontStyle::Light, FontStyle::Light);
    assert_eq!(
        FontStyle::ExtraLight - FontStyle::Light - FontStyle::Light,
        FontStyle::Regular
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
    assert_eq!(
        FontStyle::ThinItalic - FontStyle::ThinItalic,
        FontStyle::Regular
    );
    assert_eq!(FontStyle::Thin - FontStyle::Regular, FontStyle::Thin);
    assert_eq!(FontStyle::Regular - FontStyle::Regular, FontStyle::Regular);
    assert_eq!(
        FontStyle::ExtraLightItalic - FontStyle::ExtraLightItalic,
        FontStyle::Regular
    );
    assert_eq!(FontStyle::BoldItalic - FontStyle::Bold, FontStyle::Italic);
}

#[test]
#[should_panic]
fn panicky_sub_font_style() {
    let _ = FontStyle::Bold - FontStyle::Thin;
}
