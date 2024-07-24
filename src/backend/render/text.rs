use std::{collections::VecDeque, sync::Arc};

use fontdue::Metrics;

use crate::{
    config::{spacing::Spacing, TextAlignment},
    data::text::Text,
};

use super::{color::Bgra, font::FontCollection, font::FontStyle, image::Image};

#[derive(Default)]
pub(crate) struct TextRect {
    words: VecDeque<WordRect>,
    lines: Vec<Line>,

    width: usize,
    height: usize,

    spacebar_width: usize,
    line_height: usize,
    line_spacing: usize,
    max_bearing_y: usize,

    fg_color: Bgra,

    margin: Spacing,
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
            spacebar_width: Self::get_spacebar_width(&font_collection, px_size),
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
            spacebar_width: Self::get_spacebar_width(&font_collection, px_size),
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

    fn convert_to_words(glyph_collection: Vec<LocalGlyph>) -> VecDeque<WordRect> {
        let mut words = VecDeque::new();
        let mut word = vec![];
        for local_glyph in glyph_collection {
            if local_glyph.is_empty() {
                if !word.is_empty() {
                    words.push_back(WordRect::from_local_glyphs(word));
                }
                word = vec![];
            } else {
                word.push(local_glyph);
            }
        }

        if !word.is_empty() {
            words.push_back(WordRect::from_local_glyphs(word));
        }

        words
    }

    fn get_spacebar_width(font_collection: &Arc<FontCollection>, px_size: f32) -> usize {
        font_collection
            .font_by_style(&FontStyle::Regular)
            .font_arc()
            .metrics(' ', px_size)
            .advance_width
            .round() as usize
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

    pub(crate) fn compile(&mut self, mut width: usize, mut height: usize) {
        self.margin.shrink(&mut width, &mut height);
        self.width = width;

        let (bottom_spacing, line_height) = self
            .words
            .iter()
            .map(|word| (word.bottom_spacing, word.line_height))
            .reduce(|lhs, rhs| (std::cmp::max(lhs.0, rhs.0), std::cmp::max(lhs.1, rhs.1)))
            .unwrap_or_default();

        self.max_bearing_y = line_height;
        self.line_height = bottom_spacing + line_height;

        let mut lines = vec![];

        for y in (0..height as isize)
            .step_by(self.line_height + self.line_spacing)
            .take_while(|y| height - *y as usize > self.line_height)
        {
            let mut remaining_width = width;
            let mut words = vec![];
            while let Some(word) = self.words.pop_front() {
                let inserting_width = word.advance_width
                    + if words.len() > 1 {
                        self.spacebar_width
                    } else {
                        0
                    };
                if inserting_width > remaining_width {
                    break;
                }

                words.push(word);
                remaining_width -= inserting_width;
            }

            if words.len() == 0 {
                break;
            }

            lines.push(Line::new(
                y as usize,
                remaining_width as usize + self.spacebar_width * words.len().saturating_sub(1),
                words,
            ))
        }

        let total_lines = lines.len();
        self.height =
            total_lines * self.line_height + total_lines.saturating_sub(1) * self.line_spacing;

        //TODO: add ellipsis to last line of word if needed

        self.lines = lines;
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn draw<O: FnMut(isize, isize, Bgra)>(
        &self,
        text_alignment: &TextAlignment,
        mut callback: O,
    ) {
        let mut y_offset = self.margin.top() as isize;

        for line in &self.lines {
            let remaining_width = line.compute_unfilled_space(self.spacebar_width) as isize;
            let (mut x, x_incrementor) = match text_alignment {
                TextAlignment::Center => (remaining_width / 2, self.spacebar_width as isize),
                TextAlignment::Left => (0, self.spacebar_width as isize),
                TextAlignment::Right => (remaining_width, self.spacebar_width as isize),
                TextAlignment::SpaceBetween => (
                    0,
                    if line.words.len() == 1 {
                        0
                    } else {
                        line.available_space as isize / line.words.len().saturating_sub(1) as isize
                    },
                ),
            };
            x += self.margin.left() as isize;

            for word in &line.words {
                word.glyphs.iter().for_each(|local_glyph| {
                    let (x_shift, _y_shift) = local_glyph.draw(
                        x,
                        y_offset,
                        self.max_bearing_y as isize,
                        &self.fg_color,
                        &mut callback,
                    );
                    x += x_shift;
                });

                x += x_incrementor;
            }

            y_offset += self.line_height as isize + self.line_spacing as isize;
        }
    }
}

struct Line {
    y_offset: usize,
    available_space: usize,

    words: Vec<WordRect>,
}

impl Line {
    fn new(y_offset: usize, available_space: usize, words: Vec<WordRect>) -> Self {
        Self {
            y_offset,
            available_space,
            words,
        }
    }

    #[inline(always = true)]
    fn compute_unfilled_space(&self, spacebar_width: usize) -> usize {
        self.available_space - self.words.len().saturating_sub(1) * spacebar_width
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
