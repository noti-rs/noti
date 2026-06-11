use std::collections::{HashMap, HashSet};

use log::warn;

use crate::{
    context::{LoadExtent, ScopedContext, ScopedManageState},
    types::{Extent, Point, WidgetId},
    widget::{Widget, WidgetGetType, WidgetInformation},
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

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
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
    current_router: EventRouter,

    tracking_routers: HashMap<TrackingEvent, EventRouter>,
}

impl EventManager {
    pub(crate) fn new() -> Self {
        Self {
            current_router: EventRouter::default(),
            tracking_routers: HashMap::new(),
        }
    }

    pub(crate) fn dispatch_event<C>(
        &mut self,
        context: &mut C,
        event: RawEvent,
        widget_tree: &mut Widget,
    ) where
        C: EventContext<f32>,
    {
        match event.kind {
            RawEventKind::MouseMove => {
                self.dispatch_mouse_move(context, event, widget_tree);
            }
            RawEventKind::MouseDown(mouse_button) => {
                self.dispatch_mouse_down(context, event, mouse_button, widget_tree);
            }
            RawEventKind::MouseUp(mouse_button) => {
                self.dispatch_mouse_up(context, event, mouse_button, widget_tree);
            }
        }
    }

    fn dispatch_mouse_move<C>(&mut self, context: &mut C, event: RawEvent, widget_tree: &mut Widget)
    where
        C: EventContext<f32>,
    {
        let mut router = EventRouter::default();
        widget_tree.hit_test(context, event.local_coord, &mut router);

        if let DiffResult::Changed { position } = self.current_router.diff(&router) {
            self.current_router
                .iter_node_metadata_mut()
                .skip(position)
                .for_each(|metadata| Self::make_cancel_event(metadata, &event.kind));

            widget_tree.route_events(context, &self.current_router);
            self.current_router.clear_pending_events();

            router.copy_states_from(&self.current_router);
            router
                .iter_node_metadata_mut()
                .for_each(|metadata| Self::make_pending_event(metadata, &event.kind));

            widget_tree.route_events(context, &router);

            router.clear_pending_events();
            self.current_router = router;
        }
    }

    fn dispatch_mouse_down<C>(
        &mut self,
        context: &mut C,
        event: RawEvent,
        mouse_button: MouseButton,
        widget_tree: &mut Widget,
    ) where
        C: EventContext<f32>,
    {
        if let Some((DiffResult::Changed { position }, tracking)) = self
            .tracking_routers
            .get_mut(&TrackingEvent::Pressed(mouse_button))
            .map(|tracking| (self.current_router.diff(tracking), tracking))
        {
            tracking
                .iter_node_metadata_mut()
                .skip(position)
                .for_each(|metadata| Self::make_cancel_event(metadata, &event.kind));

            widget_tree.route_events(context, tracking);
        }

        self.current_router
            .iter_node_metadata_mut()
            .for_each(|metadata| Self::make_pending_event(metadata, &event.kind));

        widget_tree.route_events(context, &self.current_router);

        self.current_router.clear_pending_events();
        self.tracking_routers.insert(
            TrackingEvent::Pressed(mouse_button),
            self.current_router.clone(),
        );
    }

    fn dispatch_mouse_up<C>(
        &mut self,
        context: &mut C,
        event: RawEvent,
        mouse_button: MouseButton,
        widget_tree: &mut Widget,
    ) where
        C: EventContext<f32>,
    {
        if let Some((DiffResult::Changed { position }, tracking)) = self
            .tracking_routers
            .get_mut(&TrackingEvent::Pressed(mouse_button))
            .map(|tracking| (self.current_router.diff(tracking), tracking))
        {
            tracking
                .iter_node_metadata_mut()
                .skip(position)
                .for_each(|metadata| Self::make_cancel_event(metadata, &event.kind));

            widget_tree.route_events(context, tracking)
        }

        self.current_router
            .iter_node_metadata_mut()
            .for_each(|metadata| Self::make_pending_event(metadata, &event.kind));

        widget_tree.route_events(context, &self.current_router);

        self.current_router.clear_pending_events();
        self.tracking_routers
            .remove(&TrackingEvent::Pressed(mouse_button));
    }

    fn make_cancel_event(metadata: &mut EventNodeMetadata, raw_event: &RawEventKind) {
        match raw_event {
            RawEventKind::MouseMove => {
                if metadata.capabilities.contains(EventNodeCapabilities::HOVER)
                    && metadata.states.contains(&EventNodeStates::Hovered)
                {
                    metadata.states.remove(&EventNodeStates::Hovered);
                    metadata.pending_events.push(PendingEvent::HoverOut);
                }
            }
            RawEventKind::MouseDown(mouse_button) | RawEventKind::MouseUp(mouse_button) => {
                if metadata.capabilities.contains(EventNodeCapabilities::PRESS)
                    && metadata
                        .states
                        .contains(&EventNodeStates::Pressed(*mouse_button))
                {
                    metadata
                        .states
                        .remove(&EventNodeStates::Pressed(*mouse_button));
                    metadata
                        .pending_events
                        .push(PendingEvent::PressOut(*mouse_button));
                }
            }
        }
    }

    fn make_pending_event(metadata: &mut EventNodeMetadata, raw_event: &RawEventKind) {
        match raw_event {
            RawEventKind::MouseMove => {
                if metadata.capabilities.contains(EventNodeCapabilities::HOVER)
                    && !metadata.states.contains(&EventNodeStates::Hovered)
                {
                    metadata.states.insert(EventNodeStates::Hovered);
                    metadata.pending_events.push(PendingEvent::HoverIn);
                }
            }
            RawEventKind::MouseDown(mouse_button) => {
                if metadata.capabilities.contains(EventNodeCapabilities::PRESS)
                    && !metadata
                        .states
                        .contains(&EventNodeStates::Pressed(*mouse_button))
                {
                    metadata
                        .states
                        .insert(EventNodeStates::Pressed(*mouse_button));
                    metadata
                        .pending_events
                        .push(PendingEvent::PressIn(*mouse_button));
                }
            }
            RawEventKind::MouseUp(mouse_button) => {
                if metadata.capabilities.contains(EventNodeCapabilities::CLICK)
                    && metadata
                        .states
                        .contains(&EventNodeStates::Pressed(*mouse_button))
                {
                    metadata
                        .states
                        .remove(&EventNodeStates::Pressed(*mouse_button));
                    metadata
                        .pending_events
                        .push(PendingEvent::Click(*mouse_button));
                }
            }
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum TrackingEvent {
    Pressed(MouseButton),
}

#[derive(Debug, Clone)]
pub(crate) struct EventNodeMetadata {
    widget_id: WidgetId,
    next_node_index: usize,
    capabilities: EventNodeCapabilities,
    states: HashSet<EventNodeStates>,
    pending_events: Vec<PendingEvent>,
}

impl EventNodeMetadata {
    pub(crate) fn new_empty(widget_id: WidgetId) -> Self {
        Self {
            widget_id,
            next_node_index: 0,
            capabilities: EventNodeCapabilities::empty(),
            states: HashSet::new(),
            pending_events: vec![],
        }
    }

    fn clear_pending_events(&mut self) {
        self.pending_events.clear()
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct EventNodeCapabilities: u8 {
        const HOVER = 1;
        const PRESS = 1 << 1;
        const CLICK = 1 << 2;
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum EventNodeStates {
    Hovered,
    Pressed(MouseButton),
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) enum PendingEvent {
    HoverIn,
    HoverOut,
    PressIn(MouseButton),
    PressOut(MouseButton),
    Click(MouseButton),
}

#[derive(Debug, Default, Clone)]
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
            self.register_node_if_missing({
                let mut metadata = EventNodeMetadata::new_empty(widget_id);
                metadata.capabilities = capability;
                metadata
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

    fn diff(&self, other: &Self) -> DiffResult {
        if let Some(position) = self
            .path
            .iter()
            .zip(other.path.iter())
            .position(|(lhs, rhs)| lhs.widget_id != rhs.widget_id)
        {
            DiffResult::Changed { position }
        } else if self.path.len() != other.path.len() {
            DiffResult::Changed {
                position: std::cmp::min(self.path.len(), other.path.len()),
            }
        } else {
            DiffResult::NotChanged
        }
    }

    fn clear_pending_events(&mut self) {
        self.path
            .iter_mut()
            .for_each(EventNodeMetadata::clear_pending_events);
    }

    fn copy_states_from(&mut self, other: &Self) {
        for (current, other) in self
            .path
            .iter_mut()
            .zip(other.path.iter())
            .take_while(|(current, other)| current.widget_id == other.widget_id)
        {
            current.states = other.states.clone();
        }
    }

    fn iter_node_metadata_mut(&mut self) -> impl Iterator<Item = &mut EventNodeMetadata> {
        self.path.iter_mut()
    }

    fn get_metadata(&self, widget_id: WidgetId) -> Option<&EventNodeMetadata> {
        self.map
            .get(&widget_id)
            .and_then(|&index| self.path.get(index))
    }
}

enum DiffResult {
    NotChanged,
    Changed { position: usize },
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

pub(crate) trait EventHandling<T, C>: WidgetInformation
where
    C: EventContext<T>,
    T: Default + Copy,
{
    fn handle_events(
        &mut self,
        context: &mut C,
        pending_events: Vec<PendingEvent>,
        next_child: usize,
        router: &EventRouter,
    );

    fn route_events(&mut self, context: &mut C, router: &EventRouter) {
        let Some(metadata) = router.get_metadata(self.get_id()) else {
            // INFO: possibly widget tries to route to next child widget and it not persisted as event
            // node in path, therefore we ignore it.
            return;
        };

        self.handle_events(
            context,
            metadata.pending_events.clone(),
            metadata.next_node_index,
            router,
        );
    }
}

pub trait FunctionCallback<'a, T, R>: FnMut(ScopedContext<'a>, T) -> R {}
impl<'a, T, R, F> FunctionCallback<'a, T, R> for F where F: FnMut(ScopedContext<'a>, T) -> R {}

pub type Callback<T, R> = Box<dyn for<'a> FunctionCallback<'a, T, R>>;
