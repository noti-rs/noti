use crate::{
    state::{MutableState, State, StateInfo},
    types::{
        dirty_flags::DirtyFlags,
        identifiers::WidgetKey,
        measure::{Constraints, Intrinsic},
        Extent, WidgetClass, WidgetDependency, WidgetId, WidgetStyle,
    },
};
use std::collections::HashMap;

pub struct Context {
    id_counter: u64,
    state_descriptor_counter: usize,
    state_registry: HashMap<usize, StateInfo>,
    key_registry: HashMap<WidgetKey, WidgetId>,
    dependendices: HashMap<WidgetClass, WidgetDependency>,
    measure_cache: HashMap<WidgetId, MeasureCache>,
    dirty_registry: HashMap<WidgetId, DirtyFlags>,
    font_collection: skia_safe::textlayout::FontCollection,
}

impl Context {
    pub fn new(font_collection: skia_safe::textlayout::FontCollection) -> Self {
        Self {
            id_counter: 1,
            state_descriptor_counter: 1,
            state_registry: HashMap::new(),
            font_collection,
            key_registry: HashMap::new(),
            dependendices: HashMap::new(),
            measure_cache: HashMap::new(),
            dirty_registry: HashMap::new(),
        }
    }

    fn register_state(&mut self, state_info: StateInfo) -> usize {
        let descriptor = self.state_descriptor_counter;
        self.state_descriptor_counter += 1;
        self.state_registry.insert(descriptor, state_info);
        descriptor
    }
}

#[derive(Debug, Clone, Default)]
struct MeasureCache {
    intrinsic: Option<Intrinsic<f32>>,
    extent: Option<Extent<f32>>,
    constraints: Option<Constraints<Extent<f32>>>,
}

pub trait CreateState<T> {
    fn create_state(&mut self, value: T) -> State<T>;
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

pub trait GetState {
    fn get<T: 'static, S: Into<State<T>>>(&self, state: S) -> Option<&T>;
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

pub trait SetState {
    fn set<T: 'static>(&mut self, mutable_state: MutableState<T>, value: T);
}

impl SetState for Context {
    fn set<T: 'static>(&mut self, mutable_state: MutableState<T>, value: T) {
        if let Some(state_info) = self.state_registry.get_mut(&mutable_state.descriptor) {
            if let Some(data) = state_info.get_data_mut(mutable_state) {
                *data = value;

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

pub(crate) trait Subscribe<Id>
where
    Id: Into<WidgetId>,
{
    fn subscribe<S, T>(&mut self, id: Id, state: S)
    where
        S: Into<State<T>>,
        T: 'static;
}

impl<Id> Subscribe<Id> for Context
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
}

pub trait InjectDependency {
    fn inject<Class, Dependency>(&mut self, class: Class, dependency: Dependency)
    where
        Class: Into<WidgetClass>,
        Dependency: Into<WidgetDependency>;
}

impl InjectDependency for Context {
    fn inject<Class, Dependency>(&mut self, class: Class, dependency: Dependency)
    where
        Class: Into<WidgetClass>,
        Dependency: Into<WidgetDependency>,
    {
        let dependency = dependency.into();
        self.dependendices
            .entry(class.into())
            .and_modify(|existing_dependency| {
                if let Some(style) = dependency.style.clone() {
                    existing_dependency.style = Some(style);
                }
            })
            .or_insert_with(|| dependency);
    }
}

pub(crate) trait Tick {
    fn tick(&mut self, delta_ns: u64);
}

impl Tick for Context {
    fn tick(&mut self, delta_ns: u64) {
        // for (widget_id, presence_state) in &mut self.presence_registry {
        //     presence_state.update(delta_ns);
        //
        //     self.dirty_registry
        //         .entry(*widget_id)
        //         .and_modify(|flags| *flags |= DirtyFlags::NEEDS_MEASURE)
        //         .or_insert(DirtyFlags::NEEDS_MEASURE);
        // }
    }
}

pub(crate) trait GenerateId {
    fn generate_id(&mut self) -> WidgetId;
}

impl GenerateId for Context {
    fn generate_id(&mut self) -> WidgetId {
        let id = self.id_counter;
        self.id_counter += 1;
        id.into()
    }
}

pub(crate) trait RegisterKey<K, Id>
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

pub(crate) trait ManageDirtyFlags<Id>
where
    Id: Into<WidgetId>,
{
    fn get_dirty_flags(&self, id: Id) -> DirtyFlags;
    fn set_dirty_flags(&mut self, id: Id, dirty_flags: DirtyFlags);
    fn remove_dirty_flags(&mut self, id: Id);
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
        self.dirty_registry.insert(id.into(), dirty_flags);
    }

    fn remove_dirty_flags(&mut self, id: Id) {
        self.dirty_registry.remove(&id.into());
    }
}

pub(crate) trait GetStyle {
    fn get_style(&self, class: &WidgetClass) -> Option<&WidgetStyle>;
}

impl GetStyle for Context {
    fn get_style(&self, class: &WidgetClass) -> Option<&WidgetStyle> {
        self.dependendices
            .get(class)
            .and_then(|dependency| dependency.style.as_ref())
    }
}

pub(crate) trait GetFont {
    fn get_font(&self) -> skia_safe::textlayout::FontCollection;
}

impl GetFont for Context {
    fn get_font(&self) -> skia_safe::textlayout::FontCollection {
        self.font_collection.clone()
    }
}

pub(crate) trait SaveIntrinsic<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, intrinsic: Intrinsic<T>);
}

pub(crate) trait LoadIntrinsic<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Intrinsic<T>>;
}

pub(crate) trait ManageIntrinsic<T, Id>:
    SaveIntrinsic<T, Id> + LoadIntrinsic<T, Id>
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

pub(crate) trait SaveExtent<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, extent: Extent<T>);
}

pub(crate) trait LoadExtent<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Extent<T>>;
}

pub(crate) trait ManageExtent<T, Id>: SaveExtent<T, Id> + LoadExtent<T, Id>
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

pub(crate) trait SaveConstraints<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn save(&mut self, id: Id, constraints: Constraints<Extent<T>>);
}

pub(crate) trait LoadConstraints<T, Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
    fn load(&self, id: Id) -> Option<Constraints<Extent<T>>>;
}

pub(crate) trait ManageConstraints<T, Id>:
    SaveConstraints<T, Id> + LoadConstraints<T, Id>
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
