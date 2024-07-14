use derive_more::Display;
use fontdue::FontSettings;
use owned_ttf_parser::{AsFaceRef, OwnedFace};
use rayon::prelude::*;
use std::{
    collections::HashMap,
    ops::{Add, AddAssign, Sub, SubAssign},
    process::Command,
    sync::Arc,
};

use crate::data::{aliases::Result, text::EntityKind};

use super::image::Image;

pub(crate) struct FontCollection {
    map: HashMap<FontStyle, Font>,
    pub(crate) emoji: Option<OwnedFace>,
}

impl FontCollection {
    pub(crate) fn load_by_font_name(font_name: &str) -> Result<Self> {
        let process_result = Command::new("fc-list")
            .args([font_name, "--format", "%{file}:%{style}\n"])
            .output()?;

        let output: String = process_result
            .stdout
            .into_iter()
            .map(|data| data as char)
            .collect();

        let map = output
            .par_split('\n')
            .filter(|line| !line.is_empty())
            .map(|line| {
                let (filepath, styles) = line
                    .split_once(":")
                    .expect("Must be the colon delimiter in --format when calling fc-list");
                Font::try_read(filepath, styles).expect("Can't read the font file")
            })
            .fold(HashMap::new, |mut acc, font| {
                acc.insert(font.style.clone(), font);
                acc
            })
            .reduce(HashMap::new, |mut lhs, rhs| {
                lhs.extend(rhs);
                lhs
            });

        let emoji = OwnedFace::from_vec(
            std::fs::read(
                &Command::new("fc-list")
                    .args(["NotoColorEmoji", "--format", "%{file}"])
                    .output()?
                    .stdout
                    .into_iter()
                    .map(|byte| byte as char)
                    .collect::<String>(),
            )?,
            0,
        )
        .ok();

        Ok(Self { map, emoji })
    }

    pub(crate) fn font_by_style(&self, font_style: &FontStyle) -> &Font {
        self.map.get(font_style).unwrap_or(
            self.map
                .get(&FontStyle::Regular)
                .expect("Not found regular font in Font collection"),
        )
    }

    pub(crate) fn emoji_image(&self, ch: char, size: u16) -> Option<Image> {
        let face = self.emoji.as_ref()?.as_face_ref();
        let glyph_id = face.glyph_index(ch)?;
        Image::from_raster_glyph_image(face.glyph_raster_image(glyph_id, size)?, size as u32)
    }
}

#[derive(Debug)]
pub(crate) struct Font {
    style: FontStyle,
    data: Arc<fontdue::Font>,
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
        let font_settings = FontSettings::default();
        let data = Arc::new(fontdue::Font::from_bytes(bytes, font_settings)?);

        Ok(Self { style, data })
    }

    pub(crate) fn font_arc(&self) -> Arc<fontdue::Font> {
        self.data.clone()
    }
}

#[derive(Debug, Display, Hash, PartialEq, Eq, Clone)]
pub(crate) enum FontStyle {
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
    #[display(fmt = "Black")]
    Black,
    #[display(fmt = "BlacItalic")]
    BlackItalic,
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
            "Black" => Self::Black,
            "Black Italic" => Self::BlackItalic,
            other => panic!("Invalid style: {other}"),
        }
    }
}

impl From<EntityKind> for FontStyle {
    fn from(value: EntityKind) -> Self {
        FontStyle::from(&value)
    }
}

impl From<&EntityKind> for FontStyle {
    fn from(value: &EntityKind) -> Self {
        match value {
            EntityKind::Bold => FontStyle::Bold,
            EntityKind::Italic => FontStyle::Italic,
            other => todo!("Unsupported style {other:?} at current moment"),
        }
    }
}

fn union_font_styles(lhs: &FontStyle, rhs: &FontStyle) -> FontStyle {
    match lhs {
        FontStyle::Bold => match rhs {
            FontStyle::Regular | FontStyle::Bold => FontStyle::Bold,
            FontStyle::Italic | FontStyle::BoldItalic => FontStyle::BoldItalic,
            FontStyle::ExtraBold => FontStyle::ExtraBold,
            FontStyle::ExtraBoldItalic => FontStyle::ExtraBoldItalic,
            FontStyle::SemiBold => FontStyle::SemiBold,
            FontStyle::SemiBoldItalic => FontStyle::SemiBoldItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::Italic => match rhs {
            FontStyle::Regular | FontStyle::Italic => FontStyle::Italic,
            FontStyle::Bold | FontStyle::BoldItalic => FontStyle::BoldItalic,
            FontStyle::ExtraLight | FontStyle::ExtraLightItalic => FontStyle::ExtraLightItalic,
            FontStyle::ExtraBold | FontStyle::ExtraBoldItalic => FontStyle::ExtraBoldItalic,
            FontStyle::Light | FontStyle::LightItalic => FontStyle::LightItalic,
            FontStyle::Thin | FontStyle::ThinItalic => FontStyle::ThinItalic,
            FontStyle::SemiBold | FontStyle::SemiBoldItalic => FontStyle::SemiBoldItalic,
            FontStyle::Medium | FontStyle::MediumItalic => FontStyle::MediumItalic,
            FontStyle::Black | FontStyle::BlackItalic => FontStyle::BlackItalic,
        },
        FontStyle::BoldItalic => match rhs {
            FontStyle::Regular | FontStyle::Bold | FontStyle::Italic | FontStyle::BoldItalic => {
                FontStyle::BoldItalic
            }
            FontStyle::ExtraBold | FontStyle::ExtraBoldItalic => FontStyle::ExtraBoldItalic,
            FontStyle::SemiBold | FontStyle::SemiBoldItalic => FontStyle::SemiBoldItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::ExtraBold => match rhs {
            FontStyle::Regular | FontStyle::Bold | FontStyle::ExtraBold => FontStyle::ExtraBold,
            FontStyle::Italic | FontStyle::BoldItalic | FontStyle::ExtraBoldItalic => {
                FontStyle::ExtraBoldItalic
            }
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::ExtraLight => match rhs {
            FontStyle::Regular | FontStyle::Light | FontStyle::ExtraLight => FontStyle::ExtraLight,
            FontStyle::Italic | FontStyle::ExtraLightItalic | FontStyle::LightItalic => {
                FontStyle::ExtraLightItalic
            }
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::ExtraLightItalic => match rhs {
            FontStyle::Regular
            | FontStyle::Italic
            | FontStyle::ExtraLight
            | FontStyle::ExtraLightItalic
            | FontStyle::LightItalic
            | FontStyle::Light => FontStyle::ExtraLightItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::ExtraBoldItalic => match rhs {
            FontStyle::Regular
            | FontStyle::Bold
            | FontStyle::Italic
            | FontStyle::BoldItalic
            | FontStyle::ExtraBold
            | FontStyle::ExtraBoldItalic => FontStyle::ExtraBoldItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::LightItalic => match rhs {
            FontStyle::Regular | FontStyle::Italic | FontStyle::Light | FontStyle::LightItalic => {
                FontStyle::LightItalic
            }
            FontStyle::ExtraLight | FontStyle::ExtraLightItalic => FontStyle::ExtraLightItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::Thin => match rhs {
            FontStyle::Regular | FontStyle::Thin => FontStyle::Thin,
            FontStyle::Italic | FontStyle::ThinItalic => FontStyle::ThinItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::ThinItalic => match rhs {
            FontStyle::Regular | FontStyle::Thin | FontStyle::Italic | FontStyle::ThinItalic => {
                FontStyle::ThinItalic
            }
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::SemiBold => match rhs {
            FontStyle::Regular | FontStyle::Bold | FontStyle::SemiBold => FontStyle::SemiBold,
            FontStyle::Italic | FontStyle::BoldItalic | FontStyle::SemiBoldItalic => {
                FontStyle::SemiBoldItalic
            }
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::SemiBoldItalic => match rhs {
            FontStyle::Regular
            | FontStyle::Bold
            | FontStyle::SemiBold
            | FontStyle::Italic
            | FontStyle::BoldItalic
            | FontStyle::SemiBoldItalic => FontStyle::SemiBoldItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::Medium => match rhs {
            FontStyle::Regular | FontStyle::Medium => FontStyle::Medium,
            FontStyle::Italic | FontStyle::MediumItalic => FontStyle::MediumItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::Regular => rhs.clone(),
        FontStyle::Light => match rhs {
            FontStyle::Regular | FontStyle::Light => FontStyle::Light,
            FontStyle::Italic | FontStyle::LightItalic => FontStyle::LightItalic,
            FontStyle::ExtraLight => FontStyle::ExtraLight,
            FontStyle::ExtraLightItalic => FontStyle::ExtraLightItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::MediumItalic => match rhs {
            FontStyle::Regular
            | FontStyle::Medium
            | FontStyle::Italic
            | FontStyle::MediumItalic => FontStyle::MediumItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::Black => match rhs {
            FontStyle::Regular | FontStyle::Black => FontStyle::Black,
            FontStyle::Italic | FontStyle::BlackItalic => FontStyle::BlackItalic,
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
        FontStyle::BlackItalic => match rhs {
            FontStyle::Regular | FontStyle::Black | FontStyle::Italic | FontStyle::BlackItalic => {
                FontStyle::BlackItalic
            }
            other => panic!("Incorrect combination of {lhs} and {other}"),
        },
    }
}

impl Add for FontStyle {
    type Output = FontStyle;

    fn add(self, rhs: Self) -> Self::Output {
        union_font_styles(&self, &rhs)
    }
}

impl Add for &FontStyle {
    type Output = FontStyle;

    fn add(self, rhs: Self) -> Self::Output {
        union_font_styles(self, rhs)
    }
}

impl AddAssign for FontStyle {
    fn add_assign(&mut self, rhs: Self) {
        *self = union_font_styles(self, &rhs);
    }
}

fn intersect_font_styles(lhs: &FontStyle, rhs: &FontStyle) -> FontStyle {
    match lhs {
        FontStyle::Bold => match rhs {
            FontStyle::Regular => FontStyle::Bold,
            FontStyle::Bold => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::Italic => match rhs {
            FontStyle::Regular => FontStyle::Italic,
            FontStyle::Italic => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::BoldItalic => match rhs {
            FontStyle::Bold => FontStyle::Italic,
            FontStyle::Italic => FontStyle::Bold,
            FontStyle::BoldItalic => FontStyle::Regular,
            FontStyle::Regular => FontStyle::BoldItalic,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::ExtraBold => match rhs {
            FontStyle::Bold => FontStyle::Bold,
            FontStyle::ExtraBold => FontStyle::Regular,
            FontStyle::Regular => FontStyle::ExtraBold,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::ExtraLight => match rhs {
            FontStyle::Regular => FontStyle::ExtraLight,
            FontStyle::ExtraLight => FontStyle::Regular,
            FontStyle::Light => FontStyle::Light,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::ExtraLightItalic => match rhs {
            FontStyle::Italic => FontStyle::ExtraLight,
            FontStyle::ExtraLight => FontStyle::Italic,
            FontStyle::ExtraLightItalic => FontStyle::Regular,
            FontStyle::LightItalic => FontStyle::Light,
            FontStyle::Regular => FontStyle::ExtraLightItalic,
            FontStyle::Light => FontStyle::LightItalic,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::ExtraBoldItalic => match rhs {
            FontStyle::Bold => FontStyle::BoldItalic,
            FontStyle::Italic => FontStyle::ExtraBold,
            FontStyle::BoldItalic => FontStyle::Bold,
            FontStyle::ExtraBold => FontStyle::Italic,
            FontStyle::ExtraBoldItalic => FontStyle::Regular,
            FontStyle::Regular => FontStyle::ExtraBoldItalic,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::LightItalic => match rhs {
            FontStyle::Italic => FontStyle::Light,
            FontStyle::LightItalic => FontStyle::Regular,
            FontStyle::Regular => FontStyle::LightItalic,
            FontStyle::Light => FontStyle::Italic,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::Thin => match rhs {
            FontStyle::Thin => FontStyle::Regular,
            FontStyle::Regular => FontStyle::Thin,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::ThinItalic => match rhs {
            FontStyle::Italic => FontStyle::Thin,
            FontStyle::Thin => FontStyle::Italic,
            FontStyle::ThinItalic => FontStyle::Regular,
            FontStyle::Regular => FontStyle::ThinItalic,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::SemiBold => match rhs {
            FontStyle::SemiBold => FontStyle::Regular,
            FontStyle::Regular => FontStyle::SemiBold,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::SemiBoldItalic => match rhs {
            FontStyle::Italic => FontStyle::SemiBold,
            FontStyle::SemiBold => FontStyle::Italic,
            FontStyle::SemiBoldItalic => FontStyle::Regular,
            FontStyle::Regular => FontStyle::SemiBoldItalic,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::Medium => match rhs {
            FontStyle::Medium => FontStyle::Regular,
            FontStyle::Regular => FontStyle::Medium,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::Regular => match rhs {
            FontStyle::Regular => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::Light => match rhs {
            FontStyle::Regular => FontStyle::Light,
            FontStyle::Light => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::MediumItalic => match rhs {
            FontStyle::Regular => FontStyle::MediumItalic,
            FontStyle::Italic => FontStyle::Medium,
            FontStyle::Medium => FontStyle::Italic,
            FontStyle::MediumItalic => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::Black => match rhs {
            FontStyle::Regular => FontStyle::Black,
            FontStyle::Black => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
        FontStyle::BlackItalic => match rhs {
            FontStyle::Regular => FontStyle::BlackItalic,
            FontStyle::Black => FontStyle::Italic,
            FontStyle::Italic => FontStyle::Black,
            FontStyle::BlackItalic => FontStyle::Regular,
            other => panic!("Incorrect intersection from {lhs} by {other}"),
        },
    }
}

impl Sub for FontStyle {
    type Output = FontStyle;

    fn sub(self, rhs: Self) -> Self::Output {
        intersect_font_styles(&self, &rhs)
    }
}

impl Sub for &FontStyle {
    type Output = FontStyle;

    fn sub(self, rhs: Self) -> Self::Output {
        intersect_font_styles(self, rhs)
    }
}

impl SubAssign for FontStyle {
    fn sub_assign(&mut self, rhs: Self) {
        *self = intersect_font_styles(self, &rhs);
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
