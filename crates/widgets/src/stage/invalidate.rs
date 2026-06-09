use crate::{
    context::{
        AnimationQuery, GetState, GetStyle, ManageAnimationRegistry, ManageDirtyFlags,
        ScopedManageState,
    },
    stage::measure::SizingMode,
    types::{dirty_flags::DirtyFlags, WidgetId, WidgetStyle},
    widget::{WidgetInformation, WidgetSizingMode},
};

pub(crate) trait InvalidateContext:
    ManageDirtyFlags<WidgetId>
    + ManageAnimationRegistry<WidgetId>
    + AnimationQuery<WidgetId>
    + GetState
    + GetStyle
    + ScopedManageState
{
}

impl<C> InvalidateContext for C where
    C: ManageDirtyFlags<WidgetId>
        + ManageAnimationRegistry<WidgetId>
        + AnimationQuery<WidgetId>
        + GetState
        + GetStyle
        + ScopedManageState
{
}

pub(crate) enum RebuildStatus {
    NeedsMeasure,
    NothingChanged,
}

pub(crate) trait InvalidateVisitor<C>
where
    C: InvalidateContext,
{
    fn invalidate<W: Invalidate<C>>(&mut self, child: &mut W);
}

pub(crate) trait Invalidate<C>: WidgetInformation + WidgetSizingMode
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, context: &mut C, style: WidgetStyle);
    fn on_rebuild(&mut self, context: &mut C) -> RebuildStatus;
    fn invalidate_children(&mut self, visitor: &mut impl InvalidateVisitor<C>);

    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        let mut dirty_flags = context.get_dirty_flags(self.get_id());

        if dirty_flags.contains(DirtyFlags::NEEDS_UPDATE_STYLES) {
            if let Some(style) = context.get_style(&self.get_class()) {
                self.on_style_update(context, style.clone());

                dirty_flags |= DirtyFlags::NEEDS_MEASURE;
            }

            dirty_flags -= DirtyFlags::NEEDS_UPDATE_STYLES;
        }

        if dirty_flags.contains(DirtyFlags::NEEDS_REBUILD) {
            match self.on_rebuild(context) {
                RebuildStatus::NeedsMeasure => dirty_flags |= DirtyFlags::NEEDS_MEASURE,
                RebuildStatus::NothingChanged => (),
            }

            dirty_flags -= DirtyFlags::NEEDS_REBUILD;
        }

        struct ChildrenVisitor<'a, C> {
            context: &'a mut C,
            parent_df: &'a mut DirtyFlags,
            sizing_mode: SizingMode,
        }

        impl<'a, C> InvalidateVisitor<C> for ChildrenVisitor<'a, C>
        where
            C: InvalidateContext,
        {
            fn invalidate<W: Invalidate<C>>(&mut self, child: &mut W) {
                let child_flags = child.invalidate(self.context);

                match self.sizing_mode {
                    SizingMode::Fixed => {
                        if child_flags
                            .intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE)
                        {
                            *self.parent_df |= DirtyFlags::CHILD_NEEDS_MEASURE
                        }
                    }
                    SizingMode::Dynamic => {
                        if child_flags.contains(DirtyFlags::NEEDS_MEASURE) {
                            *self.parent_df |=
                                DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE;
                        } else if child_flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE) {
                            *self.parent_df |= DirtyFlags::CHILD_NEEDS_MEASURE;
                        }
                    }
                }
            }
        }

        let mut visitor = ChildrenVisitor {
            context,
            parent_df: &mut dirty_flags,
            sizing_mode: self.sizing_mode(),
        };
        self.invalidate_children(&mut visitor);

        if dirty_flags.is_empty() {
            context.remove_dirty_flags(self.get_id());
        } else {
            context.set_dirty_flags(self.get_id(), dirty_flags);
        }

        dirty_flags
    }
}
