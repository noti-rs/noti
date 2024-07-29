use std::{collections::VecDeque, sync::Arc};

use derive_builder::Builder;
use itertools::Itertools;

use crate::{
    config::{spacing::Spacing, EllipsizeAt, TextAlignment},
    data::text::Text,
};

use super::{
    banner::{Draw, DrawColor},
    color::Bgra,
    font::{FontCollection, FontStyle, Glyph},
    types::Offset,
};

#[derive(Default)]
pub(crate) struct TextRect {
    words: VecDeque<WordRect>,
    lines: Vec<LineRect>,

    width: usize,

    spacebar_width: usize,
    line_height: usize,
    line_spacing: usize,
    ascent: usize,

    ellipsize_at: EllipsizeAt,
    ellipsis: Glyph,
    alignment: TextAlignment,

    fg_color: Bgra,

    margin: Spacing,
}

impl TextRect {
    pub(crate) fn from_str(
        string: &str,
        px_size: f32,
        font_collection: Arc<FontCollection>,
    ) -> Self {
        let glyph_collection: Vec<Glyph> = string
            .chars()
            .map(|ch| font_collection.load_glyph_by_style(&FontStyle::Regular, ch, px_size))
            .collect();

        let words = Self::convert_to_words(glyph_collection);
        Self {
            words,
            spacebar_width: Self::get_spacebar_width(&font_collection, px_size),
            ellipsis: font_collection.get_ellipsis(px_size),
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

        let glyph_collection: Vec<Glyph> = body
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

                let glyph = font_collection.load_glyph_by_style(&current_style, ch, px_size);

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
            ellipsis: font_collection.get_ellipsis(px_size),
            ..Default::default()
        }
    }

    fn convert_to_words(glyph_collection: Vec<Glyph>) -> VecDeque<WordRect> {
        glyph_collection
            .into_iter()
            .chunk_by(|glyph| !glyph.is_empty())
            .into_iter()
            .filter_map(|(matches, word)| matches.then_some(WordRect::from_glyphs(word.collect())))
            .collect()
    }

    fn get_spacebar_width(font_collection: &Arc<FontCollection>, px_size: f32) -> usize {
        font_collection
            .font_by_style(&FontStyle::Regular)
            .font()
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

    pub(crate) fn set_alignment(&mut self, alignment: &TextAlignment) {
        self.alignment = alignment.to_owned();
    }

    pub(crate) fn compile(&mut self, mut width: usize, mut height: usize) {
        self.width = width;

        self.margin.shrink(&mut width, &mut height);

        let (descent, ascent) = self
            .words
            .iter()
            .map(|word| (word.max_descent(), word.max_ascent()))
            .reduce(|lhs, rhs| (std::cmp::max(lhs.0, rhs.0), std::cmp::max(lhs.1, rhs.1)))
            .unwrap_or_default();

        self.ascent = ascent;
        self.line_height = descent + ascent;

        let mut lines = vec![];

        for y in (0..height)
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

            lines.push(LineRectBuilder::create_empty()
                .y_offset(y)
                .available_space(remaining_width)
                .spacebar_width(self.spacebar_width)
                .max_ascent(self.ascent)
                .alignment(self.alignment.to_owned())
                .foreground(self.fg_color.to_owned())
                .words(words)
                .build()
                .expect("Can't create a Line rect from existing components. Please contact with developers with this information."));
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
                .map(|last_line| {
                    last_line.ellipsize(word, self.ellipsis.clone(), &self.ellipsize_at)
                })
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
}

impl Draw for TextRect {
    fn draw<Output: FnMut(usize, usize, DrawColor)>(&self, mut output: Output) {
        let offset: Offset = (&self.margin).into();

        self.lines
            .iter()
            .for_each(|line| line.draw(|x, y, color| output(x + offset.x, y + offset.y, color)))
    }
}

#[derive(Builder, Default)]
#[builder(pattern = "owned")]
struct LineRect {
    y_offset: usize,

    max_ascent: usize,
    available_space: usize,
    spacebar_width: usize,

    foreground: Bgra,
    alignment: TextAlignment,

    words: Vec<WordRect>,
}

impl LineRect {
    fn blank_space(&self) -> usize {
        self.available_space + self.spacebar_width * self.words.len().saturating_sub(1)
    }

    fn ellipsize(
        &mut self,
        last_word: Option<WordRect>,
        ellipsis: Glyph,
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

    fn ellipsize_middle(&mut self, mut last_word: WordRect, ellipsis: Glyph) {
        let ellipsis_width = ellipsis.advance_width();
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

        let mut last_word = self.pop_word();

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

    fn ellipsize_end(&mut self, ellipsis: Glyph) -> EllipsiationState {
        if ellipsis.advance_width() <= self.available_space {
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

    fn pop_word(&mut self) -> WordRect {
        let last_word = self.words.pop().expect(
            "Here must have a WordRect struct in the LineRect. \
            But it doesn't have, so the LineRect is not correct. Please to contact the devs of \
            the Noti application with this information, please.",
        );

        self.available_space += last_word.advance_width
            + if !self.words.is_empty() {
                self.spacebar_width
            } else {
                0
            };

        last_word
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

    fn push_ellipsis_to_last_word(&mut self, ellipsis: Glyph) {
        if let Some(last_word) = self.words.last_mut() {
            self.available_space -= ellipsis.advance_width();
            last_word.push_glyph(ellipsis);
        }
    }
}

impl Draw for LineRect {
    fn draw<Output: FnMut(usize, usize, DrawColor)>(&self, mut output: Output) {
        let (x, x_incrementor) = match &self.alignment {
            TextAlignment::Center => (self.available_space / 2, self.spacebar_width),
            TextAlignment::Left => (0, self.spacebar_width),
            TextAlignment::Right => (self.available_space, self.spacebar_width),
            TextAlignment::SpaceBetween => (
                0,
                if self.words.len() == 1 {
                    0
                } else {
                    self.blank_space() / self.words.len().saturating_sub(1)
                },
            ),
        };

        let mut offset = Offset::new(x, self.y_offset);

        for word in &self.words {
            word.glyphs.iter().for_each(|local_glyph| {
                local_glyph.draw(&offset, self.max_ascent, &self.foreground, &mut output);
                offset.x += local_glyph.advance_width();
            });

            offset.x += x_incrementor;
        }
    }
}

#[derive(Default)]
enum EllipsiationState {
    Continue(Option<WordRect>),
    #[default]
    Complete,
}

pub(crate) struct WordRect {
    advance_width: usize,
    glyphs: Vec<Glyph>,
}

impl WordRect {
    fn new_empty() -> Self {
        WordRect {
            advance_width: 0,
            glyphs: vec![],
        }
    }
    fn from_glyphs(outlined_glyphs: Vec<Glyph>) -> Self {
        let advance_width = outlined_glyphs.iter().map(|glyph| glyph.advance_width()).sum();

        Self {
            advance_width,
            glyphs: outlined_glyphs,
        }
    }

    fn max_ascent(&self) -> usize {
        self.glyphs
            .iter()
            .map(|glyph| glyph.ascent())
            .max()
            .unwrap_or_default()
    }

    fn max_descent(&self) -> usize {
        self.glyphs
            .iter()
            .map(|glyph| glyph.descent().abs() as usize)
            .max()
            .unwrap_or_default()
    }

    #[inline(always = true)]
    fn push_glyph(&mut self, new_glyph: Glyph) {
        self.advance_width += new_glyph.advance_width();
        self.glyphs.push(new_glyph);
    }

    #[inline(always = true)]
    fn pop_glyph(&mut self) -> Option<Glyph> {
        let last_glyph = self.glyphs.pop();
        if let Some(last_glyph) = last_glyph.as_ref() {
            self.advance_width = self.advance_width.saturating_sub(last_glyph.advance_width());
        }

        last_glyph
    }

    #[inline(always = true)]
    fn is_blank(&self) -> bool {
        self.glyphs.is_empty()
    }
}
