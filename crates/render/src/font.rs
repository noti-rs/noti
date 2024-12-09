use ab_glyph::{point, Font as AbGlyphFont, OutlinedGlyph, ScaleFont};
use derive_more::Display;
use log::{debug, info, warn};
use owned_ttf_parser::{AsFaceRef, OwnedFace};
use rayon::prelude::*;
use std::{
    collections::HashMap,
    ops::{Add, AddAssign, Sub, SubAssign},
    process::Command,
};

use config::text::TextStyle;
use dbus::text::EntityKind;

use super::{
    color::Bgra,
    image::Image,
    widget::{Coverage, Draw, DrawColor},
};

pub struct FontCollection {
    map: HashMap<FontStyle, Font>,
    pub emoji: Option<OwnedFace>,
}

impl FontCollection {
    const ELLIPSIS: char = '…';
    const ACCEPTED_STYLES: [&'static str; 3] = ["Regular", "Bold", "Italic"];

    pub fn load_by_font_name(font_name: &str) -> anyhow::Result<Self> {
        debug!("Font: Trying load font by name {font_name}");

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
                line.split_once(':')
                    .expect("Must be the colon delimiter in --format when calling fc-list")
            })
            .filter(|(_, style)| {
                Self::ACCEPTED_STYLES
                    .contains(&style.split_once(' ').map(|(lhs, _)| lhs).unwrap_or(style))
            })
            .fold(HashMap::new, |mut acc, (filepath, styles)| {
                let font = Font::try_read(filepath, styles).expect("Can't read the font file");
                acc.insert(font.style.clone(), font);
                acc
            })
            .reduce(HashMap::new, |mut lhs, rhs| {
                lhs.extend(rhs);
                lhs
            });

        info!("Font: Loaded fonts by name {font_name}");

        let emoji = OwnedFace::from_vec(
            std::fs::read(
                Command::new("fc-list")
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

        if emoji.is_none() {
            warn!("Font: Not found the 'NotoColorEmoj' font, emoji will be not displayed");
        }

        Ok(Self { map, emoji })
    }

    pub fn load_glyph_by_style(&self, font_style: &FontStyle, ch: char, px_size: f32) -> Glyph {
        let font = self.map.get(font_style).unwrap_or(self.default_font());
        let glyph = font.load_glyph(ch, px_size);

        if glyph.is_empty() {
            self.emoji_image(ch, px_size.round() as u16)
                .map(Glyph::Image)
                .unwrap_or(Glyph::Empty)
        } else {
            glyph
        }
    }

    pub fn emoji_image(&self, ch: char, size: u16) -> Option<Image> {
        let face = self.emoji.as_ref()?.as_face_ref();
        let glyph_id = face.glyph_index(ch)?;
        Image::from_raster_glyph_image(face.glyph_raster_image(glyph_id, size)?, size as u32)
    }

    pub fn max_height(&self, px_size: f32) -> usize {
        self.map
            .values()
            .map(|font| font.get_height(px_size).round() as usize)
            .max()
            .unwrap_or_default()
    }

    pub fn get_spacebar_width(&self, px_size: f32) -> f32 {
        self.default_font().get_glyph_width(' ', px_size)
    }

    pub fn get_ellipsis(&self, px_size: f32) -> Glyph {
        self.load_glyph_by_style(&FontStyle::Regular, Self::ELLIPSIS, px_size)
    }

    fn default_font(&self) -> &Font {
        self.map
            .get(&FontStyle::Regular)
            .expect("Not found regular font in Font collection")
    }
}

#[derive(Debug)]
pub struct Font {
    style: FontStyle,
    data: ab_glyph::FontVec,
}

impl Font {
    fn try_read(filepath: &str, styles: &str) -> anyhow::Result<Self> {
        let style = FontStyle::from(
            styles
                .split_once(",")
                .map(|(important_style, _)| important_style)
                .unwrap_or(styles),
        );

        let bytes = std::fs::read(filepath)?;
        let data = ab_glyph::FontVec::try_from_vec(bytes)?;

        Ok(Self { style, data })
    }

    pub fn get_height(&self, px_size: f32) -> f32 {
        self.data.as_scaled(px_size).height()
    }

    pub fn get_glyph_width(&self, ch: char, px_size: f32) -> f32 {
        let scaled_font = self.data.as_scaled(px_size);

        let glyph_id = scaled_font.glyph_id(ch);
        scaled_font.h_advance(glyph_id)
    }

    pub fn load_glyph(&self, ch: char, px_size: f32) -> Glyph {
        if ch.is_whitespace() {
            return Glyph::Empty;
        }

        let scaled_font = self.data.as_scaled(px_size);
        let glyph_id = self.data.glyph_id(ch);

        if glyph_id.0 == 0 {
            return Glyph::Empty;
        }

        let glyph = glyph_id.with_scale_and_position(px_size, point(0.0, scaled_font.ascent()));

        if let Some(outlined_glyph) = self.data.outline_glyph(glyph) {
            Glyph::Outline {
                advance_width: scaled_font.h_advance(outlined_glyph.glyph().id),
                outlined_glyph,
                color: Bgra::new(),
            }
        } else {
            Glyph::Empty
        }
    }
}

#[derive(Default, Clone)]
pub enum Glyph {
    Image(Image),
    Outline {
        color: Bgra,
        advance_width: f32,
        outlined_glyph: OutlinedGlyph,
    },
    #[default]
    Empty,
}

impl Glyph {
    pub fn is_empty(&self) -> bool {
        matches!(self, Glyph::Empty)
    }

    pub fn set_color(&mut self, new_color: Bgra) {
        if let Glyph::Outline { color, .. } = self {
            *color = new_color;
        }
    }

    pub fn advance_width(&self) -> usize {
        match self {
            Glyph::Image(img) => img.width().unwrap_or_default(),
            Glyph::Outline { advance_width, .. } => advance_width.round() as usize,
            Glyph::Empty => 0,
        }
    }
}

impl Draw for Glyph {
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &super::types::Offset,
        output: &mut Output,
    ) {
        match self {
            Glyph::Image(img) => {
                img.draw_with_offset(offset, output);
            }
            Glyph::Outline {
                color,
                outlined_glyph,
                ..
            } => {
                let bounds = outlined_glyph.px_bounds();
                outlined_glyph.draw(|x, y, coverage| {
                    output(
                        (bounds.min.x.round() as i32 + x as i32).clamp(0, i32::MAX) as usize
                            + offset.x,
                        (bounds.min.y.round() as i32 + y as i32).clamp(0, i32::MAX) as usize
                            + offset.y,
                        DrawColor::OverlayWithCoverage(color.to_owned(), Coverage(coverage)),
                    )
                })
            }
            Glyph::Empty => unreachable!(),
        }
    }
}

#[derive(Debug, Display, Hash, PartialEq, Eq, Clone)]
pub enum FontStyle {
    #[display("Regular")]
    Regular,
    #[display("Bold")]
    Bold,
    #[display("Italic")]
    Italic,
    #[display("BoldItalic")]
    BoldItalic,
}

impl From<&str> for FontStyle {
    fn from(value: &str) -> Self {
        match value {
            "Regular" => Self::Regular,
            "Bold" => Self::Bold,
            "Italic" => Self::Italic,
            "Bold Italic" => Self::BoldItalic,
            other => panic!("Unsupported style: {other}"),
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

impl From<TextStyle> for FontStyle {
    fn from(value: TextStyle) -> Self {
        FontStyle::from(&value)
    }
}

impl From<&TextStyle> for FontStyle {
    fn from(value: &TextStyle) -> Self {
        match value {
            TextStyle::Regular => FontStyle::Regular,
            TextStyle::Bold => FontStyle::Bold,
            TextStyle::Italic => FontStyle::Italic,
            TextStyle::BoldItalic => FontStyle::BoldItalic,
        }
    }
}

fn union_font_styles(lhs: &FontStyle, rhs: &FontStyle) -> FontStyle {
    match lhs {
        FontStyle::Bold => match rhs {
            FontStyle::Regular | FontStyle::Bold => FontStyle::Bold,
            FontStyle::Italic | FontStyle::BoldItalic => FontStyle::BoldItalic,
        },
        FontStyle::Italic => match rhs {
            FontStyle::Regular | FontStyle::Italic => FontStyle::Italic,
            FontStyle::Bold | FontStyle::BoldItalic => FontStyle::BoldItalic,
        },
        FontStyle::BoldItalic => lhs.clone(),
        FontStyle::Regular => rhs.clone(),
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
        },
        FontStyle::Regular => match rhs {
            FontStyle::Regular => FontStyle::Regular,
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
        FontStyle::BoldItalic + FontStyle::Bold,
        FontStyle::BoldItalic
    );
    assert_eq!(FontStyle::Bold + FontStyle::Bold, FontStyle::Bold);
    assert_eq!(FontStyle::Regular + FontStyle::Bold, FontStyle::Bold);
    assert_eq!(FontStyle::Regular + FontStyle::Italic, FontStyle::Italic);
    assert_eq!(
        FontStyle::Regular + FontStyle::Italic + FontStyle::Bold,
        FontStyle::BoldItalic
    );
    assert_eq!(FontStyle::Regular + FontStyle::Regular, FontStyle::Regular);
    assert_eq!(FontStyle::Italic + FontStyle::Italic, FontStyle::Italic);
    assert_eq!(
        FontStyle::Regular + FontStyle::BoldItalic + FontStyle::BoldItalic,
        FontStyle::BoldItalic
    );
}

#[test]
fn sub_font_styles() {
    assert_eq!(FontStyle::BoldItalic - FontStyle::Italic, FontStyle::Bold);
    assert_eq!(
        FontStyle::BoldItalic - FontStyle::Italic - FontStyle::Regular,
        FontStyle::Bold
    );
    assert_eq!(FontStyle::Regular - FontStyle::Regular, FontStyle::Regular);
    assert_eq!(FontStyle::BoldItalic - FontStyle::Bold, FontStyle::Italic);
    assert_eq!(
        FontStyle::BoldItalic - FontStyle::Bold - FontStyle::Italic,
        FontStyle::Regular
    );
    assert_eq!(
        FontStyle::BoldItalic - FontStyle::Regular - FontStyle::Italic,
        FontStyle::Bold
    );
}

#[test]
#[should_panic]
fn panicky_sub_font_style() {
    let _ = FontStyle::Bold - FontStyle::Italic;
}
