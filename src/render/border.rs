use derive_builder::Builder;

use super::color::Bgra;

#[derive(Default, Builder)]
pub(crate) struct Border {
    color: Bgra,
    background_color: Bgra,

    frame_width: usize,
    frame_height: usize,

    #[builder(setter(into))]
    size: usize,
    #[builder(setter(into))]
    radius: usize,
}

enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Border {
    pub(crate) fn draw<O: FnMut(usize, usize, Bgra)>(&self, mut callback: O) {
        let coverage = match (self.size, self.radius) {
            (0, 0) => return,
            (width, 0) => self.get_bordered_coverage(width),
            (0, radius) => self.get_corner_coverage(radius),
            (width, radius) => self.get_bordered_corner_coverage(width, radius),
        };

        let coverage_size = coverage.len();
        Self::draw_corner(0, 0, &coverage, Corner::TopLeft, &mut callback);
        Self::draw_corner(
            self.frame_width - coverage_size,
            0,
            &coverage,
            Corner::TopRight,
            &mut callback,
        );
        Self::draw_corner(
            self.frame_width - coverage_size,
            self.frame_height - coverage_size,
            &coverage,
            Corner::BottomRight,
            &mut callback,
        );
        Self::draw_corner(
            0,
            self.frame_height - coverage_size,
            &coverage,
            Corner::BottomLeft,
            &mut callback,
        );

        if self.size != 0 {
            // Top
            self.draw_rectangle(
                coverage_size,
                0,
                self.frame_width - coverage_size * 2,
                self.size,
                &mut callback,
            );

            // Bottom
            self.draw_rectangle(
                coverage_size,
                self.frame_height - self.size,
                self.frame_width - coverage_size * 2,
                self.size,
                &mut callback,
            );

            // Left
            self.draw_rectangle(
                0,
                coverage_size,
                self.size,
                self.frame_height - coverage_size * 2,
                &mut callback,
            );

            // Right
            self.draw_rectangle(
                self.frame_width - self.size,
                coverage_size,
                self.size,
                self.frame_height - coverage_size * 2,
                &mut callback,
            );
        }
    }

    #[inline]
    fn get_bordered_coverage(&self, width: usize) -> Vec<Vec<Option<Bgra>>> {
        vec![vec![Some(self.color.clone()); width]; width]
    }

    fn get_corner_coverage(&self, radius: usize) -> Vec<Vec<Option<Bgra>>> {
        let mut coverage = vec![vec![None; radius]; radius];
        self.traverse_circle_with(radius, |inner_x, inner_y, rev_x, rev_y| {
            let cell_coverage = Self::get_coverage_by(radius as f32, rev_x as f32, rev_y as f32);

            if cell_coverage == 1.0 {
                return false;
            }

            let color = self.background_color.clone() * cell_coverage;
            coverage[inner_x][inner_y] = Some(color.clone());
            coverage[inner_y][inner_x] = Some(color);

            true
        });

        coverage
    }

    fn get_bordered_corner_coverage(&self, width: usize, radius: usize) -> Vec<Vec<Option<Bgra>>> {
        let mut coverage = vec![vec![None; radius]; radius];
        let inner_radius = radius.saturating_sub(width);

        self.traverse_circle_with(radius, |inner_x, inner_y, rev_x, rev_y| {
            let (x_f32, y_f32) = (rev_x as f32, rev_y as f32);
            let outer_color =
                self.color.clone() * Self::get_coverage_by(radius as f32, x_f32, y_f32);

            let inner_color = if inner_radius != 0 {
                let inner_cell_coverage = Self::get_coverage_by(inner_radius as f32, x_f32, y_f32);
                if inner_cell_coverage == 1.0 {
                    return false;
                }

                self.background_color.clone() * inner_cell_coverage
            } else {
                Bgra::new()
            };

            let color = inner_color.overlay_on(&outer_color);
            coverage[inner_x][inner_y] = Some(color.clone());
            coverage[inner_y][inner_x] = Some(color);

            true
        });

        coverage
    }

    fn traverse_circle_with<Calc: FnMut(usize, usize, usize, usize) -> bool>(
        &self,
        radius: usize,
        mut calc: Calc,
    ) {
        for x in 0..radius {
            let rev_x = radius - x - 1;

            for y in 0..radius {
                let rev_y = radius - y - 1;
                let to_continue = calc(x, y, rev_x, rev_y);

                if !to_continue {
                    break;
                }
            }
        }
    }

    #[inline]
    fn get_coverage_by(radius: f32, x: f32, y: f32) -> f32 {
        let inner_hypot = f32::hypot(x, y);
        let inner_diff = radius - inner_hypot;
        let outer_hypot = f32::hypot(x + 1.0, y + 1.0);

        if inner_hypot >= radius {
            0.0
        } else if outer_hypot >= radius {
            inner_diff.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    #[inline]
    fn draw_corner<O: FnMut(usize, usize, Bgra)>(
        x_offset: usize,
        y_offset: usize,
        coverage: &Vec<Vec<Option<Bgra>>>,
        corner_type: Corner,
        callback: &mut O,
    ) {
        let coverage_size = coverage.len();
        let mut x_range = x_offset..x_offset + coverage_size;
        let y_range = y_offset..y_offset + coverage_size;

        let x_range: &mut dyn Iterator<Item = usize> = match corner_type {
            Corner::TopLeft | Corner::BottomLeft => &mut x_range,
            Corner::TopRight | Corner::BottomRight => &mut x_range.rev(),
        };

        for (x, coverage_row) in x_range.zip(coverage) {
            let y_range: &mut dyn Iterator<Item = usize> = match corner_type {
                Corner::TopLeft | Corner::TopRight => &mut y_range.clone(),
                Corner::BottomLeft | Corner::BottomRight => &mut y_range.clone().rev(),
            };

            for (y, coverage_cell) in y_range.zip(coverage_row) {
                if let Some(color) = coverage_cell {
                    callback(x, y, color.clone());
                } else {
                    break;
                }
            }
        }
    }

    #[inline]
    fn draw_rectangle<O: FnMut(usize, usize, Bgra)>(
        &self,
        x_offset: usize,
        y_offset: usize,
        width: usize,
        height: usize,
        callback: &mut O,
    ) {
        for x in x_offset..width + x_offset {
            for y in y_offset..height + y_offset {
                callback(x, y, self.color.clone())
            }
        }
    }
}
