use indextree::NodeId;
use shared::unique::Unique;

use crate::{
    events::EventContext,
    forest::Forest,
    stage::{
        draw::DrawContext,
        layout::LayoutContext,
        measure::{Constraints, Intrinsic, MeasureContext},
        rebuild::{
            keyed_diffing, positional_diffing, scan_forest, RebuildContext, RebuildOperation,
            RebuildTree,
        },
    },
    state::{MutableState, State, StateInfo},
    types::{
        dirty_flags::{propagate_needs_measure, DirtyFlags},
        identifiers::WidgetKey,
        Extent, Point, WidgetId,
    },
    widget::{Widget, WidgetInformationContext},
};
use std::{any::Any, collections::HashMap, time::Duration};

/// A context for a widget tree that stores runtime data and other information associated with its
/// widgets.
///
/// It is essential to a widget tree because widgets are blueprints, while their runtime data is
/// stored in the context. The context can operate on widgets without depending on their structure
/// and provides the information required to perform runtime operations.
///
/// The implementation of [Context] is hidden from users but available to developers.
/// See the internal documentation for more details.
pub struct Context {
    debug_options: DebugOptions,

    /// The id counter for generating identifiers for widgets.
    id_counter: u64,

    /// The number counter for generating state descriptors.
    state_descriptor_counter: usize,

    /// Forest of widget trees, containing main and pending trees.
    widget_forest: Forest<WidgetId, Box<dyn Widget<Context>>>,

    /// Registry of arbitrary data associated with particular widgets.
    widget_data_registry: HashMap<WidgetId, Box<dyn Any>>,

    /// Registry of measurement caches for avoiding unnecessary widget re-measurement.
    measure_cache: HashMap<WidgetId, MeasureCache>,

    /// Registry of mutable and immutable states bound to their descriptors.
    state_registry: HashMap<usize, StateInfo>,

    /// Registry of unique widget keys mapped to known widget IDs.
    key_registry: HashMap<WidgetKey, WidgetId>,

    /// Registry of dirty flags bound to widget IDs.
    dirty_registry: HashMap<WidgetId, DirtyFlags>,

    /// Registry of animation progress associated with widget IDs.
    animation_regirsty: HashMap<WidgetId, AnimationProgress>,

    /// Global storage of fonts used for text rendering without wasting memory.
    font_collection: skia_safe::textlayout::FontCollection,
}

impl Context {
    pub fn new(font_collection: skia_safe::textlayout::FontCollection) -> Self {
        Self {
            debug_options: DebugOptions::default(),
            id_counter: 1,
            state_descriptor_counter: 1,
            font_collection,
            widget_forest: Forest::default(),
            widget_data_registry: HashMap::new(),
            measure_cache: HashMap::new(),
            state_registry: HashMap::new(),
            key_registry: HashMap::new(),
            dirty_registry: HashMap::new(),
            animation_regirsty: HashMap::new(),
        }
    }

    pub(super) fn main_tree_root(&self) -> Option<&WidgetId> {
        self.widget_forest.main_tree_root_id()
    }

    pub fn update_debug_options(&mut self, debug_options: DebugOptions) {
        self.debug_options = debug_options;
    }

    fn register_state(&mut self, state_info: StateInfo) -> usize {
        let descriptor = self.state_descriptor_counter;
        self.state_descriptor_counter += 1;
        self.state_registry.insert(descriptor, state_info);
        descriptor
    }

    pub(super) fn has_pending_tree_root(&self) -> bool {
        self.widget_forest.pending_tree_root().is_some()
    }

    pub(super) fn has_invalid_widgets(&self) -> bool {
        self.dirty_registry
            .values()
            .any(|flags| flags.intersects(DirtyFlags::NEEDS_REBUILD))
    }

    pub(super) fn invalidate_widgets(&mut self) {
        let suitable_widget_identifiers = self
            .dirty_registry
            .iter()
            .filter(|(_, flags)| flags.intersects(DirtyFlags::NEEDS_REBUILD))
            .map(|(widget_id, _)| *widget_id)
            .collect::<Vec<_>>();

        for widget_id in &suitable_widget_identifiers {
            if let Some(mut widget) = self.widget_forest.node_by_id_mut(widget_id) {
                widget.invalidate(self);
            }
        }
    }

    pub(super) fn needs_measurement(&self) -> bool {
        self.dirty_registry.values().any(|flags| {
            flags.intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE)
        })
    }

    pub(super) fn measure_widgets(&mut self, root_constraints: Constraints<Extent<f32>>) {
        if let Some(root_id) = self.widget_forest.main_tree_root_id().copied() {
            let root_dirty_flags = self
                .dirty_registry
                .get(&root_id)
                .copied()
                .unwrap_or(DirtyFlags::empty());

            if root_dirty_flags
                .intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE)
            {
                let main_root = self
                    .widget_forest
                    .node_by_id_mut(&root_id)
                    .expect("Since there is a root ID, then there must be a main root!");

                main_root.measure(self, root_constraints);
            }
        }

        // INFO: it's a special case when re-measuring from root might not affect some branches of
        // widget tree, because constraints are not changed for them. Then we need to check manually
        // and re-measure.
        //
        // Also it applies if re-measurement starts not from a root, but from some fixed-sized parents.
        let unmeasured_parent_widgets = self
            .dirty_registry
            .iter()
            .filter(|(_, flags)| flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE))
            .map(|(widget_id, _)| *widget_id)
            .collect::<Vec<_>>();

        for widget_id in &unmeasured_parent_widgets {
            let widget = self
                .widget_forest
                .node_by_id_mut(widget_id)
                .expect("Since there is a widget ID, then there must be a widget in a main tree!");
            let saved_constraints =
                <Context as LoadConstraints<f32, WidgetId>>::load(self, *widget_id)
                    .unwrap_or_default();

            widget.measure(self, saved_constraints);
        }
    }

    pub(super) fn needs_layout(&self) -> bool {
        self.dirty_registry
            .values()
            .any(|flags| flags.contains(DirtyFlags::NEEDS_LAYOUT))
    }

    pub(super) fn layout_widgets(&mut self) {
        let suitable_widget_identifiers = self
            .dirty_registry
            .iter()
            .filter(|(_, flags)| flags.contains(DirtyFlags::NEEDS_LAYOUT))
            .map(|(widget_id, _)| *widget_id)
            .collect::<Vec<_>>();

        for widget_id in &suitable_widget_identifiers {
            let dirty_flags = self
                .dirty_registry
                .get(widget_id)
                .copied()
                .unwrap_or(DirtyFlags::empty());

            // INFO: during re-layouting one widget it may affect other, so here need ot check to
            // avoid unnecessary re-layouts.
            if !dirty_flags.contains(DirtyFlags::NEEDS_LAYOUT) {
                continue;
            }

            let mut current_node_id = *widget_id;
            while let Some(parent_id) = self.widget_forest.parent_id_by_id(current_node_id) {
                let parent_dirty_flags = self
                    .dirty_registry
                    .get(&parent_id)
                    .copied()
                    .unwrap_or(DirtyFlags::empty());
                current_node_id = parent_id;

                if !parent_dirty_flags.contains(DirtyFlags::NEEDS_LAYOUT) {
                    break;
                }
            }

            let mut current_widget = self
                .widget_forest
                .node_by_id_mut(&current_node_id)
                .expect("Since there is a widget ID, then there must be a widget in a main tree!");

            let local_coord = if Some(&current_node_id) == self.widget_forest.main_tree_root_id() {
                Point::default()
            } else {
                <Context as LoadLocalCoord<f32, WidgetId>>::load(self, current_node_id)
                    .unwrap_or_default()
            };

            current_widget.layout(self, local_coord);
        }
    }

    pub(super) fn collect_input_regions(&self) -> Vec<(Point<f32>, Extent<f32>)> {
        let mut input_regions = vec![];

        if let Some(main_tree_root_id) = self.widget_forest.main_tree_root_id() {
            collect_input_regions(*main_tree_root_id, self, &mut input_regions);
        }

        input_regions
    }
}

/// A collection of options that made for debuggin an UI or internal work detail.
#[derive(Clone, Debug)]
pub struct DebugOptions {
    /// Shows the layout bounds of each UI element using different colors.
    pub show_layout_bounds: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for DebugOptions {
    fn default() -> Self {
        Self {
            show_layout_bounds: false,
        }
    }
}

/// A collection of a widget measurement. Usually these widget measurements
/// are cached into [Context] to reuse them further witouth re-measuring.
#[derive(Debug, Clone, Default)]
struct MeasureCache {
    /// The local coordinates of a widget.
    ///
    /// The value always set by a parent widget.
    local_coord: Option<Point<f32>>,

    /// The intrinsic size of a widget.
    ///
    /// A widget must know what is the minimal and the maximal size of itself.
    intrinsic: Option<Intrinsic<f32>>,

    /// The actual exetnt of a widget under a speficif constraints.
    extent: Option<Extent<f32>>,

    /// The constraints that used to a particular widget. It uses for re-measuring to check whether
    /// constraints were changed or not.
    ///
    /// The rule is simple: if constraints were changed then do re-measurement, otherwise use the
    /// cached extent.
    constraints: Option<Constraints<Extent<f32>>>,
}

/// The information about animation for a widget.
///
/// The animation progress is detached from widget, but provides information to it. It allows tick a
/// progress independently and tell the [Context] whether to call a widget to rebuild or not.
#[derive(Debug, Clone)]
pub struct AnimationProgress {
    /// The time was passed since start in nanoseconds.
    ///
    /// Actual start might be at zero or at maximum duration depending on an animation direction. If
    /// it passing forward then it will start at zero, otherwise at maximum duration but moves
    /// toward to zero.
    elapsed_ns: u128,

    /// The maximum duration of an animation.
    duration: Duration,

    /// The dirty flags to pass a widget to trigger some actions from the [Context].
    flag_on_change: DirtyFlags,

    /// The animation direction telling to increase or decrease the passed time.
    direction: AnimationDirection,
}

#[derive(Debug, Clone)]
pub(crate) enum AnimationDirection {
    Forward,
    Backward,
}

impl AnimationProgress {
    pub(crate) fn new(
        duration: Duration,
        flag_on_change: DirtyFlags,
        direction: AnimationDirection,
    ) -> Self {
        Self {
            elapsed_ns: match &direction {
                AnimationDirection::Forward => 0,
                AnimationDirection::Backward => duration.as_nanos(),
            },
            duration,
            flag_on_change,
            direction,
        }
    }

    fn reverse(&mut self) {
        self.direction.reverse()
    }

    fn progress(&self) -> f32 {
        self.elapsed_ns as f32 / self.duration.as_nanos() as f32
    }

    fn is_finished(&self) -> bool {
        match self.direction {
            AnimationDirection::Forward => self.elapsed_ns == self.duration.as_nanos(),
            AnimationDirection::Backward => self.elapsed_ns == 0,
        }
    }
}

impl AnimationDirection {
    fn reverse(&mut self) {
        *self = match self {
            AnimationDirection::Forward => AnimationDirection::Backward,
            AnimationDirection::Backward => AnimationDirection::Forward,
        }
    }
}

impl Tick for AnimationProgress {
    fn tick(&mut self, delta_ns: u128) {
        match self.direction {
            AnimationDirection::Forward => {
                let duration_ns = self.duration.as_nanos();
                self.elapsed_ns = (self.elapsed_ns + delta_ns).min(duration_ns);
            }
            AnimationDirection::Backward => {
                self.elapsed_ns = self.elapsed_ns - delta_ns.min(self.elapsed_ns);
            }
        }
    }
}

pub trait GetDebugOptions {
    /// Retrieves debug options.
    fn get_debug_options(&self) -> &DebugOptions;
}

impl GetDebugOptions for Context {
    fn get_debug_options(&self) -> &DebugOptions {
        &self.debug_options
    }
}

impl WidgetInformationContext for Context {
    fn widget_sizing_mode(
        &self,
        widget_id: &WidgetId,
    ) -> Option<crate::stage::measure::SizingMode> {
        self.widget_forest
            .node_by_id(widget_id)
            .map(|w| w.sizing_mode(self))
    }
}

pub trait WidgetTreeCreation {
    fn create_widget(&mut self, widget: Box<dyn Widget<Context>>) -> NodeId;
    fn append_child(&mut self, parent: NodeId, child: NodeId);
    fn set_pending_root(&mut self, root: NodeId);
}

impl WidgetTreeCreation for Context {
    fn create_widget(&mut self, widget: Box<dyn Widget<Context>>) -> NodeId {
        self.widget_forest.create_node(widget)
    }

    fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.widget_forest.append_node(parent, child)
    }

    fn set_pending_root(&mut self, root: NodeId) {
        self.widget_forest.set_pending_tree_root(root)
    }
}

pub trait WidgetTreeAccess<Id>
where
    Id: Into<WidgetId>,
{
    fn widget_by(&self, id: Id) -> Option<&dyn Widget<Context>>;
    fn widget_by_mut(&mut self, id: Id) -> Option<Unique<dyn Widget<Context>>>;

    fn parent_id_of(&self, id: Id) -> Option<WidgetId>;
    fn parent_of(&self, id: Id) -> Option<&dyn Widget<Context>>;
    fn parent_of_mut(&mut self, id: Id) -> Option<Unique<dyn Widget<Context>>>;

    fn childrens_identifiers_of(&self, id: Id) -> Vec<WidgetId>;
    fn children_of(&self, id: Id) -> Vec<&dyn Widget<Context>>;
    fn children_of_mut(&mut self, id: Id) -> Vec<Unique<dyn Widget<Context>>>;
}

impl<Id> WidgetTreeAccess<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn widget_by(&self, id: Id) -> Option<&dyn Widget<Context>> {
        self.widget_forest.node_by_id(&id.into()).map(|w| &**w)
    }

    fn widget_by_mut(&mut self, id: Id) -> Option<Unique<dyn Widget<Context>>> {
        self.widget_forest
            .node_by_id_mut(&id.into())
            .map(|mut w| unsafe { Unique::from_mut(&mut **w) })
    }

    fn parent_id_of(&self, id: Id) -> Option<WidgetId> {
        self.widget_forest.parent_id_by_id(id.into())
    }

    fn parent_of(&self, id: Id) -> Option<&dyn Widget<Context>> {
        self.widget_forest.parent_by_id(id.into()).map(|w| &**w)
    }

    fn parent_of_mut(&mut self, id: Id) -> Option<Unique<dyn Widget<Context>>> {
        self.widget_forest
            .parent_by_id_mut(id.into())
            .map(|mut w| unsafe { Unique::from_mut(&mut **w) })
    }

    fn childrens_identifiers_of(&self, id: Id) -> Vec<WidgetId> {
        self.widget_forest.childrens_indices_by_id(id.into())
    }

    fn children_of(&self, id: Id) -> Vec<&dyn Widget<Context>> {
        self.widget_forest
            .children_by_id(id.into())
            .into_iter()
            .map(|w| &**w)
            .collect()
    }

    fn children_of_mut(&mut self, id: Id) -> Vec<Unique<dyn Widget<Context>>> {
        self.widget_forest
            .children_by_id_mut(id.into())
            .into_iter()
            .map(|mut w| unsafe { Unique::from_mut(&mut **w) })
            .collect()
    }
}

impl RebuildTree for Context {
    fn rebuild_tree(&mut self) {
        if self.widget_forest.pending_tree_root().is_none() {
            return;
        }

        self.widget_forest.remove_standalone_subtrees();

        let mut rebuild_context = RebuildContext::default();
        scan_forest(&self.widget_forest, &mut rebuild_context);
        positional_diffing(
            &self.widget_forest,
            self.widget_forest.main_tree_root(),
            self.widget_forest.pending_tree_root(),
            &mut rebuild_context,
        );
        keyed_diffing(&self.widget_forest, &mut rebuild_context);
        perform_operation_set(self, &rebuild_context);

        self.widget_forest.promote_pending_tree_root();
    }
}

fn perform_operation_set(context: &mut Context, rebuild_context: &RebuildContext) {
    for operation in rebuild_context.operation_set() {
        match operation {
            RebuildOperation::Initialize { node_id } => {
                let mut node = unsafe {
                    Unique::from_mut(
                        &mut **context
                            .widget_forest
                            .node_mut(node_id)
                            .expect("There must be a node in forest!"),
                    )
                };
                node.init(context);
                context.widget_forest.make_relation(node.get_id(), *node_id);
            }
            RebuildOperation::Deinitialize { node_id } => {
                let mut node = unsafe {
                    Unique::from_mut(
                        &mut **context
                            .widget_forest
                            .node_mut(node_id)
                            .expect("There must be a node in forest!"),
                    )
                };
                let id = node.get_id();
                node.deinit(context);
                context.widget_forest.remove_relation(id);
            }
            RebuildOperation::Reuse {
                old_node_id,
                new_node_id,
            } => {
                let old_node = context
                    .widget_forest
                    .node(old_node_id)
                    .expect("There must be a node in a forest!");
                let old_node_id = old_node.get_id();

                let new_node = context
                    .widget_forest
                    .node(new_node_id)
                    .expect("There must be a node in a forest!");

                let dirty_flags = new_node.diff(old_node.as_ref());

                let new_node_mut = context
                    .widget_forest
                    .node_mut(new_node_id)
                    .expect("There must be a node in a forest!");
                new_node_mut.set_id(old_node_id);

                context
                    .widget_forest
                    .make_relation(old_node_id, *new_node_id);

                context.append_dirty_flags(old_node_id, dirty_flags);
                if dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) {
                    propagate_needs_measure(old_node_id, context);
                }
            }
        }
    }
}

/// Retrieves a reference to a widget data that bound to its id.
///
/// Instead of trying to write the whole turbofish generics better to use output matching:
///```rs
///let some_data: &WidgetData = widget_data(context, &self.id).except("There's must be a widget data.")
///```
pub fn widget_data<T: 'static, C, Id>(context: &C, widget_id: Id) -> Option<&T>
where
    C: ManageWidgetData<Id>,
    Id: Into<WidgetId>,
{
    context
        .get_widget_data(widget_id)
        .and_then(|data| data.downcast_ref::<T>())
}

/// Retrieves an unique access to a widget data that bound to its id.
///
/// Instead of trying to write the whole turbofish generics better to use output matching:
///```rs
///let some_data: Unique<WidgetData> = widget_data_mut(context, &self.id).except("There's must be a widget data.")
///```
pub fn widget_data_mut<T: 'static, C, Id>(context: &mut C, widget_id: Id) -> Option<Unique<T>>
where
    C: ManageWidgetData<Id>,
    Id: Into<WidgetId>,
{
    context
        .get_widget_data_mut(widget_id)
        .and_then(|data| data.downcast_mut::<T>())
        .map(|data| unsafe { Unique::from_mut(data) })
}

pub trait ManageWidgetData<Id>
where
    Id: Into<WidgetId>,
{
    /// Saves a provided widget data by its id into a context.
    fn set_widget_data(&mut self, widget_id: Id, widget_data: Box<dyn Any>);

    /// Retrieves an associated widget data by reference.
    fn get_widget_data(&self, widget_id: Id) -> Option<&dyn Any>;

    /// Retrieves an associated widget data by mutable reference.
    fn get_widget_data_mut(&mut self, widget_id: Id) -> Option<&mut dyn Any>;

    /// Removes a widget data from a context.
    fn remove_widget_data(&mut self, widget_id: Id);
}

impl<Id> ManageWidgetData<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn set_widget_data(&mut self, widget_id: Id, widget_data: Box<dyn Any>) {
        self.widget_data_registry
            .insert(widget_id.into(), widget_data);
    }

    fn get_widget_data(&self, widget_id: Id) -> Option<&dyn Any> {
        self.widget_data_registry
            .get(&widget_id.into())
            .map(|data| &**data)
    }
    fn get_widget_data_mut(&mut self, widget_id: Id) -> Option<&mut dyn Any> {
        self.widget_data_registry
            .get_mut(&widget_id.into())
            .map(|data| &mut **data)
    }

    fn remove_widget_data(&mut self, widget_id: Id) {
        self.widget_data_registry.remove(&widget_id.into());
    }
}

pub trait CreateState<T> {
    /// Creates a state with a provided value in a context.
    fn create_state(&mut self, value: T) -> State<T>;

    /// Creates a mutable stata with a provided value in a context.
    fn create_state_mut(&mut self, value: T) -> MutableState<T>;
}

impl<T> CreateState<T> for Context {
    fn create_state(&mut self, value: T) -> State<T> {
        let state_info = StateInfo::new(value, false);
        State::new(self.register_state(state_info))
    }

    fn create_state_mut(&mut self, value: T) -> MutableState<T> {
        let state_info = StateInfo::new(value, true);
        MutableState::new(self.register_state(state_info))
    }
}

/// The scoped context that solves the [Context] issues with traits that cannot be dyn-compatible.
///
/// It aims to scope users callbacks, in which enough to manage states. And with it the [Context]
/// can be reduced to it's capabilities of [GetState] and [SetState] by using dyn-compatible [ScopedManageState].
pub struct ScopedContext<'a> {
    inner: &'a mut dyn ScopedManageState,
}

impl<'a> ScopedContext<'a> {
    pub(crate) fn new<C: ScopedManageState>(context: &'a mut C) -> Self {
        Self { inner: context }
    }
}

/// The dyn-compatible version of [GetState] and [SetState].
pub trait ScopedManageState {
    fn get_raw(&self, descriptor: usize) -> Option<&dyn Any>;
    fn set_raw(&mut self, descriptor: usize, val: Box<dyn Any>);
}

impl ScopedManageState for Context {
    fn set_raw(&mut self, descriptor: usize, val: Box<dyn Any>) {
        if let Some(state_info) = self.state_registry.get_mut(&descriptor) {
            if state_info.set_raw_data(val) {
                // TODO: check the validity of exising widget
                // For instance, in subscriber list may be some unexisting widget and marking dirty
                // flags will be invalid
                state_info.subscribers().for_each(|subscriber| {
                    self.dirty_registry
                        .entry(*subscriber)
                        .and_modify(|flags| *flags |= DirtyFlags::NEEDS_REBUILD)
                        .or_insert(DirtyFlags::NEEDS_REBUILD);
                });
            }
        }
    }

    fn get_raw(&self, descriptor: usize) -> Option<&dyn Any> {
        self.state_registry
            .get(&descriptor)
            .and_then(|state_info| state_info.get_raw_data())
    }
}

pub trait GetState {
    /// Retrieves a state by it's descriptor.
    fn get<T: 'static, S: Into<State<T>>>(&self, state: S) -> Option<&T>;
}

pub trait SetState {
    /// Assigns a value to a state by it's descriptor.
    fn set<T: 'static>(&mut self, mutable_state: MutableState<T>, value: T);
}

impl GetState for Context {
    fn get<T, S>(&self, state: S) -> Option<&T>
    where
        T: 'static,
        S: Into<State<T>>,
    {
        let state = state.into();
        self.state_registry
            .get(&state.descriptor)
            .and_then(|state_info| state_info.get_data(state))
    }
}

impl SetState for Context {
    fn set<T: 'static>(&mut self, mutable_state: MutableState<T>, value: T) {
        if let Some(state_info) = self.state_registry.get_mut(&mutable_state.descriptor) {
            if let Some(data) = state_info.get_data_mut(mutable_state) {
                *data = value;

                // TODO: check the validity of exising widget
                // For instance, in subscriber list may be some unexisting widget and marking dirty
                // flags will be invalid
                state_info.subscribers().for_each(|subscriber| {
                    self.dirty_registry
                        .entry(*subscriber)
                        .and_modify(|flags| *flags |= DirtyFlags::NEEDS_REBUILD)
                        .or_insert(DirtyFlags::NEEDS_REBUILD);
                });
            }
        }
    }
}

impl<'a> GetState for ScopedContext<'a> {
    fn get<T: 'static, S: Into<State<T>>>(&self, state: S) -> Option<&T> {
        let state = state.into();

        self.inner
            .get_raw(state.descriptor)
            .and_then(|val| val.downcast_ref())
    }
}

impl<'a> SetState for ScopedContext<'a> {
    fn set<T: 'static>(&mut self, mutable_state: MutableState<T>, value: T) {
        self.inner
            .set_raw(mutable_state.descriptor, Box::new(value));
    }
}

pub trait StateSubscription<Id>
where
    Id: Into<WidgetId>,
{
    /// A provided id subscribes to state changes by its descriptor.
    fn subscribe<S, T>(&mut self, id: Id, state: S)
    where
        S: Into<State<T>>,
        T: 'static;

    /// A provided id unsubscribes from state change by its descriptor.
    fn unsubscribe<S, T>(&mut self, id: Id, state: S)
    where
        S: Into<State<T>>,
        T: 'static;
}

impl<Id> StateSubscription<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn subscribe<S, T>(&mut self, id: Id, state: S)
    where
        S: Into<State<T>>,
        T: 'static,
    {
        let state = state.into();

        if let Some(state_info) = self.state_registry.get_mut(&state.descriptor) {
            state_info.add_subscriber(id);
        }
    }

    fn unsubscribe<S, T>(&mut self, id: Id, state: S)
    where
        S: Into<State<T>>,
        T: 'static,
    {
        if let Some(state_info) = self.state_registry.get_mut(&state.into().descriptor) {
            state_info.remove_subscriber(id);
        }
    }
}

pub trait Tick {
    fn tick(&mut self, delta_ns: u128);
}

impl Tick for Context {
    fn tick(&mut self, delta_ns: u128) {
        for (widget_id, animation_progress) in &mut self.animation_regirsty {
            animation_progress.tick(delta_ns);

            if !animation_progress.flag_on_change.is_empty() {
                self.dirty_registry
                    .entry(*widget_id)
                    .and_modify(|flags| *flags |= animation_progress.flag_on_change)
                    .or_insert(animation_progress.flag_on_change);
            }
        }
    }
}

pub trait GenerateId {
    fn generate_id(&mut self) -> WidgetId;
}

impl GenerateId for Context {
    fn generate_id(&mut self) -> WidgetId {
        let id = self.id_counter;
        self.id_counter += 1;
        id.into()
    }
}

pub trait RegisterKey<K, Id>
where
    K: Into<WidgetKey>,
    Id: Into<WidgetId>,
{
    fn register_key(&mut self, key: K, id: Id);
}

impl<K, Id> RegisterKey<K, Id> for Context
where
    K: Into<WidgetKey>,
    Id: Into<WidgetId>,
{
    fn register_key(&mut self, key: K, id: Id) {
        self.key_registry.insert(key.into(), id.into());
    }
}

pub trait UnregisterKey<K>
where
    K: Into<WidgetKey>,
{
    fn unregister_key(&mut self, key: K);
}

impl<K> UnregisterKey<K> for Context
where
    K: Into<WidgetKey>,
{
    fn unregister_key(&mut self, key: K) {
        self.key_registry.remove(&key.into());
    }
}

pub trait ManageDirtyFlags<Id>
where
    Id: Into<WidgetId>,
{
    fn get_dirty_flags(&self, id: Id) -> DirtyFlags;
    fn set_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags);
    fn append_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags);
    fn remove_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags);
    fn clear_dirty_flags(&mut self, id: Id);
}

impl<Id> ManageDirtyFlags<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn get_dirty_flags(&self, id: Id) -> DirtyFlags {
        self.dirty_registry
            .get(&id.into())
            .cloned()
            .unwrap_or_else(DirtyFlags::empty)
    }

    fn set_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags) {
        if dirty_flags.is_empty() {
            self.clear_dirty_flags(id);
        } else {
            self.dirty_registry.insert(id.into(), dirty_flags);
        }
    }

    fn append_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags) {
        if !dirty_flags.is_empty() {
            self.dirty_registry
                .entry(id.into())
                .and_modify(|flags| *flags |= dirty_flags)
                .or_insert(dirty_flags);
        }
    }

    fn remove_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags) {
        let widget_id = id.into();

        if let Some(flags) = self.dirty_registry.get_mut(&widget_id) {
            flags.remove(dirty_flags);

            if flags.is_empty() {
                self.dirty_registry.remove(&widget_id);
            }
        }
    }

    fn clear_dirty_flags(&mut self, id: Id) {
        self.dirty_registry.remove(&id.into());
    }
}

pub trait GetFont {
    fn get_font(&self) -> skia_safe::textlayout::FontCollection;
}

impl GetFont for Context {
    fn get_font(&self) -> skia_safe::textlayout::FontCollection {
        self.font_collection.clone()
    }
}

pub trait SaveLocalCoord<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, local_coord: Point<T>);
}

pub trait LoadLocalCoord<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Point<T>>;
}

pub trait ManageLocalCoord<T, Id>: SaveLocalCoord<T, Id> + LoadLocalCoord<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> ManageLocalCoord<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: SaveLocalCoord<T, Id> + LoadLocalCoord<T, Id>,
{
}

pub trait SaveIntrinsic<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, intrinsic: Intrinsic<T>);
}

pub trait LoadIntrinsic<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Intrinsic<T>>;
}

pub trait ManageIntrinsic<T, Id>: SaveIntrinsic<T, Id> + LoadIntrinsic<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> ManageIntrinsic<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: SaveIntrinsic<T, Id> + LoadIntrinsic<T, Id>,
{
}

pub trait SaveExtent<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, extent: Extent<T>);
}

pub trait LoadExtent<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Extent<T>>;
}

pub trait ManageExtent<T, Id>: SaveExtent<T, Id> + LoadExtent<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> ManageExtent<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: SaveExtent<T, Id> + LoadExtent<T, Id>,
{
}

pub trait SaveConstraints<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, constraints: Constraints<Extent<T>>);
}

pub trait LoadConstraints<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Constraints<Extent<T>>>;
}

pub trait ManageConstraints<T, Id>: SaveConstraints<T, Id> + LoadConstraints<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> ManageConstraints<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: SaveConstraints<T, Id> + LoadConstraints<T, Id>,
{
}

impl<Id> SaveLocalCoord<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, local_coord: Point<f32>) {
        self.measure_cache
            .entry(id.into())
            .and_modify(|cache| {
                cache.local_coord.replace(local_coord);
            })
            .or_insert(MeasureCache {
                local_coord: local_coord.into(),
                ..Default::default()
            });
    }
}

impl<Id> LoadLocalCoord<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Point<f32>> {
        self.measure_cache
            .get(&id.into())
            .and_then(|cache| cache.local_coord)
    }
}

impl<Id> SaveIntrinsic<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, intrinsic: Intrinsic<f32>) {
        self.measure_cache
            .entry(id.into())
            .and_modify(|cache| {
                cache.intrinsic.replace(intrinsic);
            })
            .or_insert(MeasureCache {
                intrinsic: intrinsic.into(),
                ..Default::default()
            });
    }
}

impl<Id> LoadIntrinsic<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Intrinsic<f32>> {
        self.measure_cache
            .get(&id.into())
            .and_then(|cache| cache.intrinsic)
    }
}

impl<Id> SaveExtent<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, extent: Extent<f32>) {
        self.measure_cache
            .entry(id.into())
            .and_modify(|cache| {
                cache.extent.replace(extent);
            })
            .or_insert(MeasureCache {
                extent: extent.into(),
                ..Default::default()
            });
    }
}

impl<Id> LoadExtent<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Extent<f32>> {
        self.measure_cache
            .get(&id.into())
            .and_then(|cache| cache.extent)
    }
}

impl<Id> SaveConstraints<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, constraints: Constraints<Extent<f32>>) {
        self.measure_cache
            .entry(id.into())
            .and_modify(|cache| {
                cache.constraints.replace(constraints);
            })
            .or_insert(MeasureCache {
                constraints: constraints.into(),
                ..Default::default()
            });
    }
}

impl<Id> LoadConstraints<f32, Id> for Context
where
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Constraints<Extent<f32>>> {
        self.measure_cache
            .get(&id.into())
            .and_then(|cache| cache.constraints)
    }
}

impl MeasureContext<f32> for Context {
    fn widget_intrinsic(&mut self, widget_id: &WidgetId) -> Option<Intrinsic<f32>> {
        self.widget_forest
            .node_by_id_mut(widget_id)
            .map(|w| w.intrinsic(self))
    }

    fn measure_widget(
        &mut self,
        widget_id: &WidgetId,
        constraints: Constraints<Extent<f32>>,
    ) -> Option<Extent<f32>> {
        self.widget_forest
            .node_by_id_mut(widget_id)
            .map(|w| w.measure(self, constraints))
    }
}

impl LayoutContext<f32> for Context {
    fn layout_widget(&mut self, widget_id: &WidgetId, local_coord: Point<f32>) {
        if let Some(mut widget) = self.widget_forest.node_by_id_mut(widget_id) {
            widget.layout(self, local_coord);
        }
    }
}

impl DrawContext<f32> for Context {
    fn draw_widget(
        &self,
        widget_id: &WidgetId,
        offset: &crate::types::Offset<f32>,
        drawer: &mut crate::stage::draw::Drawer,
    ) {
        if let Some(widget) = self.widget_forest.node_by_id(widget_id) {
            widget.draw(self, offset, drawer)
        }
    }
}

impl EventContext<f32> for Context {
    fn hit_test_widget(
        &self,
        widget_id: &WidgetId,
        local_coords: crate::types::Point<f32>,
        router: &mut crate::events::EventRouter,
    ) -> Option<crate::events::HitTestResult> {
        self.widget_forest
            .node_by_id(widget_id)
            .map(|w| w.hit_test(self, local_coords, router))
    }

    fn route_events_to_widget(
        &mut self,
        widget_id: &WidgetId,
        router: &crate::events::EventRouter,
    ) {
        if let Some(mut widget) = self.widget_forest.node_by_id_mut(widget_id) {
            widget.route_events(self, router)
        }
    }
}

pub trait ManageAnimationRegistry<Id>
where
    Id: Into<WidgetId>,
{
    fn register_animation(&mut self, id: Id, animation_progress: AnimationProgress);
    fn remove_animation(&mut self, id: Id);
    fn reverse_animation(&mut self, id: Id);
}

pub trait AnimationQuery<Id>
where
    Id: Into<WidgetId>,
{
    fn animation_progress(&self, id: Id) -> Option<f32>;
    fn is_finished(&self, id: Id) -> bool;
}

impl<Id> ManageAnimationRegistry<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn register_animation(&mut self, id: Id, animation_progress: AnimationProgress) {
        self.animation_regirsty
            .insert(id.into(), animation_progress);
    }

    fn remove_animation(&mut self, id: Id) {
        self.animation_regirsty.remove(&id.into());
    }

    fn reverse_animation(&mut self, id: Id) {
        self.animation_regirsty
            .entry(id.into())
            .and_modify(AnimationProgress::reverse);
    }
}

impl<Id> AnimationQuery<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn animation_progress(&self, id: Id) -> Option<f32> {
        self.animation_regirsty
            .get(&id.into())
            .map(AnimationProgress::progress)
    }

    fn is_finished(&self, id: Id) -> bool {
        self.animation_regirsty
            .get(&id.into())
            .map(AnimationProgress::is_finished)
            .unwrap_or(true)
    }
}

pub trait ClearWidgetResources<Id>
where
    Id: Into<WidgetId>,
{
    /// Clears safely all runtime resources and information of a widget.
    fn clear_widget_resources(&mut self, id: Id);
}

impl<Id> ClearWidgetResources<Id> for Context
where
    Id: Into<WidgetId>,
{
    fn clear_widget_resources(&mut self, id: Id) {
        let widget_id = id.into();
        self.widget_data_registry.remove(&widget_id);
        self.measure_cache.remove(&widget_id);
        self.dirty_registry.remove(&widget_id);
        self.animation_regirsty.remove(&widget_id);
    }
}

fn collect_input_regions<C>(
    current_node: WidgetId,
    context: &C,
    input_regions: &mut Vec<(Point<f32>, Extent<f32>)>,
) where
    C: WidgetInformationContext + LoadLocalCoord<f32, WidgetId> + LoadExtent<f32, WidgetId>,
{
    if let Some(widget) = context.widget_by(current_node) {
        match widget.input_behavior() {
            crate::types::InputBehavior::Accepts => {
                input_regions.push((
                    <C as LoadLocalCoord<f32, WidgetId>>::load(context, current_node)
                        .unwrap_or_default(),
                    <C as LoadExtent<f32, WidgetId>>::load(context, current_node)
                        .unwrap_or_default(),
                ));
            }
            crate::types::InputBehavior::PassesToChildren => {
                for child_widget_id in context.childrens_identifiers_of(current_node) {
                    collect_input_regions(child_widget_id, context, input_regions);
                }
            }
        }
    }
}
