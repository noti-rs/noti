use std::collections::VecDeque;

use derive_builder::Builder;
use itertools::Itertools;

use config::{
    spacing::Spacing,
    text::{EllipsizeAt, TextJustification},
};
use dbus::text::Text;

use crate::drawer::Drawer;

use super::{
    color::Bgra,
    font::{FontCollection, FontStyle, Glyph},
    types::{Offset, RectSize},
    widget::Draw,
};

#[derive(Default)]
pub struct TextRect {
    paragraphs: VecDeque<VecDeque<WordRect>>,
    current_paragraph: VecDeque<WordRect>,

    lines: Vec<LineRect>,
    wrap: bool,

    rect_size: RectSize,

    spacebar_width: usize,
    line_height: usize,
    line_spacing: usize,

    ellipsize_at: EllipsizeAt,
    ellipsis: Glyph,
    justification: TextJustification,

    foreground: Bgra,

    margin: Spacing,
}

impl TextRect {
    pub fn from_str<Style: Into<FontStyle> + Clone>(
        string: &str,
        px_size: f32,
        base_style: Style,
        font_collection: &FontCollection,
    ) -> Self {
        let font_style = base_style.into();
        let paragraphs: VecDeque<VecDeque<WordRect>> = string
            .chars()
            .chunk_by(|char| *char != '\n')
            .into_iter()
            .filter_map(|(matches, chunk)| {
                matches.then(|| {
                    chunk
                        .into_iter()
                        .map(|ch| font_collection.load_glyph_by_style(&font_style, ch, px_size))
                        .collect()
                })
            })
            .map(Self::convert_to_words)
            .collect();

        Self {
            paragraphs,
            wrap: true,
            spacebar_width: Self::get_spacebar_width(font_collection, px_size),
            ellipsis: font_collection.get_ellipsis(px_size),
            line_height: font_collection.max_height(px_size),
            ..Default::default()
        }
    }

    pub fn from_text<Style: Into<FontStyle>>(
        text: &Text,
        px_size: f32,
        base_style: Style,
        font_collection: &FontCollection,
    ) -> Self {
        let Text { body, entities } = text;
        let base_style: FontStyle = base_style.into();

        let mut entities = VecDeque::from_iter(entities.iter());
        let mut current_entities = VecDeque::new();
        let mut current_style = FontStyle::Regular;

        let mut paragraphs = VecDeque::new();
        let mut current_paragraph = vec![];

        for (position, ch) in body.chars().enumerate() {
            while let Some(entity) = entities.front() {
                if entity.offset == position {
                    current_style += FontStyle::from(&entity.kind);
                    current_entities.push_back(entities.pop_front().unwrap());
                } else {
                    break;
                }
            }

            if ch == '\n' {
                paragraphs.push_back(Self::convert_to_words(current_paragraph));
                current_paragraph = vec![];
            } else {
                current_paragraph.push(font_collection.load_glyph_by_style(
                    &(&base_style + &current_style),
                    ch,
                    px_size,
                ))
            }

            while let Some(entity) = current_entities.front() {
                if entity.offset + entity.length <= position {
                    let entity = current_entities.pop_front().unwrap();
                    current_style -= FontStyle::from(&entity.kind);
                } else {
                    break;
                }
            }
        }

        if !current_paragraph.is_empty() {
            paragraphs.push_back(Self::convert_to_words(current_paragraph));
        }

        Self {
            paragraphs,
            wrap: true,
            spacebar_width: Self::get_spacebar_width(font_collection, px_size),
            ellipsis: font_collection.get_ellipsis(px_size),
            line_height: font_collection.max_height(px_size),
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

    fn get_spacebar_width(font_collection: &FontCollection, px_size: f32) -> usize {
        font_collection.get_spacebar_width(px_size).round() as usize
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }

    pub fn set_line_spacing(&mut self, line_spacing: usize) {
        self.line_spacing = line_spacing;
    }

    pub fn set_margin(&mut self, margin: &Spacing) {
        self.margin = margin.clone();
    }

    pub fn set_foreground(&mut self, color: Bgra) {
        self.foreground = color;
    }

    pub fn set_ellipsize_at(&mut self, ellipsize_at: &EllipsizeAt) {
        self.ellipsize_at = ellipsize_at.clone();
    }

    pub fn set_justification(&mut self, justification: &TextJustification) {
        self.justification = justification.to_owned();
    }

    pub fn compile(&mut self, mut rect_size: RectSize) {
        self.rect_size.width = rect_size.width;
        rect_size.shrink_by(&self.margin);

        let mut paragraph_num = 0;
        self.current_paragraph = self.paragraphs.pop_front().unwrap_or_default();

        let mut lines = vec![];

        for y in (0..rect_size.height)
            .step_by(self.line_height + self.line_spacing)
            .take_while(|y| rect_size.height - *y >= self.line_height)
            .take(if self.wrap { usize::MAX } else { 1 })
        {
            let mut line = LineRectBuilder::create_empty()
                .paragraph_num(paragraph_num)
                .y_offset(y)
                .available_space(rect_size.width as isize)
                .spacebar_width(self.spacebar_width)
                .justification(self.justification.to_owned())
                .words(vec![])
                .build()
                .expect("Can't create a Line rect from existing components. Please contact with developers with this information.");

            while let Some(word) = self.current_paragraph.pop_front() {
                line.push_word(word);

                if line.is_overflow() {
                    // INFO: here is a logic when the line have single word which overflows current
                    // line and use it for ellipsization. Otherwise (when it is not single word)
                    // remove last word to clean up the overflow and return it to `self.words`.
                    if line.len() > 1 {
                        self.current_paragraph.push_front(line.pop_word().unwrap())
                    }

                    break;
                }
            }

            let is_overflow = line.is_overflow();
            lines.push(line);

            if is_overflow {
                break;
            }

            if self.current_paragraph.is_empty() {
                self.current_paragraph = self.paragraphs.pop_front().unwrap_or_default();
                paragraph_num += 1;

                if self.current_paragraph.is_empty() {
                    break;
                }
            }
        }

        self.lines = lines;
        self.ellipsize(paragraph_num);
        self.apply_color();
    }

    fn ellipsize(&mut self, paragraph_num: u8) {
        let mut state = self
            .lines
            .last_mut()
            .map(|last_line| {
                last_line.ellipsize(
                    paragraph_num,
                    self.current_paragraph.pop_front(),
                    self.ellipsis.clone(),
                    &self.ellipsize_at,
                )
            })
            .unwrap_or_default();

        while let EllipsizationState::Continue {
            paragraph_num,
            last_word,
        } = state
        {
            self.lines.pop();

            state = self
                .lines
                .last_mut()
                .map(|last_line| {
                    last_line.ellipsize(
                        paragraph_num,
                        last_word,
                        self.ellipsis.clone(),
                        &self.ellipsize_at,
                    )
                })
                .unwrap_or_default();
        }
    }

    fn apply_color(&mut self) {
        self.lines
            .iter_mut()
            .for_each(|line| line.set_color(self.foreground));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty() || self.lines.iter().all(|line| line.is_empty())
    }

    #[allow(unused)]
    pub fn width(&self) -> usize {
        self.rect_size.width
    }

    pub fn height(&self) -> usize {
        let total_lines = self.lines.len();
        total_lines * self.line_height
            + total_lines.saturating_sub(1) * self.line_spacing
            + self.margin.top() as usize
            + self.margin.bottom() as usize
    }
}

impl Draw for TextRect {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        let offset = Offset::from(&self.margin) + *offset;

        self.lines
            .iter()
            .for_each(|line| line.draw_with_offset(&offset, drawer))
    }
}

#[derive(Builder, Default)]
#[builder(pattern = "owned")]
struct LineRect {
    paragraph_num: u8,
    y_offset: usize,

    available_space: isize,
    spacebar_width: usize,

    justification: TextJustification,

    words: Vec<WordRect>,
}

impl LineRect {
    fn available_space(&self) -> usize {
        assert!(
            self.available_space >= 0,
            "The available space of line rect is negative. Maybe you forgot ellipsize it."
        );

        self.available_space as usize
    }

    fn blank_space(&self) -> usize {
        assert!(
            self.available_space >= 0,
            "The available space of line rect is negative. Maybe you forgot ellipsize it."
        );
        self.available_space as usize + self.spacebar_width * self.words.len().saturating_sub(1)
    }

    fn ellipsize(
        &mut self,
        paragraph_num: u8,
        mut last_word: Option<WordRect>,
        ellipsis: Glyph,
        ellipsize_at: &EllipsizeAt,
    ) -> EllipsizationState {
        match ellipsize_at {
            EllipsizeAt::Middle => {
                self.ellipsize_middle(last_word, ellipsis);
                EllipsizationState::Complete
            }
            EllipsizeAt::End => {
                if last_word.is_some() || self.available_space < 0 {
                    if paragraph_num != self.paragraph_num {
                        last_word.replace(WordRect::new_empty());
                    }
                    self.ellipsize_end(ellipsis)
                } else {
                    EllipsizationState::Complete
                }
            }
        }
    }

    fn ellipsize_middle(&mut self, last_word: Option<WordRect>, ellipsis: Glyph) {
        let ellipsis_width = ellipsis.advance_width();

        if let Some(mut last_word) = last_word {
            while !last_word.is_blank()
                && (last_word.width() + self.spacebar_width + ellipsis_width) as isize
                    > self.available_space
            {
                last_word.pop_glyph();
            }

            if !last_word.is_blank() {
                last_word.push_glyph(ellipsis);
                self.push_word(last_word);
                return;
            } else if ellipsis_width as isize <= self.available_space {
                self.push_ellipsis_to_last_word(ellipsis);
                return;
            }
        } else if self.available_space >= 0 {
            return;
        }

        let mut last_word = self.pop_word().expect(
            "Here must have a WordRect struct in the LineRect. \
            But it doesn't have, so the LineRect is not correct. Please to contact the devs of \
            the Noti application with this information, please.",
        );

        while !last_word.is_blank()
            && (last_word.width() + self.spacebar_width + ellipsis_width) as isize
                > self.available_space
        {
            last_word.pop_glyph();
        }

        if last_word.is_blank() {
            return;
        }

        // INFO: here MUST be enough space for cutting word and ellipsization
        // so here doesn't check if the last word is blank
        last_word.push_glyph(ellipsis);
        self.push_word(last_word);
    }

    fn ellipsize_end(&mut self, ellipsis: Glyph) -> EllipsizationState {
        if self.words.is_empty() {
            return EllipsizationState::Continue {
                paragraph_num: self.paragraph_num,
                last_word: Some(WordRect::new_empty()),
            };
        }

        if ellipsis.advance_width() as isize <= self.available_space {
            self.push_ellipsis_to_last_word(ellipsis);
            EllipsizationState::Complete
        } else {
            self.pop_word();
            self.ellipsize_end(ellipsis)
        }
    }

    fn pop_word(&mut self) -> Option<WordRect> {
        let last_word = self.words.pop()?;

        self.available_space += (last_word.width()
            + if !self.words.is_empty() {
                self.spacebar_width
            } else {
                0
            }) as isize;

        Some(last_word)
    }

    fn push_word(&mut self, word: WordRect) {
        self.available_space -= (word.width()
            + if !self.words.is_empty() {
                self.spacebar_width
            } else {
                0
            }) as isize;
        self.words.push(word);
    }

    fn push_ellipsis_to_last_word(&mut self, ellipsis: Glyph) {
        if let Some(last_word) = self.words.last_mut() {
            self.available_space -= ellipsis.advance_width() as isize;
            last_word.push_glyph(ellipsis);
        }
    }

    fn len(&self) -> usize {
        self.words.len()
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    fn is_overflow(&self) -> bool {
        self.available_space < 0
    }

    fn set_color(&mut self, color: Bgra) {
        self.words.iter_mut().for_each(|word| word.set_color(color));
    }
}

impl Draw for LineRect {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        let (x, x_incrementor) = match &self.justification {
            TextJustification::Center => (self.available_space() / 2, self.spacebar_width),
            TextJustification::Left => (0, self.spacebar_width),
            TextJustification::Right => (self.available_space(), self.spacebar_width),
            TextJustification::SpaceBetween => (
                0,
                if self.words.len() == 1 {
                    0
                } else {
                    self.blank_space() / self.words.len().saturating_sub(1)
                },
            ),
        };

        let mut offset = *offset + Offset::new(x, self.y_offset);

        self.words.iter().for_each(|word| {
            word.draw_with_offset(&offset, drawer);
            offset.x += x_incrementor + word.width();
        });
    }
}

#[derive(Default)]
enum EllipsizationState {
    Continue {
        paragraph_num: u8,
        last_word: Option<WordRect>,
    },
    #[default]
    Complete,
}

pub struct WordRect {
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
        let advance_width = outlined_glyphs
            .iter()
            .map(|glyph| glyph.advance_width())
            .sum();

        Self {
            advance_width,
            glyphs: outlined_glyphs,
        }
    }

    #[inline(always = true)]
    fn set_color(&mut self, color: Bgra) {
        self.glyphs
            .iter_mut()
            .for_each(|glyph| glyph.set_color(color));
    }

    #[inline(always = true)]
    fn width(&self) -> usize {
        self.advance_width
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
            self.advance_width = self
                .advance_width
                .saturating_sub(last_glyph.advance_width());
        }

        last_glyph
    }

    #[inline(always = true)]
    fn is_blank(&self) -> bool {
        self.glyphs.is_empty()
    }
}

impl Draw for WordRect {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        let mut offset = offset.to_owned();
        self.glyphs.iter().for_each(|glyph| {
            glyph.draw_with_offset(&offset, drawer);
            offset.x += glyph.advance_width();
        })
    }
}
