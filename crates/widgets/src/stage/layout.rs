use crate::{
    context::{LoadExtent, ManageWidgetData},
    types::WidgetId,
};

pub trait LayoutContext<T>: LoadExtent<T, WidgetId> + ManageWidgetData<WidgetId>
where
    T: Default + Copy,
{
}

impl<C, T> LayoutContext<T> for C
where
    C: LoadExtent<T, WidgetId> + ManageWidgetData<WidgetId>,
    T: Default + Copy,
{
}

pub trait Layout<C, T>
where
    C: LayoutContext<T>,
    T: Default + Copy,
{
    fn layout(&mut self, context: &mut C);
}
