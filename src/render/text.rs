use std::{collections::VecDeque, sync::Arc};

use fontdue::Metrics;

use crate::{
    data::text::{EntityKind, Text},
    render::font::FontStyle,
};

use super::{color::Bgra, font::FontCollection};

#[derive(Default)]
pub(crate) struct TextRect {
    words: Vec<TextObject>,
    line_spacing: usize,
    padding: usize,
}

impl TextRect {
    pub(crate) fn from_text(
        text: &Text,
        px_size: f32,
        font_collection: Arc<FontCollection>,
    ) -> Self {
        let Text { body, entities } = text;

        let mut entities = VecDeque::from_iter(entities.iter());
        let mut current_entities = VecDeque::new();
        let mut current_style = FontStyle::Regular;

        let glyph_collection: Vec<Option<(Metrics, Vec<u8>)>> = body
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
                    Some(font.rasterize_indexed(font.lookup_glyph_index(ch), px_size))
                } else {
                    None
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

        let words: Vec<TextObject> = glyph_collection
            .split(|maybe_glyph| maybe_glyph.is_none())
            .map(|word| TextObject::from_outlined_glyphs(word))
            .collect();

        Self {
            words,
            ..Default::default()
        }
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
        mut callback: O,
    ) {
        const SPACEBAR_WIDTH: isize = 15;
        let bottom = height - self.padding;

        let bottom_spacing = self
            .words
            .iter()
            .flat_map(|word| word.glyphs.iter())
            .map(|(metrics, _)| metrics.ymin as isize)
            .min()
            .unwrap_or_default()
            .abs() as usize;

        let line_height = self
            .words
            .iter()
            .flat_map(|word| word.glyphs.iter())
            .map(|(metrics, _)| metrics.height as isize + metrics.ymin as isize)
            .max()
            .unwrap_or_default();

        let total_height = bottom_spacing + line_height as usize;

        let mut word_index = 0;

        for y in (self.padding as isize..bottom as isize)
            .step_by(total_height + self.line_spacing)
            .take_while(|y| bottom - *y as usize > total_height)
        {
            let mut remaining_width = (width - self.padding * 2) as isize;
            let mut words = vec![];
            while let Some(word) = self.words.get(word_index) {
                if remaining_width < 0 || word.width > remaining_width as usize {
                    break;
                }

                words.push(word);
                word_index += 1;
                remaining_width -=
                    word.width as isize + if words.len() > 1 { SPACEBAR_WIDTH } else { 0 };
            }

            //TODO: in future here maybe justification algorithm

            let mut x = self.padding as isize;
            for word in words {
                word.glyphs.iter().for_each(|(metrics, coverage)| {
                    let mut coverage_iter = coverage.iter();
                    let (width, height) = (metrics.width as isize, metrics.height as isize);
                    let y_diff = -metrics.ymin as isize + line_height - height;
                    let x_diff = metrics.xmin as isize;

                    for glyph_y in y_diff..height + y_diff {
                        for glyph_x in x_diff..width + x_diff {
                            let mut bgra = Bgra::new();
                            bgra.alpha = *coverage_iter.next().unwrap() as f32 / 255.0;
                            callback(x + glyph_x, y as isize + glyph_y, bgra);
                        }
                    }

                    x += metrics.advance_width.round() as isize;
                });

                // TODO: replace here a code after calculating text justification
                x += SPACEBAR_WIDTH;
            }
        }
    }
}

pub(crate) struct TextObject {
    width: usize,
    height: usize,
    glyphs: Vec<(Metrics, Vec<u8>)>,
}

impl TextObject {
    fn from_outlined_glyphs(outlined_glyphs: &[Option<(Metrics, Vec<u8>)>]) -> Self {
        let outlined_glyphs: Vec<(Metrics, Vec<u8>)> = outlined_glyphs
            .into_iter()
            .map(|glyph| glyph.as_ref().cloned().unwrap())
            .collect();

        let width = outlined_glyphs
            .iter()
            .map(|glyph| glyph.0.advance_width)
            .sum::<f32>()
            .round() as usize;

        let height = outlined_glyphs[0].0.height;

        Self {
            width,
            height,
            glyphs: outlined_glyphs,
        }
    }
}
