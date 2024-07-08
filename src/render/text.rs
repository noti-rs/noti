use std::{collections::VecDeque, sync::Arc};

use ab_glyph::{Font, OutlinedGlyph};

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
        pt_size: u32,
        font_collection: Arc<FontCollection>,
    ) -> Self {
        let Text { body, entities } = text;

        let mut entities = VecDeque::from_iter(entities.iter());
        let mut current_entities = VecDeque::new();
        let mut current_style = FontStyle::Regular;

        let glyph_collection: Vec<Option<OutlinedGlyph>> = body
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
                    let glyph = font.glyph_id(ch);
                    let px_scale = font.pt_to_px_scale(pt_size as f32).unwrap();
                    font.outline_glyph(glyph.with_scale_and_position(
                        px_scale,
                        (px_scale.x, px_scale.y),
                    ))
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

    pub(crate) fn draw<O: FnMut(usize, usize, Bgra)>(
        &self,
        width: usize,
        height: usize,
        mut callback: O,
    ) {
        const SPACEBAR_WIDTH: isize = 15;
        let bottom = height - self.padding;
        let word_height = self.words[0].height;

        let mut word_index = 0;

        for y in (self.padding..bottom)
            .step_by(word_height + self.line_spacing)
            .take_while(|y| bottom - y > word_height)
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

            let mut x = self.padding;
            for word in words {
                word.glyphs.iter().for_each(|glyph| {
                    let glyph_bounds = glyph.px_bounds();
                    let (min_x, min_y) = (
                        glyph_bounds.min.x.round() as usize,
                        glyph_bounds.min.y.round() as usize,
                    );
                    glyph.draw(|glyph_x, glyph_y, alpha| {
                        let mut bgra = Bgra::new();
                        bgra.alpha = alpha;

                        callback(
                            x + glyph_x as usize + min_x,
                            y + glyph_y as usize + min_y,
                            bgra,
                        );
                    });
                    x += glyph_bounds.width().round() as usize;
                });

                // TODO: replace here a code after calculating text justification
                x += SPACEBAR_WIDTH as usize;
            }
        }
    }
}

pub(crate) struct TextObject {
    width: usize,
    height: usize,
    glyphs: Vec<OutlinedGlyph>,
}

impl TextObject {
    fn from_outlined_glyphs(outlined_glyphs: &[Option<OutlinedGlyph>]) -> Self {
        let outlined_glyphs: Vec<OutlinedGlyph> = outlined_glyphs
            .into_iter()
            .map(|glyph| glyph.as_ref().cloned().unwrap())
            .collect();

        let width = outlined_glyphs
            .iter()
            .map(|glyph| glyph.px_bounds().width())
            .sum::<f32>()
            .round() as usize;

        let height = outlined_glyphs[0].px_bounds().height().round() as usize;

        Self {
            width,
            height,
            glyphs: outlined_glyphs,
        }
    }
}
