use std::{collections::VecDeque, sync::Arc};

use fontdue::Metrics;

use crate::{
    config::{spacing::Spacing, TextAlignment},
    data::text::Text,
};

use super::{color::Bgra, font::FontCollection, font::FontStyle, image::Image};

#[derive(Default)]
pub(crate) struct TextRect {
    words: Vec<WordRect>,
    line_spacing: usize,
    margin: Spacing,
    fg_color: Bgra,
}

impl TextRect {
    pub(crate) fn from_str(
        string: &str,
        px_size: f32,
        font_collection: Arc<FontCollection>,
    ) -> Self {
        let glyph_collection: Vec<LocalGlyph> = string
            .chars()
            .map(|ch| Self::load_glyph_by_style(ch, &FontStyle::Regular, px_size, &font_collection))
            .collect();

        let words = Self::convert_to_words(glyph_collection);
        Self {
            words,
            fg_color: Bgra::new_black(),
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
                        current_style += FontStyle::from(&entity.kind);
                        current_entities.push_back(entities.pop_front().unwrap());
                    } else {
                        break;
                    }
                }

                let glyph =
                    Self::load_glyph_by_style(ch, &current_style, px_size, &font_collection);

                while let Some(entity) = current_entities.front() {
                    if entity.offset + entity.length < pos {
                        let entity = current_entities.pop_front().unwrap();
                        current_style -= FontStyle::from(&entity.kind);
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

    fn load_glyph_by_style(
        ch: char,
        style: &FontStyle,
        px_size: f32,
        font_collection: &Arc<FontCollection>,
    ) -> LocalGlyph {
        if ch.is_whitespace() {
            return LocalGlyph::Empty;
        }

        let font = font_collection.font_by_style(style).font_arc();
        let glyph_id = font.lookup_glyph_index(ch);
        if glyph_id != 0 {
            LocalGlyph::Outline(font.rasterize_indexed(font.lookup_glyph_index(ch), px_size))
        } else {
            font_collection
                .emoji_image(ch, px_size as u16)
                .map(|image| LocalGlyph::Image(image))
                .unwrap_or(LocalGlyph::Empty)
        }
    }

    fn convert_to_words(glyph_collection: Vec<LocalGlyph>) -> Vec<WordRect> {
        let mut words = vec![];
        let mut word = vec![];
        for local_glyph in glyph_collection {
            if local_glyph.is_empty() {
                if !word.is_empty() {
                    words.push(WordRect::from_local_glyphs(word));
                }
                word = vec![];
            } else {
                word.push(local_glyph);
            }
        }

        if !word.is_empty() {
            words.push(WordRect::from_local_glyphs(word));
        }

        words
    }

    pub(crate) fn set_line_spacing(&mut self, line_spacing: usize) {
        self.line_spacing = line_spacing;
    }

    pub(crate) fn set_margin(&mut self, margin: &Spacing) {
        self.margin = margin.clone();
    }

    pub(crate) fn set_foreground(&mut self, color: Bgra) {
        self.fg_color = color;
    }

    pub(crate) fn draw<O: FnMut(isize, isize, Bgra)>(
        &self,
        width: usize,
        height: usize,
        text_alignment: &TextAlignment,
        mut callback: O,
    ) -> usize {
        const SPACEBAR_WIDTH: isize = 15;
        let bottom = height - self.margin.bottom() as usize;

        let (bottom_spacing, line_height) = self
            .words
            .iter()
            .map(|word| (word.bottom_spacing, word.line_height))
            .reduce(|lhs, rhs| (std::cmp::max(lhs.0, rhs.0), std::cmp::max(lhs.1, rhs.1)))
            .unwrap_or_default();

        let total_height = bottom_spacing + line_height as usize;

        let mut actual_height = self.margin.top() as usize;
        let mut word_index = 0;

        for y in (self.margin.top() as isize..bottom as isize)
            .step_by(total_height + self.line_spacing)
            .take_while(|y| bottom - *y as usize > total_height)
        {
            let mut remaining_width =
                (width - self.margin.left() as usize - self.margin.right() as usize) as isize;
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
            x += self.margin.left() as isize;

            for word in words {
                word.glyphs.iter().for_each(|local_glyph| {
                    let (x_shift, _y_shift) = local_glyph.draw(
                        x,
                        y as isize,
                        line_height as isize,
                        &self.fg_color,
                        &mut callback,
                    );
                    x += x_shift;
                });

                x += x_incrementor;
            }

            actual_height += total_height + self.line_spacing;
        }

        if actual_height > self.margin.top() as usize {
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

    fn draw<O: FnMut(isize, isize, Bgra)>(
        &self,
        x_offset: isize,
        y_offset: isize,
        line_height: isize,
        fg_color: &Bgra,
        callback: &mut O,
    ) -> (isize, isize) {
        match self {
            LocalGlyph::Image(img) => {
                img.draw_by_xy(|img_x, img_y, bgra| {
                    callback(img_x + x_offset, img_y + y_offset, bgra)
                });
                (
                    img.width().unwrap_or_default() as isize,
                    img.height().unwrap_or_default() as isize,
                )
            }
            LocalGlyph::Outline((metrics, coverage)) => {
                let mut coverage_iter = coverage.iter();
                let (width, height) = (metrics.width as isize, metrics.height as isize);
                let y_diff = -metrics.ymin as isize + line_height - height;
                let x_diff = metrics.xmin as isize;

                for glyph_y in y_diff..height + y_diff {
                    for glyph_x in x_diff..width + x_diff {
                        let bgra =
                            fg_color.clone() * (*coverage_iter.next().unwrap() as f32 / 255.0);
                        callback(x_offset + glyph_x, y_offset as isize + glyph_y, bgra);
                    }
                }

                (
                    metrics.advance_width.round() as isize,
                    metrics.advance_height.round() as isize,
                )
            }
            LocalGlyph::Empty => unreachable!(),
        }
    }
}

pub(crate) struct WordRect {
    advance_width: usize,
    line_height: usize,
    bottom_spacing: usize,
    glyphs: Vec<LocalGlyph>,
}

impl WordRect {
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
                LocalGlyph::Empty => panic!("WordRect have an empty glyph, which is nonsense!"),
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
