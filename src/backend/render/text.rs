use std::{collections::VecDeque, sync::Arc};

use fontdue::Metrics;
use itertools::Itertools;

use crate::{
    config::{spacing::Spacing, EllipsizeAt, TextAlignment},
    data::text::Text,
};

use super::{color::Bgra, font::FontCollection, font::FontStyle, image::Image};

#[derive(Default)]
pub(crate) struct TextRect {
    words: VecDeque<WordRect>,
    lines: Vec<LineRect>,

    width: usize,

    spacebar_width: usize,
    line_height: usize,
    line_spacing: usize,
    max_bearing_y: usize,

    ellipsize_at: EllipsizeAt,
    ellipsis: LocalGlyph,

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
            ellipsis: LocalGlyph::Outline(font_collection.get_ellipsis(px_size)),
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
                // TODO: need to refactor it with handling the cases, when inputs a image
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
            ellipsis: LocalGlyph::Outline(font_collection.get_ellipsis(px_size)),
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
        glyph_collection
            .into_iter()
            .chunk_by(|glyph| !glyph.is_empty())
            .into_iter()
            .filter_map(|(matches, word)| {
                matches.then_some(WordRect::from_local_glyphs(word.collect()))
            })
            .collect()
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

    pub(crate) fn set_ellipsize_at(&mut self, ellipsize_at: &EllipsizeAt) {
        self.ellipsize_at = ellipsize_at.clone();
    }

    pub(crate) fn compile(&mut self, mut width: usize, mut height: usize) {
        self.width = width;

        self.margin.shrink(&mut width, &mut height);

        let (bottom_spacing, line_height) = self
            .words
            .iter()
            .map(|word| (word.max_bottom_spacing(), word.max_bearing_y()))
            .reduce(|lhs, rhs| (std::cmp::max(lhs.0, rhs.0), std::cmp::max(lhs.1, rhs.1)))
            .unwrap_or_default();

        self.max_bearing_y = line_height;
        self.line_height = bottom_spacing + line_height;

        let mut lines = vec![];

        for y in (self.margin.top() as usize..height + self.margin.top() as usize)
            .step_by(self.line_height + self.line_spacing)
            .take_while(|y| height - *y >= self.line_height)
        {
            let mut remaining_width = width;
            let mut words = vec![];

            while let Some(word) = self.words.front() {
                let inserting_width = word.advance_width
                    + if !words.is_empty() {
                        self.spacebar_width
                    } else {
                        0
                    };

                if inserting_width > remaining_width {
                    break;
                }

                // SAFETY: the word always valid while it check by `while let` loop around
                words.push(unsafe { self.words.pop_front().unwrap_unchecked() });
                remaining_width -= inserting_width;
            }

            if words.len() == 0 {
                break;
            }

            lines.push(LineRect::new(
                y,
                remaining_width,
                self.spacebar_width,
                words,
            ))
        }

        self.lines = lines;
        self.ellipsize();
    }

    fn ellipsize(&mut self) {
        let mut state = self
            .lines
            .last_mut()
            .map(|last_line| {
                last_line.ellipsize(
                    self.words.pop_front(),
                    self.ellipsis.clone(),
                    &self.ellipsize_at,
                )
            })
            .unwrap_or_default();

        while let EllipsiationState::Continue(word) = state {
            self.lines.pop();

            state = self
                .lines
                .last_mut()
                .map(|last_line| last_line.ellipsize(word, self.ellipsis.clone(), &self.ellipsize_at))
                .unwrap_or_default();
        }
    }

    #[allow(unused)]
    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        let total_lines = self.lines.len();
        total_lines * self.line_height
            + total_lines.saturating_sub(1) * self.line_spacing
            + self.margin.top() as usize
            + self.margin.bottom() as usize
    }

    pub(crate) fn draw<O: FnMut(isize, isize, Bgra)>(
        &self,
        text_alignment: &TextAlignment,
        mut callback: O,
    ) {
        for line in &self.lines {
            let (mut x, x_incrementor) = match text_alignment {
                TextAlignment::Center => (
                    line.available_space as isize / 2,
                    self.spacebar_width as isize,
                ),
                TextAlignment::Left => (0, self.spacebar_width as isize),
                TextAlignment::Right => {
                    (line.available_space as isize, self.spacebar_width as isize)
                }
                TextAlignment::SpaceBetween => (
                    0,
                    if line.words.len() == 1 {
                        0
                    } else {
                        line.blank_space() as isize / line.words.len().saturating_sub(1) as isize
                    },
                ),
            };
            x += self.margin.left() as isize;

            for word in &line.words {
                word.glyphs.iter().for_each(|local_glyph| {
                    let (x_shift, _y_shift) = local_glyph.draw(
                        x,
                        line.y_offset as isize,
                        self.max_bearing_y as isize,
                        &self.fg_color,
                        &mut callback,
                    );
                    x += x_shift;
                });

                x += x_incrementor;
            }
        }
    }
}

struct LineRect {
    y_offset: usize,

    available_space: usize,
    spacebar_width: usize,

    words: Vec<WordRect>,
}

impl LineRect {
    fn new(
        y_offset: usize,
        available_space: usize,
        spacebar_width: usize,
        words: Vec<WordRect>,
    ) -> Self {
        Self {
            y_offset,
            available_space,
            spacebar_width,
            words,
        }
    }

    fn blank_space(&self) -> usize {
        self.available_space + self.spacebar_width * self.words.len().saturating_sub(1)
    }

    fn ellipsize(
        &mut self,
        last_word: Option<WordRect>,
        ellipsis: LocalGlyph,
        ellipsize_at: &EllipsizeAt,
    ) -> EllipsiationState {
        let Some(word) = last_word else {
            return EllipsiationState::Complete;
        };

        match ellipsize_at {
            EllipsizeAt::Middle => {
                self.ellipsize_middle(word, ellipsis);
                return EllipsiationState::Complete;
            }
            EllipsizeAt::End => self.ellipsize_end(ellipsis),
        }
    }

    fn ellipsize_middle(&mut self, mut last_word: WordRect, ellipsis: LocalGlyph) {
        let ellipsis_width = ellipsis.width();
        while !last_word.is_blank()
            && last_word.advance_width + self.spacebar_width + ellipsis_width > self.available_space
        {
            last_word.pop_glyph();
        }

        if !last_word.is_blank() {
            last_word.push_glyph(ellipsis);
            self.push_word(last_word);
            return;
        } else if ellipsis_width <= self.available_space {
            self.push_ellipsis_to_last_word(ellipsis);
            return;
        }

        let mut last_word = self.words.pop().expect(
            "Here must have a WordRect struct in the LineRect. \
            But it doesn't have, so the LineRect is not correct. Please to contact the devs of \
            the Noti application with this information, please.",
        );

        while !last_word.is_blank()
            && last_word.advance_width + self.spacebar_width + ellipsis_width > self.available_space
        {
            last_word.pop_glyph();
        }

        // INFO: here MUST be enough space for cutting word and ellipsization
        // so here doesn't check if the last word is blank
        last_word.push_glyph(ellipsis);
        self.push_word(last_word);
    }

    fn ellipsize_end(&mut self, ellipsis: LocalGlyph) -> EllipsiationState {
        if ellipsis.width() <= self.available_space {
            self.push_ellipsis_to_last_word(ellipsis);
            EllipsiationState::Complete
        } else {
            let Some(last_word) = self.words.pop() else {
                return EllipsiationState::Continue(Some(WordRect::new_empty()));
            };

            self.available_space += last_word.advance_width + self.spacebar_width;
            self.ellipsize_end(ellipsis)
        }
    }

    fn push_word(&mut self, word: WordRect) {
        self.available_space -= word.advance_width
            + if !self.words.is_empty() {
                self.spacebar_width
            } else {
                0
            };
        self.words.push(word);
    }

    fn push_ellipsis_to_last_word(&mut self, ellipsis: LocalGlyph) {
        if let Some(last_word) = self.words.last_mut() {
            self.available_space -= ellipsis.width();
            last_word.push_glyph(ellipsis);
        }
    }
}

#[derive(Default)]
enum EllipsiationState {
    Continue(Option<WordRect>),
    #[default]
    Complete,
}

#[derive(Default, Clone)]
enum LocalGlyph {
    Image(Image),
    Outline((Metrics, Vec<u8>)),
    #[default]
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

    fn width(&self) -> usize {
        match self {
            LocalGlyph::Image(img) => img.width().unwrap_or_default(),
            LocalGlyph::Outline((metrics, _)) => metrics.advance_width.round() as usize,
            LocalGlyph::Empty => 0,
        }
    }

    fn bottom_spacing(&self) -> usize {
        match self {
            LocalGlyph::Image(_) => 0,
            LocalGlyph::Outline((metrics, _)) => metrics.ymin.abs() as usize,
            LocalGlyph::Empty => 0,
        }
    }

    fn bearing_y(&self) -> usize {
        match self {
            LocalGlyph::Image(img) => img.height().unwrap_or_default(),
            LocalGlyph::Outline((metrics, _)) => (metrics.height as i32 + metrics.ymin) as usize,
            LocalGlyph::Empty => todo!(),
        }
    }

    fn draw<O: FnMut(isize, isize, Bgra)>(
        &self,
        x_offset: isize,
        y_offset: isize,
        max_bearing_y: isize,
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
                let y_diff = -metrics.ymin as isize + max_bearing_y - height;
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
    glyphs: Vec<LocalGlyph>,
}

impl WordRect {
    fn new_empty() -> Self {
        WordRect {
            advance_width: 0,
            glyphs: vec![],
        }
    }
    fn from_local_glyphs(outlined_glyphs: Vec<LocalGlyph>) -> Self {
        let advance_width = outlined_glyphs
            .iter()
            .map(|local_glyph| local_glyph.width())
            .sum();

        Self {
            advance_width,
            glyphs: outlined_glyphs,
        }
    }

    fn max_bearing_y(&self) -> usize {
        self.glyphs
            .iter()
            .map(|glyph| glyph.bearing_y())
            .max()
            .unwrap_or_default()
    }

    fn max_bottom_spacing(&self) -> usize {
        self.glyphs
            .iter()
            .map(|glyph| glyph.bottom_spacing())
            .max()
            .unwrap_or_default()
    }

    #[inline(always = true)]
    fn push_glyph(&mut self, new_glyph: LocalGlyph) {
        self.advance_width += new_glyph.width();
        self.glyphs.push(new_glyph);
    }

    #[inline(always = true)]
    fn pop_glyph(&mut self) -> Option<LocalGlyph> {
        let last_glyph = self.glyphs.pop();
        if let Some(last_glyph) = last_glyph.as_ref() {
            self.advance_width = self.advance_width.saturating_sub(last_glyph.width());
        }

        last_glyph
    }

    #[inline(always = true)]
    fn is_blank(&self) -> bool {
        self.glyphs.is_empty()
    }
}
