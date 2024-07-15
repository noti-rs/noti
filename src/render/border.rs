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

impl Border {
    pub(crate) fn draw<O: FnMut(usize, usize, Bgra)>(&self, mut callback: O) {
        let coverage = match (self.size, self.radius) {
            (0, 0) => return,
            (width, 0) => self.get_bordered_coverage(width),
            (0, radius) => self.get_rounding_coverage(radius),
            (width, radius) => self.get_bordered_rounding_coverage(width, radius),
        };

        let coverage_size = coverage.len();

        for (frame_y, y) in (0..coverage_size).zip((0..coverage_size).rev()) {
            for (frame_x, x) in (0..coverage_size).zip((0..coverage_size).rev()) {
                if let Some(coverage) = coverage[x][y].as_ref() {
                    callback(frame_x, frame_y, coverage.clone());
                }
            }
        }

        for (frame_y, y) in (0..coverage_size).zip((0..coverage_size).rev()) {
            for (frame_x, x) in
                (self.frame_width - coverage_size..self.frame_width).zip(0..coverage_size)
            {
                if let Some(coverage) = coverage[x][y].as_ref() {
                    callback(frame_x, frame_y, coverage.clone());
                }
            }
        }

        for (frame_y, y) in
            (self.frame_height - coverage_size..self.frame_height).zip(0..coverage_size)
        {
            for (frame_x, x) in
                (self.frame_width - coverage_size..self.frame_width).zip(0..coverage_size)
            {
                if let Some(coverage) = coverage[x][y].as_ref() {
                    callback(frame_x, frame_y, coverage.clone());
                }
            }
        }

        for (frame_y, y) in
            (self.frame_height - coverage_size..self.frame_height).zip(0..coverage_size)
        {
            for (frame_x, x) in (0..coverage_size).zip((0..coverage_size).rev()) {
                if let Some(coverage) = coverage[x][y].as_ref() {
                    callback(frame_x, frame_y, coverage.clone());
                }
            }
        }

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

    fn get_bordered_coverage(&self, width: usize) -> Vec<Vec<Option<Bgra>>> {
        vec![vec![Some(self.color.clone()); width]; width]
    }

    fn get_rounding_coverage(&self, radius: usize) -> Vec<Vec<Option<Bgra>>> {
        let mut coverage = vec![vec![None; radius]; radius];
        for y in (0..radius).rev() {
            for x in (0..radius).rev() {
                let cell_coverage = Self::get_coverage_by(radius as f32, x as f32, y as f32);
                if cell_coverage == 1.0 {
                    break;
                }

                coverage[x][y] = Some(self.background_color.clone() * cell_coverage);
            }
        }

        coverage
    }

    fn get_bordered_rounding_coverage(
        &self,
        width: usize,
        radius: usize,
    ) -> Vec<Vec<Option<Bgra>>> {
        let mut coverage = vec![vec![None; radius]; radius];
        let inner_radius = radius.saturating_sub(width);

        for y in (0..radius).rev() {
            for x in (0..radius).rev() {
                let (x_f32, y_f32) = (x as f32, y as f32);
                let outer_color =
                    self.color.clone() * Self::get_coverage_by(radius as f32, x_f32, y_f32);

                let inner_color = if inner_radius != 0 {
                    let inner_cell_coverage =
                        Self::get_coverage_by(inner_radius as f32, x_f32, y_f32);
                    if inner_cell_coverage == 1.0 {
                        break;
                    }

                    self.background_color.clone() * inner_cell_coverage
                } else {
                    Bgra::new()
                };

                coverage[x][y] = Some(inner_color.overlay_on(&outer_color));
            }
        }

        coverage
    }

    fn get_coverage_by(radius: f32, x: f32, y: f32) -> f32 {
        let inner_hypot = f32::hypot(x, y);
        let inner_diff = radius - inner_hypot;
        let outer_hypot = f32::hypot(x + 1.0, y + 1.0);

        if inner_hypot >= radius {
            0.0
        } else if outer_hypot >= radius {
            inner_diff
        } else {
            1.0
        }
    }

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
