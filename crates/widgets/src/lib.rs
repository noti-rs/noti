pub mod context;
pub mod drawer;
pub mod events;
pub mod presence;
pub mod state;
pub mod types;
pub mod widget;

use crate::{
    context::{Context, CreateState, GetState, InjectDependency, LoadExtent, SetState, Tick},
    types::{
        measure::{Constraints, Measure},
        Extent, WidgetId,
    },
    widget::{Draw, Initialize, Layout, Widget, WidgetInfo},
};

pub struct UiRoot {
    root_widget: Widget,
    context: Context,
}

impl UiRoot {
    pub fn new(mut root_widget: Widget, mut context: Context) -> Self {
        root_widget.initialize(&mut context);

        Self {
            root_widget,
            context,
        }
    }

    pub fn width(&self) -> f32 {
        <Context as LoadExtent<f32, WidgetId>>::load(&self.context, self.root_widget.get_id())
            .unwrap_or_default()
            .width
    }

    pub fn height(&self) -> f32 {
        <Context as LoadExtent<f32, WidgetId>>::load(&self.context, self.root_widget.get_id())
            .unwrap_or_default()
            .height
    }

    pub fn layout(&mut self, constraints: Constraints<Extent<f32>>) {
        self.root_widget.measure(&mut self.context, constraints);
        self.root_widget.layout(&self.context);
    }

    pub fn draw(&self, offset: &types::Offset<f32>, drawer: &mut drawer::Drawer) {
        self.root_widget.draw(&self.context, offset, drawer);
    }
}

impl<T> CreateState<T> for UiRoot {
    fn create_state(&mut self, value: T) -> state::State<T> {
        self.context.create_state(value)
    }

    fn create_state_mut(&mut self, value: T) -> state::MutableState<T> {
        self.context.create_state_mut(value)
    }
}

impl GetState for UiRoot {
    fn get<T, S>(&self, state: S) -> Option<&T>
    where
        T: 'static,
        S: Into<state::State<T>>,
    {
        self.context.get(state)
    }
}

impl SetState for UiRoot {
    fn set<T: 'static>(&mut self, mutable_state: state::MutableState<T>, value: T) {
        self.context.set(mutable_state, value);
    }
}

impl InjectDependency for UiRoot {
    fn inject<Class, Dependency>(&mut self, class: Class, dependency: Dependency)
    where
        Class: Into<types::WidgetClass>,
        Dependency: Into<types::WidgetDependency>,
    {
        self.context.inject(class, dependency);
    }
}

impl Tick for UiRoot {
    fn tick(&mut self, delta_ns: u64) {
        self.context.tick(delta_ns);
    }
}
