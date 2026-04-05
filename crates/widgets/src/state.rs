use std::{any::Any, collections::HashSet, marker::PhantomData};

use crate::types::WidgetId;

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
}

impl StateInfo {
    pub(crate) fn new<T: 'static>(state: T, is_mutable: bool) -> Self {
        Self {
            data: Box::new(state),
            subscribers: HashSet::new(),
            is_mutable,
        }
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

    pub(crate) fn add_subscriber<Id: Into<WidgetId>>(&mut self, subscriber: Id) {
        self.subscribers.insert(subscriber.into());
    }

    pub(crate) fn subscribers(&self) -> impl Iterator<Item = &WidgetId> {
        self.subscribers.iter()
    }
}
