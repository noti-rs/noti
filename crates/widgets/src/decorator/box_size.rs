use std::ops::{Div, Mul};

use crate::{
    decorator::{DecoratorType, MeasureDecorator},
    measure::{Constraints, Intrinsic},
    types::Extent,
};

pub(crate) struct BoxSizeDecorator<N, T> {
    pub(super) width: Option<T>,
    pub(super) height: Option<T>,
    pub(super) ratio: Option<f32>,
    pub(super) next: N,
}

impl<N, T> MeasureDecorator<T> for BoxSizeDecorator<N, T>
where
    N: MeasureDecorator<T>,
    T: DecoratorType + Div<f32, Output = T> + Mul<f32, Output = T>,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        let mut intrinsic = self.next.intrinsic();

        if let Some(width) = self.width {
            intrinsic.min.width = width;
            intrinsic.max.width = width;

            if self.height.is_none() {
                if let Some(ratio) = self.ratio {
                    intrinsic.min.height = width / ratio;
                    intrinsic.max.height = width / ratio;
                }
            }
        }

        if let Some(height) = self.height {
            intrinsic.min.height = height;
            intrinsic.max.height = height;

            if self.width.is_none() {
                if let Some(ratio) = self.ratio {
                    intrinsic.min.width = height * ratio;
                    intrinsic.max.width = height * ratio;
                }
            }
        }

        intrinsic
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        let mut new_constraints = constraints;

        if let Some(width) = self.width {
            new_constraints.min.width = width;
            new_constraints.max.width = width;

            if self.height.is_none() {
                if let Some(ratio) = self.ratio {
                    new_constraints.min.height = width / ratio;
                    new_constraints.max.height = width / ratio;
                }
            }
        }

        if let Some(height) = self.height {
            new_constraints.min.height = height;
            new_constraints.max.height = height;

            if self.width.is_none() {
                if let Some(ratio) = self.ratio {
                    new_constraints.min.width = height * ratio;
                    new_constraints.max.width = height * ratio;
                }
            }
        }

        if self.width.is_none() && self.height.is_none() {
            if let Some(ratio) = self.ratio {
                let mut proportional_width = new_constraints.max.width;
                let mut proportional_height = proportional_width / ratio;

                if proportional_height > new_constraints.max.height {
                    proportional_height = new_constraints.max.height;
                    proportional_width = proportional_height * ratio;
                }

                let fixed_extent = Extent::new(proportional_width, proportional_height);
                new_constraints.min = fixed_extent;
                new_constraints.max = fixed_extent;
            }
        }

        self.next
            .measure(new_constraints)
            .clamp_with(constraints.min, constraints.max)
    }
}
