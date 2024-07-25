use std::{fs::File, io::Write, os::fd::AsFd, sync::Arc};

use crate::{
    config::{self, CONFIG},
    data::{
        aliases::Result,
        internal_messages::RendererMessage,
        notification::{self, Notification},
    },
};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_compositor,
        wl_pointer::{self, ButtonState},
        wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, Anchor},
};

use super::{banner::BannerRect, font::FontCollection};

pub(crate) struct WindowManager {
    connection: Connection,
    event_queue: Option<EventQueue<Window>>,
    qhandle: Option<QueueHandle<Window>>,
    window: Option<Window>,

    font_collection: Arc<FontCollection>,

    banner_stack: Vec<BannerRect>,
    events: Vec<RendererMessage>,
}

impl WindowManager {
    pub(crate) fn init() -> Result<Self> {
        let connection = Connection::connect_to_env()?;
        let font_collection = Arc::new(FontCollection::load_by_font_name(
            CONFIG.general().font().name(),
        )?);

        Ok(Self {
            connection,
            event_queue: None,
            qhandle: None,
            window: None,

            font_collection,

            banner_stack: vec![],
            events: vec![],
        })
    }

    pub(crate) fn create_notifications(&mut self, mut notifications: Vec<Notification>) {
        let init = self.init_window();

        self.replace_by_indices(&mut notifications);

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

        let rects: Vec<BannerRect> = notifications
            .into_iter()
            .map(|notification| {
                let mut banner_rect = BannerRect::init(notification);
                banner_rect.set_font_collection(self.font_collection.clone());
                banner_rect.draw();
                banner_rect
            })
            .collect();

        let gap = CONFIG.general().gap();
        let height = CONFIG.general().height() as i32;

        if init {
            window.height = rects.len() as i32 * height as i32
                + rects.len().saturating_sub(1) as i32 * gap as i32;
        } else {
            window.height += rects.len() as i32 * (height as i32 + gap as i32);
        }

        self.banner_stack.extend(rects.into_iter());
        self.banner_stack
            .sort_by(CONFIG.general().sorting().get_cmp::<BannerRect>());

        let mut file = tempfile::tempfile().unwrap();
        let gap_buffer = Self::allocate_gap_buffer(window.width, gap);

        Self::write_stack_to_file(
            &self.banner_stack,
            CONFIG.general().anchor(),
            &gap_buffer,
            &mut file,
        );

        window.create_buffer(file, qhandle);
        window.resize_layer_surface();

        self.full_commit();
    }

    pub(crate) fn close_notifications(&mut self, indices: &[u32]) {
        let indices_to_remove: Vec<usize> = self
            .banner_stack
            .iter()
            .enumerate()
            .filter(|(_, banner_rect)| indices.contains(&banner_rect.notification().id))
            .map(|(i, _)| i)
            .rev()
            .collect();

        if indices_to_remove.is_empty() {
            return;
        }

        self.remove_rects(&indices_to_remove);
        indices.iter().for_each(|&id| {
            self.events.push(RendererMessage::ClosedNotification {
                id,
                reason: crate::data::dbus::ClosingReason::CallCloseNotification,
            })
        })
    }

    pub(crate) fn remove_expired(&mut self) {
        if self.banner_stack.is_empty() {
            return;
        }

        let indices_to_remove: Vec<usize> = self
            .banner_stack
            .iter()
            .enumerate()
            .filter_map(|(i, rect)| match &rect.notification().expire_timeout {
                notification::Timeout::Millis(millis) => {
                    if rect.created_at().elapsed().as_millis() > *millis as u128 {
                        Some(i)
                    } else {
                        None
                    }
                }
                notification::Timeout::Never => None,
                notification::Timeout::Configurable => {
                    let timeout = CONFIG
                        .display_by_app(&rect.notification().app_name)
                        .timeout();
                    if timeout != 0 && rect.created_at().elapsed().as_millis() > timeout as u128 {
                        Some(i)
                    } else {
                        None
                    }
                }
            })
            .rev()
            .collect();

        if indices_to_remove.is_empty() {
            return;
        }

        let notifications = self.remove_rects(&indices_to_remove);
        notifications.into_iter().for_each(|notification| {
            self.events.push(RendererMessage::ClosedNotification {
                id: notification.id,
                reason: crate::data::dbus::ClosingReason::Expired,
            })
        })
    }

    pub(crate) fn pop_event(&mut self) -> Option<RendererMessage> {
        self.events.pop()
    }

    pub(crate) fn handle_actions(&mut self) {
        //TODO: change it to actions

        if let Some(window) = self.window.as_mut() {
            if window.pointer_state.lb_pressed {
                window.pointer_state.lb_pressed = false;

                let rect_height = CONFIG.general().height() as usize;
                let gap = CONFIG.general().gap() as usize;
                let anchor = CONFIG.general().anchor();

                if let Some(i) = (0..window.height as usize)
                    .step_by(rect_height + gap)
                    .enumerate()
                    .take(self.banner_stack.len())
                    .find(|&(_, rect_top)| {
                        let rect_bottom = rect_top + rect_height;
                        (rect_top..rect_bottom).contains(&(window.pointer_state.y as usize))
                    })
                    .map(|(i, _)| {
                        if anchor.is_top() {
                            self.banner_stack.len() - i - 1
                        } else {
                            i
                        }
                    })
                {
                    let notifications = self.remove_rects(&[i]);
                    notifications.into_iter().for_each(|notification| {
                        self.events.push(RendererMessage::ClosedNotification {
                            id: notification.id,
                            reason: crate::data::dbus::ClosingReason::DismissedByUser,
                        })
                    })
                }
            }
        }
    }

    pub(crate) fn dispatch(&mut self) -> bool {
        if self.event_queue.is_none() {
            return false;
        }

        let event_queue = unsafe { self.event_queue.as_mut().unwrap_unchecked() };
        let window = unsafe { self.window.as_mut().unwrap_unchecked() };

        let dispatched_count = event_queue
            .dispatch_pending(window)
            .expect("Successful dispatch");

        if dispatched_count > 0 {
            return true;
        }

        event_queue.flush().expect("Successful event queue flush");
        let guard = event_queue.prepare_read().expect("Get read events guard");
        let Ok(count) = guard.read() else {
            return false;
        };

        if count > 0 {
            event_queue
                .dispatch_pending(window)
                .expect("Successful dispatch");
            true
        } else {
            false
        }
    }

    fn replace_by_indices(&mut self, notifications: &mut Vec<Notification>) {
        let matching_indices: Vec<(usize, usize)> = notifications
            .iter()
            .enumerate()
            .filter_map(|(i, notification)| {
                self.banner_stack
                    .iter()
                    .position(|rect| rect.notification().id == notification.id)
                    .map(|stack_index| (i, stack_index))
            })
            .collect();

        for (notification_index, stack_index) in matching_indices.into_iter().rev() {
            let notification = notifications.remove(notification_index);
            let rect = &mut self.banner_stack[stack_index];
            rect.update_data(notification);
            rect.draw();
        }
    }

    fn remove_rects(&mut self, indices_to_remove: &[usize]) -> Vec<Notification> {
        let notifications = indices_to_remove
            .iter()
            .map(|id| self.banner_stack.remove(*id).destroy_and_get_notification())
            .collect();

        if self.banner_stack.len() == 0 {
            self.deinit_window();
            return notifications;
        }

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        let gap = CONFIG.general().gap();
        window.height -=
            notifications.len() as i32 * (CONFIG.general().height() as i32 + gap as i32);

        let mut file = tempfile::tempfile().unwrap();
        let gap_buffer = Self::allocate_gap_buffer(window.width, gap);

        Self::write_stack_to_file(
            &self.banner_stack,
            CONFIG.general().anchor(),
            &gap_buffer,
            &mut file,
        );

        let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };
        window.create_buffer(file, qhandle);
        window.resize_layer_surface();

        self.full_commit();

        notifications
    }

    fn write_stack_to_file(
        stack: &[BannerRect],
        anchor: &config::Anchor,
        gap_buffer: &[u8],
        file: &mut File,
    ) {
        if anchor.is_top() {
            stack.iter().rev().enumerate().for_each(|(i, rect)| {
                rect.write_to_file(file);

                if i < stack.len().saturating_sub(1) {
                    file.write_all(gap_buffer).unwrap();
                }
            })
        } else {
            stack.iter().enumerate().for_each(|(i, rect)| {
                rect.write_to_file(file);

                if i < stack.len().saturating_sub(1) {
                    file.write_all(gap_buffer).unwrap();
                }
            })
        }
    }

    fn allocate_gap_buffer(window_width: i32, gap: u8) -> Vec<u8> {
        let rowstride = window_width as usize * 4;
        let gap_size = gap as usize * rowstride;
        vec![0; gap_size as usize]
    }

    fn full_commit(&mut self) {
        unsafe {
            self.event_queue
                .as_mut()
                .unwrap_unchecked()
                .roundtrip(self.window.as_mut().unwrap_unchecked())
                .unwrap();
        }
    }

    fn init_window(&mut self) -> bool {
        if let None = self.window {
            let mut event_queue = self.connection.new_event_queue();
            let qhandle = event_queue.handle();
            let display = self.connection.display();
            display.get_registry(&qhandle, ());

            let mut window = Window::init();
            while !window.configured {
                let _ = event_queue.blocking_dispatch(&mut window);
            }

            self.event_queue = Some(event_queue);
            self.qhandle = Some(qhandle);
            self.window = Some(window);
            true
        } else {
            false
        }
    }

    fn deinit_window(&mut self) {
        unsafe {
            let window = self.window.as_mut().unwrap_unchecked();
            window.surface.as_ref().unwrap_unchecked().destroy();
            self.event_queue
                .as_mut()
                .unwrap_unchecked()
                .roundtrip(window)
                .unwrap();
        }
        self.window = None;
        self.event_queue = None;
        self.qhandle = None;
    }
}

struct Window {
    width: i32,
    height: i32,
    margin: Margin,

    shm: Option<wl_shm::WlShm>,
    buffer: Option<wl_buffer::WlBuffer>,

    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,

    configured: bool,
    pointer_state: PointerState,
}

impl Window {
    fn init() -> Self {
        let width = CONFIG.general().width() as i32;
        let height = CONFIG.general().height() as i32;

        Self {
            width,
            height,
            margin: Margin::new(),

            shm: None,
            buffer: None,

            surface: None,
            layer_surface: None,

            configured: false,
            pointer_state: Default::default(),
        }
    }

    fn create_buffer(&mut self, file: File, qhandle: &QueueHandle<Window>) {
        let pool = self.shm.as_ref().unwrap().create_pool(
            file.as_fd(),
            self.width * self.height * 4,
            qhandle,
            (),
        );

        self.buffer = Some(pool.create_buffer(
            0,
            self.width,
            self.height,
            self.width * 4,
            wl_shm::Format::Argb8888,
            qhandle,
            (),
        ));
    }

    fn resize_layer_surface(&mut self) {
        let layer_surface = unsafe { self.layer_surface.as_ref().unwrap_unchecked() };
        layer_surface.set_size(self.width as u32, self.height as u32);
        let surface = unsafe { self.surface.as_ref().unwrap_unchecked() };
        surface.attach(self.buffer.as_ref(), 0, 0);
        surface.commit();
    }
}

struct Margin {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl Margin {
    fn new() -> Self {
        Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        }
    }

    fn apply(&self, layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1) {
        layer_surface.set_margin(self.top, self.right, self.bottom, self.left);
    }
}

#[derive(Default)]
struct PointerState {
    x: f64,
    y: f64,

    lb_pressed: bool,
    rb_pressed: bool,
    mb_pressed: bool,
}

impl PointerState {
    const LEFT_BTN: u32 = 272;
    const RIGHT_BTN: u32 = 273;
    const MIDDLE_BTN: u32 = 274;
}

impl Dispatch<wl_registry::WlRegistry, ()> for Window {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_ref() {
                "wl_compositor" => {
                    let compositor = registry.bind::<wl_compositor::WlCompositor, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    );
                    state.surface = Some(compositor.create_surface(qhandle, ()));
                }
                "wl_shm" => {
                    state.shm =
                        Some(registry.bind::<wl_shm::WlShm, _, _>(name, version, qhandle, ()))
                }
                "wl_seat" => {
                    registry.bind::<wl_seat::WlSeat, _, _>(name, version, qhandle, ());
                }
                "zwlr_layer_shell_v1" => {
                    let layer_shell = registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    );
                    if let Some(surface) = state.surface.as_ref() {
                        let layer_surface = layer_shell.get_layer_surface(
                            surface,
                            None,
                            zwlr_layer_shell_v1::Layer::Overlay,
                            "notification-layer".to_string(),
                            qhandle,
                            (),
                        );

                        let general_cfg = CONFIG.general();
                        let anchor_cfg = general_cfg.anchor();

                        let (x, y) = CONFIG.general().offset();

                        if anchor_cfg.is_top() {
                            state.margin.top = y as i32;
                        }
                        if anchor_cfg.is_bottom() {
                            state.margin.bottom = y as i32;
                        }
                        if anchor_cfg.is_left() {
                            state.margin.left = x as i32;
                        }
                        if anchor_cfg.is_right() {
                            state.margin.right = x as i32;
                        }

                        let anchor = match anchor_cfg {
                            config::Anchor::Top => Anchor::Top,
                            config::Anchor::TopLeft => Anchor::Top.union(Anchor::Left),
                            config::Anchor::TopRight => Anchor::Top.union(Anchor::Right),
                            config::Anchor::Bottom => Anchor::Bottom,
                            config::Anchor::BottomLeft => Anchor::Bottom.union(Anchor::Left),
                            config::Anchor::BottomRight => Anchor::Bottom.union(Anchor::Right),
                            config::Anchor::Left => Anchor::Left,
                            config::Anchor::Right => Anchor::Right,
                        };

                        layer_surface.set_anchor(anchor);
                        state.margin.apply(&layer_surface);

                        layer_surface.set_size(state.width as u32, state.height as u32);
                        layer_surface.set_keyboard_interactivity(
                            zwlr_layer_surface_v1::KeyboardInteractivity::None,
                        );
                        surface.commit();

                        state.layer_surface = Some(layer_surface);
                    }
                }
                _ => (),
            }
        }
    }
}

delegate_noop!(Window: ignore wl_compositor::WlCompositor);
delegate_noop!(Window: ignore wl_surface::WlSurface);
delegate_noop!(Window: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Window: ignore wl_shm::WlShm);
delegate_noop!(Window: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Window: ignore wl_buffer::WlBuffer);

impl Dispatch<wl_seat::WlSeat, ()> for Window {
    fn event(
        _state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capability),
        } = event
        {
            if capability.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qhandle, ());
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Window {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            } => {
                (state.pointer_state.x, state.pointer_state.y) = (surface_x, surface_y);
            }
            wl_pointer::Event::Leave { .. } => (),
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                (state.pointer_state.x, state.pointer_state.y) = (surface_x, surface_y);
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(ButtonState::Pressed),
                ..
            } => match button {
                PointerState::LEFT_BTN => {
                    state.pointer_state.lb_pressed = true;
                }
                PointerState::RIGHT_BTN => {
                    state.pointer_state.rb_pressed = true;
                }
                PointerState::MIDDLE_BTN => {
                    state.pointer_state.mb_pressed = true;
                }
                _ => (),
            },
            _ => (),
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Window {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: <zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            layer_surface.ack_configure(serial);

            if width == 0 && height == 0 {
                let general_cfg = CONFIG.general();
                state.width = general_cfg.width() as i32;
                state.height = general_cfg.height() as i32;
            } else {
                state.width = width as i32;
                state.height = height as i32;
            }

            state.configured = true;
        }
    }
}
