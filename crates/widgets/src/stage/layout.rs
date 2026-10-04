use log::warn;

use crate::{
    context::{LoadExtent, ManageDirtyFlags, ManageLocalCoord},
    types::{DirtyFlags, Extent, Point, WidgetId},
    widget::{WidgetGetType, WidgetInformation, WidgetInformationContext},
};

pub trait LayoutContext<T>:
    ManageLocalCoord<T, WidgetId>
    + LoadExtent<T, WidgetId>
    + WidgetInformationContext
    + ManageDirtyFlags<WidgetId>
where
    T: Default + Copy,
{
    fn layout_widget(&mut self, widget_id: &WidgetId, local_coord: Point<T>);
}

pub trait Layout<C, T>: WidgetInformation + WidgetGetType
where
    C: LayoutContext<T>,
    T: Default + Copy,
{
    fn on_layout(&mut self, context: &mut C, local_coord: Point<T>, provided_extent: Extent<T>);

    fn layout(&mut self, context: &mut C, local_coord: Point<T>) {
        context.save(self.get_id(), local_coord);

        let Some(provided_extent) = <C as LoadExtent<T, WidgetId>>::load(context, self.get_id())
        else {
            warn!(
                "{} widget with id {} didn't measured! Refused to layout.",
                self.get_type(),
                *self.get_id()
            );
            return;
        };

        self.on_layout(context, local_coord, provided_extent);

        context.remove_dirty_flags(self.get_id(), DirtyFlags::NEEDS_LAYOUT);
    }
}
