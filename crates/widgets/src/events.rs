#[derive(Debug, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub local_coord: Point,
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

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

pub enum Action {
    ButtonPressed { button_id: String },
    RequestMenu { menu_id: String },
    OpenLink { link: String },
    None,
}

pub trait DispatchEvent {
    fn dispatch_event(&self, event: Event) -> Action;
}
