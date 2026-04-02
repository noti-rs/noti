use std::collections::HashMap;

use crate::types::{
    measure::Intrinsic, Extent, WidgetClass, WidgetData, WidgetDependency, WidgetId, WidgetStyle,
};

pub struct Context {
    id_counter: u64,
    dependendices: HashMap<WidgetClass, WidgetDependency>,
    measure_cache: HashMap<WidgetId, MeasureCache>,
    font_collection: skia_safe::textlayout::FontCollection,
}

impl Context {
    pub fn new(font_collection: skia_safe::textlayout::FontCollection) -> Self {
        Self {
            id_counter: 1,
            font_collection,
            dependendices: HashMap::new(),
            measure_cache: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct MeasureCache {
    intrinsic: Option<Intrinsic<f32>>,
    extent: Option<Extent<f32>>,
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
                if let Some(data) = dependency.data.clone() {
                    existing_dependency.data = Some(data);
                }

                if let Some(style) = dependency.style.clone() {
                    existing_dependency.style = Some(style);
                }
            })
            .or_insert_with(|| dependency);
    }
}

pub trait GetData {
    fn get_data(&self, class: &WidgetClass) -> Option<&WidgetData>;
}

impl GetData for Context {
    fn get_data(&self, class: &WidgetClass) -> Option<&WidgetData> {
        self.dependendices
            .get(class)
            .and_then(|dependency| dependency.data.as_ref())
    }
}

pub trait GetStyle {
    fn get_style(&self, class: &WidgetClass) -> Option<&WidgetStyle>;
}

impl GetStyle for Context {
    fn get_style(&self, class: &WidgetClass) -> Option<&WidgetStyle> {
        self.dependendices
            .get(class)
            .and_then(|dependency| dependency.style.as_ref())
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
