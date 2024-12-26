use ab_glyph::{point, Font as AbGlyphFont, OutlinedGlyph, ScaleFont};
use derive_more::Display;
use log::{debug, error, info, warn};
use std::{
    collections::HashMap,
    ffi::c_void,
    ops::{Add, AddAssign, Sub, SubAssign},
    os::fd::AsRawFd,
    process::Command,
};

use config::text::TextStyle;
use dbus::text::EntityKind;

use crate::drawer::Drawer;

use super::{
    color::Bgra,
    image::Image,
    widget::{Coverage, Draw, DrawColor},
};

pub struct FontCollection {
    font_name: String,
    font_map: HashMap<FontStyle, Font>,
    math_font: Option<MathFont>,
    emoji_font: Option<EmojiFont>,
}

impl FontCollection {
    const ELLIPSIS: char = '…';
    const ACCEPTED_STYLES: [&'static str; 3] = ["Regular", "Bold", "Italic"];

    pub fn update_by_font_name(&mut self, font_name: &str) -> anyhow::Result<()> {
        if self.font_name == font_name {
            return Ok(());
        }

        *self = Self::load_by_font_name(font_name)?;
        Ok(())
    }

    pub fn load_by_font_name(font_name: &str) -> anyhow::Result<Self> {
        debug!("Font: Trying load font by name {font_name}");

        let output: String = Command::new("fc-list")
            .args([font_name, "--format", "%{file}:%{style}\n"])
            .output()?
            .stdout
            .into_iter()
            .map(|data| data as char)
            .collect();

        let mut font_map = HashMap::new();

        for (filepath, styles) in output
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(|line| {
                line.split_once(':')
                    .expect("Must be the colon delimiter in --format when calling fc-list")
            })
            .filter(|(_, style)| {
                Self::ACCEPTED_STYLES
                    .contains(&style.split_once(' ').map(|(lhs, _)| lhs).unwrap_or(style))
            })
        {
            let font = match Font::try_read(filepath, styles) {
                Ok(font) => font,
                Err(err) => {
                    error!("Failed to read or parse font at {filepath}. Error: {err}");
                    continue;
                }
            };

            font_map.insert(font.style.clone(), font);
        }

        info!("Font: Loaded fonts by name {font_name}");

        let math_font = match MathFont::try_create() {
            Ok(emoji) => Some(emoji),
            Err(err) => {
                warn!("Font: Not found the 'NotoSansMath' font, math symbols will not be displayed. Error: {err}");
                None
            }
        };

        let emoji_font = match EmojiFont::try_create() {
            Ok(emoji) => Some(emoji),
            Err(err) => {
                warn!("Font: Not found the 'NotoColorEmoj' font, emoji will not be displayed. Error: {err}");
                None
            }
        };

        Ok(Self {
            font_name: font_name.to_owned(),
            font_map,
            math_font,
            emoji_font,
        })
    }

    pub fn load_glyph_by_style(&self, font_style: &FontStyle, ch: char, px_size: f32) -> Glyph {
        let font = self.font_map.get(font_style).unwrap_or(self.default_font());

        font.load_glyph(ch, px_size)
            .or_else(|| {
                self.math_font
                    .as_ref()
                    .map(|math_font| math_font.load_glyph(ch, px_size))
                    .unwrap_or_default()
            })
            .or_else(|| {
                self.emoji_font
                    .as_ref()
                    .and_then(|emoji| emoji.image(ch, px_size.round() as u16))
                    .map(Glyph::Image)
                    .unwrap_or_default()
            })
    }

    pub fn max_height(&self, px_size: f32) -> usize {
        self.font_map
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
        self.font_map
            .get(&FontStyle::Regular)
            .expect("Not found regular font in Font collection")
    }
}

#[derive(Debug)]
pub struct Font {
    style: FontStyle,
    _buffer: Buffer<u8>,
    /// WARNING: DON'T CLONE THIS FIELD
    data: ab_glyph::FontRef<'static>,
}

impl Font {
    fn try_read(filepath: &str, styles: &str) -> anyhow::Result<Self> {
        let style = FontStyle::from(
            styles
                .split_once(",")
                .map(|(important_style, _)| important_style)
                .unwrap_or(styles),
        );

        let file = std::fs::File::open(filepath)?;
        let buffer = Buffer::from(file);
        let data = ab_glyph::FontRef::try_from_slice(buffer.as_slice())?;

        Ok(Self {
            style,
            _buffer: buffer,
            data,
        })
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

struct MathFont(Font);

impl MathFont {
    fn try_create() -> anyhow::Result<Self> {
        let filepath = Command::new("fc-list")
            .args(["NotoSansMath", "--format", "%{file}"])
            .output()?
            .stdout
            .into_iter()
            .map(|byte| byte as char)
            .collect::<String>();

        Font::try_read(&filepath, "Regular").map(MathFont)
    }
}

impl std::ops::Deref for MathFont {
    type Target = Font;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct EmojiFont {
    _buffer: Buffer<u8>,
    font_face: ttf_parser::Face<'static>,
}

impl EmojiFont {
    fn try_create() -> anyhow::Result<Self> {
        let filepath = Command::new("fc-list")
            .args(["NotoColorEmoji", "--format", "%{file}"])
            .output()?
            .stdout
            .into_iter()
            .map(|byte| byte as char)
            .collect::<String>();
        let file = std::fs::File::open(filepath)?;
        let buffer = Buffer::from(file);

        let font_face = ttf_parser::Face::parse(buffer.as_slice(), 0)?;
        Ok(Self {
            _buffer: buffer,
            font_face,
        })
    }

    pub fn image(&self, ch: char, size: u16) -> Option<Image> {
        let glyph_id = self.font_face.glyph_index(ch)?;
        Image::from_raster_glyph_image(
            self.font_face.glyph_raster_image(glyph_id, size)?,
            size as u32,
        )
    }
}

/// Container of data like Vec<T>.
///
/// Because of Rust allocator won't allocate memory if it can be reusable for other application
/// parts, the memory will grow when the big data allocates. Sometimes it won't be deallocated that
/// is not good.
///
/// The implementation of Buffer<T> uses libc allocator and forces to deallocate when it drops. It
/// should guarantee that the memory won't be used for other application's parts.
#[derive(Debug)]
struct Buffer<T> {
    ptr: *const c_void,
    size: usize,
    _phantom_data: std::marker::PhantomData<[T]>,
}

impl<T> Buffer<T> {
    /// Allocates new memory by len in bytes (u8).
    fn new(size: usize) -> Buffer<T> {
        unsafe {
            Self {
                ptr: libc::malloc(size),
                size,
                _phantom_data: std::marker::PhantomData,
            }
        }
    }

    /// Converts the buffer into Rust's slice. Make attention to **'static** lifetime. It made for
    /// self-referential struct.
    ///
    /// **Safety**:
    ///
    /// It will be safe if the application WON'T use after buffer's drop. Otherwise the further
    /// actions will be UB.
    fn as_slice(&self) -> &'static [T] {
        unsafe { std::slice::from_raw_parts(self.ptr.cast(), self.size) }
    }
}

impl From<std::fs::File> for Buffer<u8> {
    fn from(file: std::fs::File) -> Self {
        let buffer = Buffer::new(file.metadata().map(|m| m.len() as usize).ok().unwrap_or(0));
        unsafe {
            // INFO: don't mind about buffer read size because the size always fits to buffer
            let _read_size = libc::read(file.as_raw_fd(), buffer.ptr.cast_mut(), buffer.size);
        }

        buffer
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe { libc::free(self.ptr.cast_mut()) }
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

    pub fn or_else<F: FnOnce() -> Self>(self, other: F) -> Self {
        match self {
            Glyph::Empty => other(),
            _ => self,
        }
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
    fn draw_with_offset(&self, offset: &super::types::Offset, drawer: &mut Drawer) {
        match self {
            Glyph::Image(img) => {
                img.draw_with_offset(offset, drawer);
            }
            Glyph::Outline {
                color,
                outlined_glyph,
                ..
            } => {
                let bounds = outlined_glyph.px_bounds();
                outlined_glyph.draw(|x, y, coverage| {
                    drawer.draw_color(
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
