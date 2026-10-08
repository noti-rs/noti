use crate::{
    context::{
        GenerateId, GetFont, GetState, ManageAnimationRegistry, ManageDirtyFlags, RegisterKey,
        StateLifetimeManagement, StateSubscription,
    },
    types::{dirty_flags::DirtyFlags, identifiers::WidgetKey, WidgetId},
    widget::{WidgetBase, WidgetInformationContext},
};

pub trait InitContext:
    GenerateId
    + RegisterKey<WidgetKey, WidgetId>
    + GetState
    + StateSubscription<WidgetId>
    + StateLifetimeManagement
    + WidgetInformationContext
    + ManageDirtyFlags<WidgetId>
    + ManageAnimationRegistry<WidgetId>
    + GetFont
{
}

impl<C> InitContext for C where
    C: GenerateId
        + RegisterKey<WidgetKey, WidgetId>
        + GetState
        + StateSubscription<WidgetId>
        + StateLifetimeManagement
        + WidgetInformationContext
        + ManageDirtyFlags<WidgetId>
        + ManageAnimationRegistry<WidgetId>
        + GetFont
{
}

pub trait Init<C>: WidgetBase<C>
where
    C: InitContext,
{
    fn init(&mut self, context: &mut C) {
        if *self.get_id() == 0 {
            self.set_id(context.generate_id());
        }

        if let Some(key) = self.get_key() {
            context.register_key(key.clone(), self.get_id());
        }

        self.on_init(context);

        context.append_dirty_flags(self.get_id(), DirtyFlags::NEEDS_MEASURE);
    }

    fn on_init(&mut self, context: &mut C);
}
