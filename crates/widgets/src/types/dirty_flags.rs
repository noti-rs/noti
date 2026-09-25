use crate::{
    context::ManageDirtyFlags, forest::Get, types::WidgetId, widget::WidgetInformationContext,
};

bitflags::bitflags! {
    /// DirtyFlags represent the pending updates for a widget.
    /// They prevent redundant calculations by marking exactly what needs
    /// synchronization or re-measurement in the current frame.
    #[derive(Debug, Clone, Copy)]
    pub struct DirtyFlags: u8 {
        const NEEDS_REBUILD       = 1;

        const NEEDS_UPDATE_STYLES = 1 << 1;

        /// The widget's own geometric constraints or intrinsic sizes are invalid.
        /// Triggers a re-calculation of the cached size in the Arena.
        const NEEDS_MEASURE       = 1 << 2;

        /// One or more descendants are marked with NEEDS_MEASURE.
        const CHILD_NEEDS_MEASURE = 1 << 3;

        const NEEDS_LAYOUT        = 1 << 4;
    }
}

pub(crate) fn propagate_needs_measure<C>(node_id: WidgetId, context: &mut C)
where
    C: WidgetInformationContext + ManageDirtyFlags<WidgetId>,
{
    context.set_dirty_flags(
        node_id,
        context.get_dirty_flags(node_id) | DirtyFlags::NEEDS_MEASURE,
    );

    let mut current_id = node_id;
    while let Some(node) = context.parent_of(current_id) {
        current_id = node.get();

        let mut parent_dirty_flags = context.get_dirty_flags(current_id);
        // INFO: If an ancestor already has NEEDS_MEASURE, propagating this invalidation farther
        // upward is unnecessary.
        if parent_dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) {
            break;
        }

        match context
            .widget_sizing_mode(&current_id)
            .expect("The widget must exist in a tree!")
        {
            crate::stage::measure::SizingMode::Fixed => {
                parent_dirty_flags |= DirtyFlags::CHILD_NEEDS_MEASURE;
                context.set_dirty_flags(current_id, parent_dirty_flags);
                break;
            }
            crate::stage::measure::SizingMode::Dynamic => {
                parent_dirty_flags |= DirtyFlags::NEEDS_MEASURE;
                context.set_dirty_flags(current_id, parent_dirty_flags);
            }
        }
    }
}
