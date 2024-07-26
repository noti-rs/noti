use derive_builder::Builder;

use super::{
    banner::{Coverage, Draw, DrawColor},
    color::Bgra,
};

type Matrix<T> = Vec<Vec<T>>;

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
    #[inline]
    fn get_bordered_coverage(&self, width: usize) -> Matrix<Option<DrawColor>> {
        vec![vec![Some(DrawColor::Replace(self.color.clone())); width]; width]
    }

    fn get_corner_coverage(&self, radius: usize) -> Matrix<Option<DrawColor>> {
        let mut corner = vec![vec![None; radius]; radius];
        let radius = std::cmp::min(radius, self.frame_height / 2);

        self.traverse_circle_with(radius, |inner_x, inner_y, rev_x, rev_y| {
            let cell_coverage = Self::get_coverage_by(radius as f32, rev_x as f32, rev_y as f32);

            if cell_coverage == 1.0 {
                return false;
            }

            let color = self.background_color.clone() * cell_coverage;
            corner[inner_x][inner_y] = Some(DrawColor::Replace(color.clone()));
            corner[inner_y][inner_x] = Some(DrawColor::Replace(color));

            true
        });

        corner
    }

    fn get_bordered_corner_coverage(
        &self,
        size: usize,
        radius: usize,
    ) -> Matrix<Option<DrawColor>> {
        let radius = std::cmp::min(radius, self.frame_height / 2);
        let inner_radius = radius.saturating_sub(size);

        let mut corner = vec![vec![None; radius]; radius];

        self.traverse_circle_with(radius, |inner_x, inner_y, rev_x, rev_y| {
            let (x_f32, y_f32) = (rev_x as f32, rev_y as f32);

            let border_color =
                self.color.clone() * Self::get_coverage_by(radius as f32, x_f32, y_f32);

            let mut to_continue = true;

            let color = if inner_radius != 0 {
                let inner_cell_coverage = Self::get_coverage_by(inner_radius as f32, x_f32, y_f32);

                match inner_cell_coverage {
                    0.0 => DrawColor::Replace(border_color),
                    1.0 => {
                        to_continue = false;
                        DrawColor::OverlayWithCoverage(Bgra::new(), Coverage(0.0))
                        // self.background_color.clone()
                    }
                    cell_coverage => match (self.color.alpha, self.background_color.alpha) {
                        (_, 0.0) => DrawColor::Replace(border_color * (1.0 - cell_coverage)),
                        (0.0, _) => {
                            DrawColor::Replace(self.background_color.clone() * cell_coverage)
                        }
                        _ => DrawColor::OverlayWithCoverage(
                            border_color,
                            Coverage(1.0 - cell_coverage),
                        ),
                    },
                }
            } else {
                DrawColor::Replace(border_color)
            };

            corner[inner_x][inner_y] = Some(color.clone());
            corner[inner_y][inner_x] = Some(color);

            to_continue
        });

        corner
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
    fn draw_corner<Output: FnMut(usize, usize, DrawColor)>(
        x_offset: usize,
        y_offset: usize,
        corner: &Matrix<Option<DrawColor>>,
        corner_type: Corner,
        output: &mut Output,
    ) {
        let corner_size = corner.len();
        let mut x_range = x_offset..x_offset + corner_size;
        let y_range = y_offset..y_offset + corner_size;

        let x_range: &mut dyn Iterator<Item = usize> = match corner_type {
            Corner::TopLeft | Corner::BottomLeft => &mut x_range,
            Corner::TopRight | Corner::BottomRight => &mut x_range.rev(),
        };

        for (x, corner_row) in x_range.zip(corner) {
            let y_range: &mut dyn Iterator<Item = usize> = match corner_type {
                Corner::TopLeft | Corner::TopRight => &mut y_range.clone(),
                Corner::BottomLeft | Corner::BottomRight => &mut y_range.clone().rev(),
            };

            for (y, corner_cell) in y_range.zip(corner_row) {
                if let Some(color) = corner_cell {
                    output(x, y, color.clone());
                } else {
                    break;
                }
            }
        }
    }

    #[inline]
    fn draw_rectangle<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        x_offset: usize,
        y_offset: usize,
        width: usize,
        height: usize,
        output: &mut Output,
    ) {
        for x in x_offset..width + x_offset {
            for y in y_offset..height + y_offset {
                output(x, y, DrawColor::Replace(self.color.clone()))
            }
        }
    }
}

impl Draw for Border {
    fn draw<Output: FnMut(usize, usize, super::banner::DrawColor)>(&self, mut output: Output) {
        let corner = match (self.size, self.radius) {
            (0, 0) => return,
            (size, 0) => self.get_bordered_coverage(size),
            (0, radius) => self.get_corner_coverage(radius),
            (size, radius) => self.get_bordered_corner_coverage(size, radius),
        };

        let corner_size = corner.len();
        Self::draw_corner(0, 0, &corner, Corner::TopLeft, &mut output);
        Self::draw_corner(
            self.frame_width - corner_size,
            0,
            &corner,
            Corner::TopRight,
            &mut output,
        );
        Self::draw_corner(
            self.frame_width - corner_size,
            self.frame_height - corner_size,
            &corner,
            Corner::BottomRight,
            &mut output,
        );
        Self::draw_corner(
            0,
            self.frame_height - corner_size,
            &corner,
            Corner::BottomLeft,
            &mut output,
        );

        if self.size != 0 {
            // Top
            self.draw_rectangle(
                corner_size,
                0,
                self.frame_width - corner_size * 2,
                self.size,
                &mut output,
            );

            // Bottom
            self.draw_rectangle(
                corner_size,
                self.frame_height - self.size,
                self.frame_width - corner_size * 2,
                self.size,
                &mut output,
            );

            // Left
            self.draw_rectangle(
                0,
                corner_size,
                self.size,
                self.frame_height - corner_size * 2,
                &mut output,
            );

            // Right
            self.draw_rectangle(
                self.frame_width - self.size,
                corner_size,
                self.size,
                self.frame_height - corner_size * 2,
                &mut output,
            );
        }
    }
}
