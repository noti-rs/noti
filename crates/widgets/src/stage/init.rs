use crate::{
    context::{
        GenerateId, GetFont, GetState, GetStyle, ManageAnimationRegistry, RegisterKey,
        StateSubscription, StyleSubscription,
    },
    types::{identifiers::WidgetKey, WidgetClass, WidgetId},
    widget::WidgetBase,
};

pub(crate) trait InitContext:
    GenerateId
    + RegisterKey<WidgetKey, WidgetId>
    + GetState
    + StateSubscription<WidgetId>
    + StyleSubscription<WidgetClass, WidgetId>
    + ManageAnimationRegistry<WidgetId>
    + GetStyle
    + GetFont
{
}

impl<C> InitContext for C where
    C: GenerateId
        + RegisterKey<WidgetKey, WidgetId>
        + GetState
        + StateSubscription<WidgetId>
        + StyleSubscription<WidgetClass, WidgetId>
        + ManageAnimationRegistry<WidgetId>
        + GetStyle
        + GetFont
{
}

pub(crate) trait Init<C>: WidgetBase
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

        if !self.get_class().is_empty() {
            <C as StyleSubscription<WidgetClass, WidgetId>>::subscribe(
                context,
                self.get_id(),
                self.get_class().clone(),
            );
        }

        self.on_init(context);
    }

    fn on_init(&mut self, context: &mut C);
}
