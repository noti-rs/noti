use crate::{
    decorator::{DecoratorType, MeasureDecorator},
    measure::{Constraints, Intrinsic},
    types::{Extent, Spacing},
};

pub(crate) struct SpacingDecorator<N> {
    pub(super) spacing: Spacing,
    pub(super) next: N,
}

impl<N, T> MeasureDecorator<T> for SpacingDecorator<N>
where
    N: MeasureDecorator<T>,
    T: DecoratorType + std::fmt::Debug,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        self.next.intrinsic() + self.spacing
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        let new_constraints = constraints.shrink_to_with(&self.spacing);

        let used_extent = self.next.measure(new_constraints) + self.spacing.into();

        used_extent.clamp_with(constraints.min, constraints.max)
    }
}
