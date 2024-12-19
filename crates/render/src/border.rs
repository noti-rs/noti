use crate::color::{Bgra, Color};
use derive_builder::Builder;
use log::warn;

use crate::drawer::Drawer;

use super::{
    types::Offset,
    widget::{Coverage, Draw, DrawColor},
};

type Matrix<T> = Vec<Vec<T>>;
type MaybeColor = Option<DrawingMethod>;

#[derive(Default, Builder, Clone)]
pub struct Border {
    color: Color,
    frame_width: usize,
    frame_height: usize,

    #[builder(setter(into))]
    size: usize,
    #[builder(setter(into))]
    radius: usize,

    #[builder(setter(skip))]
    corner_coverage: Option<Matrix<MaybeColor>>,

    #[builder(setter(skip), default = "false")]
    compiled: bool,
}

impl BorderBuilder {
    pub fn compile(&self) -> anyhow::Result<Border> {
        let mut border = self.build()?;
        border.compile();
        Ok(border)
    }
}

enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone)]
enum DrawingMethod {
    Replace(Coverage),
    Transparent(Coverage),
    DependingOnBorderAlpha(Coverage, Coverage),
}

impl Border {
    pub fn compile(&mut self) {
        self.compiled = true;

        self.corner_coverage = Some(match (self.size, self.radius) {
            (0, 0) => return,
            (size, 0) => self.get_bordered_coverage(size),
            (0, radius) => self.get_corner_coverage(radius),
            (size, radius) => self.get_bordered_corner_coverage(size, radius),
        });
    }

    pub fn get_color_at(&self, x: usize, y: usize) -> Option<DrawColor> {
        assert!(x <= self.frame_width && y <= self.frame_height);

        let corner = self.corner_coverage.as_ref()?;
        let corner_size = corner.len();

        if (corner_size..self.frame_width - corner_size).contains(&x)
            && (corner_size..self.frame_height - corner_size).contains(&y)
        {
            return None;
        }

        if (corner_size..self.frame_width - corner_size).contains(&x) {
            return if y < self.size || y > self.frame_height - self.size {
                Some(DrawColor::Replace(self.get_color(x, y)))
            } else {
                None
            };
        }

        if (corner_size..self.frame_height - corner_size).contains(&y) {
            return if x < self.size || x > self.frame_width - self.size {
                Some(DrawColor::Replace(self.get_color(x, y)))
            } else {
                None
            };
        }

        let x_pos = if x < corner_size {
            x
        } else {
            self.frame_width - x - 1
        };

        let y_pos = if y < corner_size {
            y
        } else {
            self.frame_height - y - 1
        };

        corner[x_pos][y_pos]
            .as_ref()
            .map(|drawing_method| self.map_color(drawing_method, x, y))
    }

    #[inline]
    fn get_bordered_coverage(&self, width: usize) -> Matrix<MaybeColor> {
        vec![vec![Some(DrawingMethod::Replace(Coverage(1.0))); width]; width]
    }

    fn get_corner_coverage(&self, radius: usize) -> Matrix<MaybeColor> {
        let radius = std::cmp::min(radius, self.frame_height / 2);
        let mut corner = vec![vec![None; radius]; radius];

        self.traverse_circle_with(radius, |inner_x, inner_y, rev_x, rev_y| {
            let cell_coverage = Self::get_coverage_by(radius as f32, rev_x as f32, rev_y as f32);

            if cell_coverage == 1.0 {
                return false;
            }

            corner[inner_x][inner_y] = Some(DrawingMethod::Transparent(Coverage(cell_coverage)));
            corner[inner_y][inner_x] = Some(DrawingMethod::Transparent(Coverage(cell_coverage)));

            true
        });

        corner
    }

    fn get_bordered_corner_coverage(&self, size: usize, radius: usize) -> Matrix<MaybeColor> {
        let radius = std::cmp::min(radius, self.frame_height / 2);
        let inner_radius = radius.saturating_sub(size);

        let mut corner = vec![vec![None; radius]; radius];

        self.traverse_circle_with(radius, |inner_x, inner_y, rev_x, rev_y| {
            let (x_f32, y_f32) = (rev_x as f32, rev_y as f32);

            let border_cell_coverage = Self::get_coverage_by(radius as f32, x_f32, y_f32);

            let mut to_continue = true;

            let color = if inner_radius != 0 {
                let inner_cell_coverage = Self::get_coverage_by(inner_radius as f32, x_f32, y_f32);

                match inner_cell_coverage {
                    0.0 => DrawingMethod::Replace(Coverage(border_cell_coverage)),
                    1.0 => {
                        to_continue = false;
                        DrawingMethod::Transparent(Coverage(1.0))
                    }
                    cell_coverage => DrawingMethod::DependingOnBorderAlpha(
                        Coverage(border_cell_coverage),
                        Coverage(cell_coverage),
                    ),
                }
            } else {
                DrawingMethod::Replace(Coverage(border_cell_coverage))
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
    fn draw_corner(&self, offset: Offset, corner_type: Corner, drawer: &mut Drawer) {
        let Some(corner) = &self.corner_coverage else {
            return;
        };

        let corner_size = corner.len();
        let mut x_range = offset.x..offset.x + corner_size;
        let y_range = offset.y..offset.y + corner_size;

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
                    drawer.draw_color(x, y, self.map_color(color, x, y));
                } else {
                    break;
                }
            }
        }
    }

    #[inline]
    fn draw_rectangle(&self, offset: Offset, width: usize, height: usize, drawer: &mut Drawer) {
        for x in offset.x..width + offset.x {
            for y in offset.y..height + offset.y {
                drawer.draw_color(x, y, DrawColor::Replace(self.get_color(x, y)))
            }
        }
    }

    fn get_color(&self, x: usize, y: usize) -> Bgra {
        match &self.color {
            Color::Fill(bgra) => *bgra,
            Color::LinearGradient(gradient) => gradient.color_at(
                x as f32 / self.frame_width as f32,
                y as f32 / self.frame_height as f32,
            ),
        }
    }

    fn map_color(&self, method: &DrawingMethod, x: usize, y: usize) -> DrawColor {
        match method {
            DrawingMethod::Replace(coverage) => {
                DrawColor::Replace(self.get_color(x, y) * coverage.0)
            }
            DrawingMethod::Transparent(coverage) => DrawColor::Transparent(*coverage),
            DrawingMethod::DependingOnBorderAlpha(
                Coverage(border_cell_coverage),
                Coverage(inner_cell_coverage),
            ) => {
                let border_color = self.get_color(x, y);
                match border_color.alpha {
                    0.0 => DrawColor::Transparent(Coverage(*inner_cell_coverage)),
                    _ => DrawColor::OverlayWithCoverage(
                        border_color * *border_cell_coverage,
                        Coverage(1.0 - inner_cell_coverage),
                    ),
                }
            }
        }
    }
}

impl Draw for Border {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        let Some(corner_size) = self.corner_coverage.as_ref().map(|corner| corner.len()) else {
            if !self.compiled {
                warn!("Border: Not compiled, refused to draw itself");
            }
            return;
        };

        self.draw_corner(*offset, Corner::TopLeft, drawer);
        self.draw_corner(
            *offset + Offset::new_x(self.frame_width - corner_size),
            Corner::TopRight,
            drawer,
        );
        self.draw_corner(
            *offset
                + Offset::new(
                    self.frame_width - corner_size,
                    self.frame_height - corner_size,
                ),
            Corner::BottomRight,
            drawer,
        );
        self.draw_corner(
            *offset + Offset::new_y(self.frame_height - corner_size),
            Corner::BottomLeft,
            drawer,
        );

        if self.size != 0 {
            // Top
            self.draw_rectangle(
                *offset + Offset::new_x(corner_size),
                self.frame_width - corner_size * 2,
                self.size,
                drawer,
            );

            // Bottom
            self.draw_rectangle(
                *offset + Offset::new(corner_size, self.frame_height - self.size),
                self.frame_width - corner_size * 2,
                self.size,
                drawer,
            );

            // Left
            self.draw_rectangle(
                *offset + Offset::new_y(corner_size),
                self.size,
                self.frame_height - corner_size * 2,
                drawer,
            );

            // Right
            self.draw_rectangle(
                *offset + Offset::new(self.frame_width - self.size, corner_size),
                self.size,
                self.frame_height - corner_size * 2,
                drawer,
            );
        }
    }
}
