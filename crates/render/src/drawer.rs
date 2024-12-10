use crate::{
    color::Bgra,
    types::RectSize,
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

    pub fn as_mut_output(&mut self) -> impl FnMut(usize, usize, DrawColor) + use<'_> {
        |x, y, color| self.draw_color(x, y, color)
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
            DrawColor::Transparent(Coverage(factor)) => background.clone() * factor,
        }
    }

    fn get_color_at(&self, x: usize, y: usize) -> &Bgra {
        &self.data[self.size.width * y + x]
    }

    fn put_color_at(&mut self, x: usize, y: usize, color: Bgra) {
        self.data[self.size.width * y + x] = color;
    }

    pub fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        self,
        offset: &crate::types::Offset,
        output: &mut Output,
    ) {
        let mut x = 0;
        let mut y = 0;
        for cell in self.data {
            if cell.is_transparent() {
                output(offset.x + x, offset.y + y, DrawColor::Overlay(cell));
            } else {
                output(offset.x + x, offset.y + y, DrawColor::Replace(cell));
            }

            x += 1;
            if x == self.size.width {
                x = 0;
                y += 1;
            }
        }
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
