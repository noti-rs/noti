use crate::{
    color::Bgra,
    types::{Offset, RectSize},
    widget::{Coverage, DrawColor},
};

pub struct Drawer {
    size: RectSize,
    data: Vec<Bgra>,
}

impl Drawer {
    pub fn new(color: Bgra, size: RectSize) -> Self {
        Self {
            data: vec![color; size.area()],
            size,
        }
    }

    pub fn draw_area(&mut self, offset: &Offset, mut subdrawer: Drawer) {
        if let Some(untransparent_pos) = subdrawer
            .data
            .iter()
            .position(|color| !color.is_transparent())
        {
            let start_y = untransparent_pos / subdrawer.size.width;
            let start_x = untransparent_pos - subdrawer.size.width * start_y;

            let end_x = subdrawer.size.width - start_x;
            let end_y = subdrawer.size.height - start_y;

            for y in start_y..end_y {
                let is_corner =
                    start_x.saturating_sub(y) > 0 || start_x.saturating_sub(end_y - y) > 0;
                if is_corner {
                    let start_range = subdrawer.abs_pos_at(0, y)..subdrawer.abs_pos_at(start_x, y);
                    let end_range = subdrawer.abs_pos_at(end_x, y)
                        ..subdrawer.abs_pos_at(subdrawer.size.width, y);
                    subdrawer.data[start_range]
                        .iter()
                        .zip(0..start_x)
                        .chain(
                            subdrawer.data[end_range]
                                .iter()
                                .zip(end_x..subdrawer.size.width),
                        )
                        .for_each(|(color, x)| {
                            if color.is_transparent() {
                                self.draw_color(
                                    offset.x + x,
                                    offset.y + y,
                                    DrawColor::Overlay(color.to_owned()),
                                )
                            } else {
                                self.draw_color(
                                    offset.x + x,
                                    offset.y + y,
                                    DrawColor::Replace(color.to_owned()),
                                );
                            }
                        });

                    let line_in_parent = self.abs_pos_at(offset.x + start_x, offset.y + y)
                        ..self.abs_pos_at(offset.x + end_x, offset.y + y);
                    let line_in_child =
                        subdrawer.abs_pos_at(start_x, y)..subdrawer.abs_pos_at(end_x, y);
                    self.data[line_in_parent].swap_with_slice(&mut subdrawer.data[line_in_child]);
                } else {
                    let line_in_parent = self.abs_pos_at(offset.x, offset.y + y)
                        ..self.abs_pos_at(offset.x + subdrawer.size.width, offset.y + y);
                    let line_in_child =
                        subdrawer.abs_pos_at(0, y)..subdrawer.abs_pos_at(subdrawer.size.width, y);

                    self.data[line_in_parent].swap_with_slice(&mut subdrawer.data[line_in_child]);
                }
            }
        }
    }

    pub fn draw_color(&mut self, x: usize, y: usize, color: DrawColor) {
        self.put_color_at(x, y, Self::convert_color(color, self.get_color_at(x, y)));
    }

    fn convert_color(color: DrawColor, background: &Bgra) -> Bgra {
        match color {
            DrawColor::Replace(color) => color,
            DrawColor::Overlay(foreground) => foreground.overlay_on(background),
            DrawColor::OverlayWithCoverage(foreground, Coverage(factor)) => {
                foreground.linearly_interpolate(background, factor)
            }
            DrawColor::Transparent(Coverage(factor)) => *background * factor,
        }
    }

    fn get_color_at(&self, x: usize, y: usize) -> &Bgra {
        &self.data[self.abs_pos_at(x, y)]
    }

    fn put_color_at(&mut self, x: usize, y: usize, color: Bgra) {
        let pos = self.abs_pos_at(x, y);
        self.data[pos] = color;
    }

    #[inline(always = true)]
    fn abs_pos_at(&self, x: usize, y: usize) -> usize {
        self.size.width * y + x
    }
}

impl From<Drawer> for Vec<u8> {
    fn from(drawer: Drawer) -> Self {
        drawer
            .data
            .into_iter()
            .flat_map(|color| color.into_slice())
            .collect()
    }
}
