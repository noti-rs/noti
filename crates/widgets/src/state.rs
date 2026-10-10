use std::{
    any::{Any, TypeId},
    collections::HashSet,
    marker::PhantomData,
};

use crate::types::WidgetId;

/// States are handles to underlying data managed by an internal system.
///
/// They are lightweight and cannot be meaningful without the internal system. By an internal system,
/// we mean an arbitrary system that manages different data. In the widget system, the internal
/// system is the [crate::context::Context].
///
/// Since states are lightweight handles, an internal system cannot know where they are copied or
/// dropped. Therefore, the internal system relies on explicit state management by explicitly
/// increasing or decreasing the reference counter.
///
/// If a state handle is not explicitly freed, it may cause a memory leak due to incorrect usage.
pub struct State<T: 'static> {
    pub(crate) descriptor: usize,
    pub(crate) _marker: PhantomData<T>,
}

impl<T: 'static> State<T> {
    pub(crate) fn new(descriptor: usize) -> Self {
        Self {
            descriptor,
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for State<T> {}

impl<T: 'static> PartialEq for State<T> {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor
    }
}

/// A mutable subset of [`State<T>`] that allows the underlying data to be modified.
///
/// Like [`State<T>`], mutable states are lightweight handles to data managed by an internal system
/// and cannot be meaningful without it. In the widget system, the internal system is the
/// [crate::context::Context].
///
/// Since mutable states are lightweight handles, an internal system cannot know where they are
/// copied or dropped. Therefore, the internal system relies on explicit state management by
/// explicitly increasing or decreasing the reference counter.
///
/// If a mutable state handle is not explicitly freed, it may cause a memory leak due to incorrect
/// usage.
pub struct MutableState<T: 'static> {
    pub(crate) descriptor: usize,
    pub(crate) _marker: PhantomData<T>,
}

impl<T: 'static> MutableState<T> {
    pub(crate) fn new(descriptor: usize) -> Self {
        Self {
            descriptor,
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for MutableState<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for MutableState<T> {}

impl<T: 'static> From<MutableState<T>> for State<T> {
    fn from(value: MutableState<T>) -> Self {
        Self {
            descriptor: value.descriptor,
            _marker: PhantomData,
        }
    }
}

pub(crate) struct StateInfo {
    data: Box<dyn Any>,
    subscribers: HashSet<WidgetId>,
    is_mutable: bool,
    type_id: TypeId,
    strong_references: usize,
}

impl StateInfo {
    pub(crate) fn new<T: 'static>(state: T, is_mutable: bool) -> Self {
        Self {
            data: Box::new(state),
            subscribers: HashSet::new(),
            is_mutable,
            type_id: TypeId::of::<T>(),
            strong_references: 1,
        }
    }

    pub(crate) fn increase_ref_count(&mut self) {
        self.strong_references = self.strong_references.saturating_add(1);
    }

    pub(crate) fn decrease_ref_count(&mut self) {
        self.strong_references = self.strong_references.saturating_sub(1);
    }

    pub(crate) fn strong_references(&self) -> usize {
        self.strong_references
    }

    pub(crate) fn get_data<T: 'static, S: Into<State<T>>>(&self, _state: S) -> Option<&T> {
        self.data.downcast_ref()
    }

    pub(crate) fn get_data_mut<T: 'static>(
        &mut self,
        _mutable_state: MutableState<T>,
    ) -> Option<&mut T> {
        if !self.is_mutable {
            return None;
        }

        self.data.downcast_mut()
    }

    pub(crate) fn get_raw_data(&self) -> Option<&dyn Any> {
        Some(&*self.data)
    }

    pub(crate) fn set_raw_data(&mut self, raw_data: Box<dyn Any>) -> bool {
        if !self.is_mutable || self.type_id != raw_data.as_ref().type_id() {
            return false;
        }

        self.data = raw_data;
        true
    }

    pub(crate) fn add_subscriber<Id: Into<WidgetId>>(&mut self, subscriber: Id) {
        self.subscribers.insert(subscriber.into());
    }

    pub(crate) fn remove_subscriber<Id: Into<WidgetId>>(&mut self, subscriber: Id) {
        self.subscribers.remove(&subscriber.into());
    }

    pub(crate) fn subscribers(&self) -> impl Iterator<Item = &WidgetId> {
        self.subscribers.iter()
    }
}
