use crate::{
    decorator::{DecoratorType, MeasureDecorator},
    types::{Border, Spacing},
};

pub(crate) struct BorderDecorator<N> {
    pub(super) border: Border,
    pub(super) next: N,
}

impl<N, T> MeasureDecorator<T> for BorderDecorator<N>
where
    N: MeasureDecorator<T>,
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> crate::measure::Intrinsic<T> {
        self.next.intrinsic() + Spacing::all_directional(self.border.size)
    }

    fn measure(
        &mut self,
        constraints: crate::measure::Constraints<crate::types::Extent<T>>,
    ) -> crate::types::Extent<T> {
        let spacing = Spacing::all_directional(self.border.size);
        let new_constraints = constraints.shrink_to_with(&spacing);

        let used_extent = self.next.measure(new_constraints) + spacing.into();

        used_extent.clamp_with(constraints.min, constraints.max)
    }
}
