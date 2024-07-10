use std::{collections::VecDeque, sync::Arc};

use fontdue::Metrics;

use crate::{
    data::text::{EntityKind, Text},
    render::font::FontStyle,
};

use super::{color::Bgra, font::FontCollection, image::Image};

#[derive(Default)]
pub(crate) struct TextRect {
    words: Vec<TextObject>,
    line_spacing: usize,
    padding: usize,
}

impl TextRect {
    pub(crate) fn from_str(
        string: &str,
        px_size: f32,
        font_collection: Arc<FontCollection>,
    ) -> Self {
        let font = font_collection
            .font_by_style(&FontStyle::Regular)
            .font_arc();
        {}
        let glyph_collection: Vec<LocalGlyph> = string
            .chars()
            .map(|ch| {
                if !ch.is_whitespace() {
                    let glyph_id = font.lookup_glyph_index(ch);
                    if glyph_id != 0 {
                        LocalGlyph::Outline(
                            font.rasterize_indexed(font.lookup_glyph_index(ch), px_size),
                        )
                    } else {
                        font_collection
                            .emoji_image(ch, px_size as u16)
                            .map(|image| LocalGlyph::Image(image))
                            .unwrap_or(LocalGlyph::Empty)
                    }
                } else {
                    LocalGlyph::Empty
                }
            })
            .collect();

        let words = Self::convert_to_words(glyph_collection);
        Self {
            words,
            ..Default::default()
        }
    }

    pub(crate) fn from_text(
        text: &Text,
        px_size: f32,
        font_collection: Arc<FontCollection>,
    ) -> Self {
        let Text { body, entities } = text;

        let mut entities = VecDeque::from_iter(entities.iter());
        let mut current_entities = VecDeque::new();
        let mut current_style = FontStyle::Regular;

        let glyph_collection: Vec<LocalGlyph> = body
            .chars()
            .enumerate()
            .map(|(pos, ch)| {
                while let Some(entity) = entities.front() {
                    if entity.offset == pos {
                        match entity.kind {
                            EntityKind::Bold => current_style += FontStyle::Bold,
                            EntityKind::Italic => current_style += FontStyle::Italic,
                            _ => todo!("Unsupported styles at current moment"),
                        }
                        current_entities.push_back(entities.pop_front().unwrap());
                    } else {
                        break;
                    }
                }

                let glyph = if !ch.is_whitespace() {
                    let font = font_collection.font_by_style(&current_style).font_arc();
                    // LocalGlyph::Outline(
                    //     font.rasterize_indexed(font.lookup_glyph_index(ch), px_size),
                    // )
                    let glyph_id = font.lookup_glyph_index(ch);
                    if glyph_id != 0 {
                        LocalGlyph::Outline(
                            font.rasterize_indexed(font.lookup_glyph_index(ch), px_size),
                        )
                    } else {
                        font_collection
                            .emoji_image(ch, px_size as u16)
                            .map(|image| LocalGlyph::Image(image))
                            .unwrap_or(LocalGlyph::Empty)
                    }
                } else {
                    LocalGlyph::Empty
                };

                while let Some(entity) = current_entities.front() {
                    if entity.offset + entity.length < pos {
                        let entity = current_entities.pop_front().unwrap();
                        match entity.kind {
                            EntityKind::Bold => current_style -= FontStyle::Bold,
                            EntityKind::Italic => current_style -= FontStyle::Italic,
                            _ => todo!("Unsupported styles at current moment"),
                        }
                    } else {
                        break;
                    }
                }

                glyph
            })
            .collect();

        let words = Self::convert_to_words(glyph_collection);
        Self {
            words,
            ..Default::default()
        }
    }

    fn convert_to_words(glyph_collection: Vec<LocalGlyph>) -> Vec<TextObject> {
        let mut words = vec![];
        let mut word = vec![];
        for local_glyph in glyph_collection {
            if local_glyph.is_empty() {
                if !word.is_empty() {
                    words.push(TextObject::from_local_glyphs(word));
                }
                word = vec![];
            } else {
                word.push(local_glyph);
            }
        }

        if !word.is_empty() {
            words.push(TextObject::from_local_glyphs(word));
        }

        words
    }

    pub(crate) fn set_line_spacing(&mut self, line_spacing: usize) {
        self.line_spacing = line_spacing;
    }

    pub(crate) fn set_padding(&mut self, padding: usize) {
        self.padding = padding;
    }

    pub(crate) fn draw<O: FnMut(isize, isize, Bgra)>(
        &self,
        width: usize,
        height: usize,
        text_alignment: TextAlignment,
        mut callback: O,
    ) -> usize {
        const SPACEBAR_WIDTH: isize = 15;
        let bottom = height - self.padding;

        let (bottom_spacing, line_height) = self
            .words
            .iter()
            .map(|word| (word.bottom_spacing, word.line_height))
            .reduce(|lhs, rhs| (std::cmp::max(lhs.0, rhs.0), std::cmp::max(lhs.1, rhs.1)))
            .unwrap_or_default();

        let total_height = bottom_spacing + line_height as usize;

        let mut actual_height = self.padding;
        let mut word_index = 0;

        for y in (self.padding as isize..bottom as isize)
            .step_by(total_height + self.line_spacing)
            .take_while(|y| bottom - *y as usize > total_height)
        {
            let mut remaining_width = (width - self.padding * 2) as isize;
            let mut words = vec![];
            while let Some(word) = self.words.get(word_index) {
                let new_width =
                    word.advance_width as isize + if words.len() > 1 { SPACEBAR_WIDTH } else { 0 };
                if remaining_width < 0 || new_width > remaining_width {
                    break;
                }

                words.push(word);
                word_index += 1;
                remaining_width -=
                    word.advance_width as isize + if words.len() > 1 { SPACEBAR_WIDTH } else { 0 };
            }

            if words.len() == 0 {
                break;
            }

            let (mut x, x_incrementor) = match text_alignment {
                TextAlignment::Center => (remaining_width / 2, SPACEBAR_WIDTH),
                TextAlignment::Left => (0, SPACEBAR_WIDTH),
                TextAlignment::Right => (remaining_width, SPACEBAR_WIDTH),
                TextAlignment::SpaceBetween => (
                    0,
                    if words.len() == 1 {
                        0
                    } else {
                        let count = words.len() as isize - 1;
                        (remaining_width + SPACEBAR_WIDTH * count) / count
                    },
                ),
            };
            x += self.padding as isize;

            for word in words {
                word.glyphs
                    .iter()
                    .for_each(|local_glyph| match local_glyph {
                        LocalGlyph::Image(img) => {
                            img.draw_by_xy(|img_x, img_y, bgra| {
                                callback(img_x + x, img_y + y, bgra)
                            });
                            x += img.width().unwrap_or_default() as isize;
                        }
                        LocalGlyph::Outline((metrics, coverage)) => {
                            let mut coverage_iter = coverage.iter();
                            let (width, height) = (metrics.width as isize, metrics.height as isize);
                            let y_diff = -metrics.ymin as isize + line_height as isize - height;
                            let x_diff = metrics.xmin as isize;

                            for glyph_y in y_diff..height + y_diff {
                                for glyph_x in x_diff..width + x_diff {
                                    let mut bgra = Bgra::new();
                                    bgra.alpha = *coverage_iter.next().unwrap() as f32 / 255.0;
                                    callback(x + glyph_x, y as isize + glyph_y, bgra);
                                }
                            }

                            x += metrics.advance_width.round() as isize;
                        }
                        LocalGlyph::Empty => unreachable!(),
                    });

                x += x_incrementor;
            }

            actual_height += total_height + self.line_spacing;
        }

        if actual_height > self.padding {
            actual_height -= self.line_spacing;
        }

        actual_height
    }
}

enum LocalGlyph {
    Image(Image),
    Outline((Metrics, Vec<u8>)),
    Empty,
}

impl LocalGlyph {
    fn is_empty(&self) -> bool {
        if let LocalGlyph::Empty = self {
            true
        } else {
            false
        }
    }
}

pub(crate) struct TextObject {
    advance_width: usize,
    line_height: usize,
    bottom_spacing: usize,
    glyphs: Vec<LocalGlyph>,
}

impl TextObject {
    fn from_local_glyphs(outlined_glyphs: Vec<LocalGlyph>) -> Self {
        let (advance_width, bottom_spacing, line_height) = outlined_glyphs
            .iter()
            .map(|local_glyph| match local_glyph {
                LocalGlyph::Image(img) => (
                    img.width().unwrap_or_default() as f32,
                    0,
                    img.height().unwrap_or_default() as usize,
                ),
                LocalGlyph::Outline((metrics, _)) => (
                    metrics.advance_width,
                    metrics.ymin,
                    (metrics.height as i32 + metrics.ymin) as usize,
                ),
                LocalGlyph::Empty => panic!("TextObject have an empty glyph, which is nonsense!"),
            })
            .reduce(|lhs, rhs| {
                (
                    lhs.0 + rhs.0,
                    std::cmp::min(lhs.1, rhs.1),
                    std::cmp::max(lhs.2, rhs.2),
                )
            })
            .unwrap();

        Self {
            advance_width: advance_width.round() as usize,
            bottom_spacing: bottom_spacing.abs() as usize,
            line_height,
            glyphs: outlined_glyphs,
        }
    }
}

#[derive(Default)]
#[allow(unused)]
pub(crate) enum TextAlignment {
    Center,
    #[default]
    Left,
    Right,
    SpaceBetween,
}
