use derive_builder::Builder;

use super::color::Bgra;

#[derive(Default, Builder)]
pub(crate) struct Border {
    color: Bgra,
    background_color: Bgra,

    frame_width: usize,
    frame_height: usize,

    #[builder(setter(into))]
    width: Option<usize>,
    #[builder(setter(into))]
    radius: Option<usize>,
}

impl Border {
    pub(crate) fn draw<O: FnMut(usize, usize, Bgra)>(&self, mut callback: O) {
        let coverage = match (self.width, self.radius) {
            (None, None) => return,
            (Some(width), None) => self.get_bordered_coverage(width),
            (None, Some(radius)) => self.get_rounding_coverage(radius),
            (Some(width), Some(radius)) => self.get_bordered_rounding_coverage(width, radius),
        };

        let coverage_size = coverage.len();

        for (frame_y, y) in (0..coverage_size).zip((0..coverage_size).rev()) {
            for (frame_x, x) in (0..coverage_size).zip((0..coverage_size).rev()) {
                callback(frame_x, frame_y, coverage[x][y].clone());
            }
        }

        for (frame_y, y) in (0..coverage_size).zip((0..coverage_size).rev()) {
            for (frame_x, x) in
                (self.frame_width - coverage_size..self.frame_width).zip(0..coverage_size)
            {
                callback(frame_x, frame_y, coverage[x][y].clone());
            }
        }

        for (frame_y, y) in
            (self.frame_height - coverage_size..self.frame_height).zip(0..coverage_size)
        {
            for (frame_x, x) in
                (self.frame_width - coverage_size..self.frame_width).zip(0..coverage_size)
            {
                callback(frame_x, frame_y, coverage[x][y].clone());
            }
        }

        for (frame_y, y) in
            (self.frame_height - coverage_size..self.frame_height).zip(0..coverage_size)
        {
            for (frame_x, x) in (0..coverage_size).zip((0..coverage_size).rev()) {
                callback(frame_x, frame_y, coverage[x][y].clone());
            }
        }

        if let Some(width) = self.width {
            for y in 0..width {
                for x in coverage_size..self.frame_width - coverage_size {
                    callback(x, y, self.color.clone())
                }
            }

            for y in self.frame_height - width..self.frame_height {
                for x in coverage_size..self.frame_width - coverage_size {
                    callback(x, y, self.color.clone())
                }
            }

            for x in 0..width {
                for y in coverage_size..self.frame_height - coverage_size {
                    callback(x, y, self.color.clone())
                }
            }

            for x in self.frame_width - width..self.frame_width {
                for y in coverage_size..self.frame_height - coverage_size {
                    callback(x, y, self.color.clone())
                }
            }
        }
    }

    fn get_bordered_coverage(&self, width: usize) -> Vec<Vec<Bgra>> {
        vec![vec![self.color.clone(); width]; width]
    }

    fn get_rounding_coverage(&self, radius: usize) -> Vec<Vec<Bgra>> {
        let mut coverage = vec![vec![Bgra::new(); radius]; radius];
        for y in 0..radius {
            for x in 0..radius {
                coverage[x][y] = self.background_color.clone()
                    * Self::get_coverage_by(radius as f32, x as f32, y as f32);
            }
        }

        coverage
    }

    fn get_bordered_rounding_coverage(&self, width: usize, radius: usize) -> Vec<Vec<Bgra>> {
        let mut coverage = vec![vec![Bgra::new(); radius]; radius];
        let inner_radius = radius.saturating_sub(width);

        for y in 0..radius {
            for x in 0..radius {
                let (x_f32, y_f32) = (x as f32, y as f32);
                let outer_color =
                    self.color.clone() * Self::get_coverage_by(radius as f32, x_f32, y_f32);

                let inner_color = if inner_radius != 0 {
                    self.background_color.clone()
                        * Self::get_coverage_by(inner_radius as f32, x_f32, y_f32)
                } else {
                    Bgra::new()
                };

                coverage[x][y] = inner_color.overlay_on(&outer_color);
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
}
