use std::collections::HashMap;

use log::warn;

use crate::{
    context::{LoadExtent, ScopedContext, ScopedManageState},
    types::{Extent, Point, WidgetId},
    widget::{WidgetGetType, WidgetInformation},
};

#[derive(Debug, Clone)]
pub struct RawEvent {
    pub kind: RawEventKind,
    pub local_coord: Point<f32>,
}

#[derive(Debug, Clone)]
pub enum RawEventKind {
    MouseMove,
    MouseDown(MouseButton),
    MouseUp(MouseButton),
}

#[derive(Debug, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub local_coord: Point<f32>,
}

#[derive(Debug, Clone)]
pub enum EventKind {
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    MouseHover,
}

impl EventKind {
    pub fn is_mouse(&self) -> bool {
        matches!(
            self,
            Self::MouseDown(_) | Self::MouseUp(_) | EventKind::MouseHover
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

pub(crate) trait EventContext<T>: LoadExtent<T, WidgetId> + ScopedManageState
where
    T: Default + Copy,
{
}

impl<C, T> EventContext<T> for C
where
    C: LoadExtent<T, WidgetId> + ScopedManageState,
    T: Default + Copy,
{
}

pub(crate) struct EventManager {
    current_route: Vec<EventNodeMetadata>,
}

#[derive(Debug, Clone)]
pub(crate) struct EventNodeMetadata {
    widget_id: WidgetId,
    next_node_index: usize,
    capabilities: EventNodeCapabilities,
}

impl EventNodeMetadata {
    pub(crate) fn new_empty(widget_id: WidgetId) -> Self {
        Self {
            widget_id,
            next_node_index: 0,
            capabilities: EventNodeCapabilities::empty(),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct EventNodeCapabilities: u8 {
        const HOVERED = 1;
        const PRESSED = 1 << 1;
        const CLICKED = 1 << 2;
    }
}

#[derive(Debug, Default)]
pub(crate) struct EventRouter {
    path: Vec<EventNodeMetadata>,
    map: HashMap<WidgetId, usize>,
}

impl EventRouter {
    pub(crate) fn set_next_index(&mut self, widget_id: WidgetId, next_index: usize) {
        if let Some(current_node) = self
            .map
            .get(&widget_id)
            .and_then(|&index| self.path.get_mut(index))
        {
            current_node.next_node_index = next_index;
        }
    }

    pub(crate) fn add_capability(
        &mut self,
        widget_id: WidgetId,
        capability: EventNodeCapabilities,
    ) {
        if let Some(node) = self
            .map
            .get(&widget_id)
            .and_then(|&index| self.path.get_mut(index))
        {
            node.capabilities |= capability;
        } else {
            self.register_node_if_missing(EventNodeMetadata {
                widget_id,
                capabilities: capability,
                next_node_index: 0,
            });
        }
    }

    pub(crate) fn register_node_if_missing(&mut self, node: EventNodeMetadata) {
        if !self.map.contains_key(&node.widget_id) {
            let widget_id = node.widget_id;
            self.map.insert(widget_id, self.path.len());
            self.path.push(node);
        }
    }

    /// Deletes last node only and only if it has matching widget_id and its capabilities is empty.
    fn remove_missed_last_node(&mut self, widget_id: WidgetId) {
        if self
            .path
            .last()
            .is_some_and(|last_node| last_node.widget_id == widget_id)
        {
            self.path.pop();
            self.map.remove(&widget_id);
        }
    }
}

pub(crate) trait EventHitTest<T, C>: WidgetInformation + WidgetGetType
where
    T: Default + Copy,
    C: EventContext<T>,
{
    fn on_hit_test(
        &self,
        context: &C,
        local_coords: Point<T>,
        provided_extent: Extent<T>,
        router: &mut EventRouter,
    ) -> HitTestResult;

    fn hit_test(
        &self,
        context: &C,
        local_coords: Point<T>,
        router: &mut EventRouter,
    ) -> HitTestResult {
        let Some(provided_extent) = <C as LoadExtent<T, WidgetId>>::load(context, self.get_id())
        else {
            warn!(
                "{} widget with id {} didn't measured! Refused to hit test.",
                self.get_type(),
                *self.get_id()
            );
            return HitTestResult::Failed;
        };

        router.register_node_if_missing(EventNodeMetadata::new_empty(self.get_id()));
        let result = self.on_hit_test(context, local_coords, provided_extent, router);

        match &result {
            HitTestResult::Missed => {
                router.remove_missed_last_node(self.get_id());
            }
            HitTestResult::Hit | HitTestResult::Failed => (),
        }

        result
    }
}

pub(crate) enum HitTestResult {
    Hit,
    Missed,
    Failed,
}

pub(crate) trait EventRouting {
    fn route_event();
}

pub trait FunctionCallback<'a, T, R>: FnMut(ScopedContext<'a>, T) -> R {}
impl<'a, T, R, F> FunctionCallback<'a, T, R> for F where F: FnMut(ScopedContext<'a>, T) -> R {}

pub type Callback<T, R> = Box<dyn for<'a> FunctionCallback<'a, T, R>>;

pub(crate) trait DispatchEvent<C, T>
where
    C: EventContext<T>,
    T: Default + Copy,
{
    fn dispatch_event(&mut self, context: &mut C, event: Event);
}
