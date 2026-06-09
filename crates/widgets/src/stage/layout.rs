use crate::{context::LoadExtent, types::WidgetId};

pub(crate) trait LayoutContext<T>: LoadExtent<T, WidgetId>
where
    T: Default + Copy,
{
}

impl<C, T> LayoutContext<T> for C
where
    C: LoadExtent<T, WidgetId>,
    T: Default + Copy,
{
}

pub(crate) trait Layout<C, T>
where
    C: LayoutContext<T>,
    T: Default + Copy,
{
    fn layout(&mut self, context: &C);
}
