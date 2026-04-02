pub mod animation;
pub mod context;
pub mod drawer;
pub mod events;
pub mod types;
pub mod widget;

use crate::{
    context::{Context, InjectDependency},
    types::{
        measure::{Constraints, Measure},
        Extent,
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
        self.root_widget.width()
    }

    pub fn height(&self) -> f32 {
        self.root_widget.height()
    }

    pub fn layout(&mut self, constraints: Constraints<Extent<f32>>) {
        self.root_widget.measure(&mut self.context, constraints);
        self.root_widget.layout(&self.context);
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

impl Draw for UiRoot {
    fn draw_with_offset(&self, offset: &types::Offset<f32>, drawer: &mut drawer::Drawer) {
        self.root_widget.draw_with_offset(offset, drawer);
    }
}
