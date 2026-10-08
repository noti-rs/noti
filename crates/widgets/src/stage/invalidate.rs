use crate::{
    context::{
        AnimationQuery, GetState, ManageAnimationRegistry, ManageDirtyFlags, ScopedManageState,
        StateLifetimeManagement,
    },
    types::{
        dirty_flags::{propagate_needs_measure, DirtyFlags},
        WidgetId,
    },
    widget::{WidgetInformation, WidgetInformationContext, WidgetSizingMode},
};

pub trait InvalidateContext:
    WidgetInformationContext
    + ManageDirtyFlags<WidgetId>
    + ManageAnimationRegistry<WidgetId>
    + AnimationQuery<WidgetId>
    + GetState
    + ScopedManageState
{
}

impl<C> InvalidateContext for C where
    C: WidgetInformationContext
        + ManageDirtyFlags<WidgetId>
        + ManageAnimationRegistry<WidgetId>
        + AnimationQuery<WidgetId>
        + GetState
        + ScopedManageState
{
}

pub enum RebuildStatus {
    NeedsMeasure,
    NothingChanged,
}

pub trait Invalidate<C>: WidgetInformation + WidgetSizingMode<C>
where
    C: InvalidateContext,
{
    /// When a widget reuses during a tree rebuild process, this method is called.
    ///
    /// The implementation must check the difference from current and old information that cannot be
    /// compared by [`crate::stage::rebuild::WidgetDiff`], also requiring additional actions.
    ///
    /// For instance, states cannot be easily checked by the trait above, because on change need to
    /// decrement reference counter of the old state and increment reference counter of the new state.
    fn on_reuse(&mut self, old: &dyn std::any::Any, context: &mut C) -> DirtyFlags
    where
        C: StateLifetimeManagement;

    fn on_rebuild(&mut self, context: &mut C) -> RebuildStatus;

    fn invalidate(&mut self, context: &mut C) {
        let mut dirty_flags = context.get_dirty_flags(self.get_id());

        if dirty_flags.contains(DirtyFlags::NEEDS_REBUILD) {
            match self.on_rebuild(context) {
                RebuildStatus::NeedsMeasure => dirty_flags |= DirtyFlags::NEEDS_MEASURE,
                RebuildStatus::NothingChanged => (),
            }

            dirty_flags -= DirtyFlags::NEEDS_REBUILD;
        }

        if dirty_flags.is_empty() {
            context.clear_dirty_flags(self.get_id());
        } else {
            context.set_dirty_flags(self.get_id(), dirty_flags);

            if dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) {
                propagate_needs_measure(self.get_id(), context);
            }
        }
    }
}
