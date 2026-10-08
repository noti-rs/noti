//! Deinitialization stage of a widget.
//!
//! The last stage in widget's lifecycle, freeing its runtime data that belongs to a Context.
//! To allow a widget to be deinitialized, apply [Deinit] trait to it. In case of existing a runtime
//! resource that not belongs to a Context or needed to be manually unlinked, implement the
//! [Deinit::on_deinit] method to free this data.

use crate::{
    context::{
        ClearWidgetResources, ManageAnimationRegistry, StateLifetimeManagement, StateSubscription,
        UnregisterKey,
    },
    types::{identifiers::WidgetKey, WidgetId},
    widget::{WidgetBase, WidgetInformationContext},
};

pub trait DeinitContext:
    WidgetInformationContext
    + UnregisterKey<WidgetKey>
    + StateSubscription<WidgetId>
    + StateLifetimeManagement
    + ManageAnimationRegistry<WidgetId>
    + ClearWidgetResources<WidgetId>
{
}

impl<C> DeinitContext for C where
    C: WidgetInformationContext
        + UnregisterKey<WidgetKey>
        + StateSubscription<WidgetId>
        + StateLifetimeManagement
        + ManageAnimationRegistry<WidgetId>
        + ClearWidgetResources<WidgetId>
{
}

/// Applies the Deinit trait to a widget.
///
/// A widget with Deinit trait can be freed from a Context with clearing all related runtime resources.
/// Since all runtime data of a widget belongs to a Context, it can be freed without manipulating
/// the data.
///
/// But if a widget have some unrelated to a Context runtime data or related to it states, better to free
/// it implementing the [Deinit::on_deinit] method.
pub trait Deinit<C>: WidgetBase<C>
where
    C: DeinitContext,
{
    fn deinit(&mut self, context: &mut C) {
        self.on_deinit(context);

        if let Some(key) = self.get_key() {
            context.unregister_key(key.clone());
        }

        context.clear_widget_resources(self.get_id());
    }

    /// Frees unrelated to a Context runtime data or related to it states.
    fn on_deinit(&mut self, _context: &mut C) {}
}
