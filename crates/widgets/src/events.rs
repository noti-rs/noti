use crate::{
    context::{LoadExtent, ScopedContext, ScopedManageState},
    types::{Point, WidgetId},
};

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

pub trait FunctionCallback<'a, T, R>: FnMut(ScopedContext<'a>, T) -> R {}
impl<'a, T, R, F> FunctionCallback<'a, T, R> for F where F: FnMut(ScopedContext<'a>, T) -> R {}

pub type Callback<T, R> = Box<dyn for<'a> FunctionCallback<'a, T, R>>;

pub(crate) trait DispatchContext<T>: LoadExtent<T, WidgetId> + ScopedManageState
where
    T: Default + Copy,
{
}

impl<C, T> DispatchContext<T> for C
where
    C: LoadExtent<T, WidgetId> + ScopedManageState,
    T: Default + Copy,
{
}

pub(crate) trait DispatchEvent<C, T>
where
    C: DispatchContext<T>,
    T: Default + Copy,
{
    fn dispatch_event(&mut self, context: &mut C, event: Event);
}
