use std::{
    fs::File,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::FileExt,
    },
    sync::Arc,
};

use crate::{
    config::{self, CONFIG},
    data::{
        aliases::Result,
        internal_messages::RendererMessage,
        notification::{self, Notification},
    },
};
use itertools::Itertools;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_callback, wl_compositor,
        wl_pointer::{self, ButtonState},
        wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, Anchor},
};

use super::{banner::BannerRect, font::FontCollection, types::RectSize};

pub(crate) struct WindowManager {
    connection: Connection,
    event_queue: Option<EventQueue<Window>>,
    qhandle: Option<QueueHandle<Window>>,
    window: Option<Window>,

    font_collection: Arc<FontCollection>,

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

            events: vec![],
        })
    }

    pub(crate) fn create_notifications(&mut self, notifications: Vec<Notification>) {
        let _ = self.init_window();

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        window.update_banners(notifications);

        self.update_window();
        self.roundtrip_event_queue();
    }

    pub(crate) fn close_notifications(&mut self, indices: &[u32]) {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_banners_by_id(indices);

            if notifications.is_empty() {
                return;
            }

            notifications
                .into_iter()
                .map(|notification| notification.id)
                .for_each(|id| {
                    self.events.push(RendererMessage::ClosedNotification {
                        id,
                        reason: crate::data::dbus::ClosingReason::CallCloseNotification,
                    })
                });

            self.update_window();
            self.roundtrip_event_queue();
        }
    }

    pub(crate) fn remove_expired(&mut self) {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_expired_banners();

            if notifications.is_empty() {
                return;
            }

            notifications.into_iter().for_each(|notification| {
                self.events.push(RendererMessage::ClosedNotification {
                    id: notification.id,
                    reason: crate::data::dbus::ClosingReason::Expired,
                })
            });

            self.update_window();
            self.roundtrip_event_queue();
        }
    }

    pub(crate) fn pop_event(&mut self) -> Option<RendererMessage> {
        self.events.pop()
    }

    pub(crate) fn handle_actions(&mut self) {
        //TODO: change it to actions which defines in config file

        if let Some(window) = self.window.as_mut() {
            let messages = window.handle_click();
            if messages.is_empty() {
                return;
            }

            self.events.extend(messages);

            self.update_window();
            self.roundtrip_event_queue();
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

    fn update_window(&mut self) {
        if let Some(window) = self.window.as_mut() {
            if window.is_empty() {
                self.deinit_window();
                return;
            }

            let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

            window.draw(qhandle);
            window.frame(qhandle);
            window.commit();
        }
    }

    fn roundtrip_event_queue(&mut self) {
        if let Some(event_queue) = self.event_queue.as_mut() {
            event_queue
                .roundtrip(unsafe { self.window.as_mut().unwrap_unchecked() })
                .unwrap();
        }
    }

    fn init_window(&mut self) -> bool {
        if let None = self.window {
            let mut event_queue = self.connection.new_event_queue();
            let qhandle = event_queue.handle();
            let display = self.connection.display();
            display.get_registry(&qhandle, ());

            let mut window = Window::init(self.font_collection.clone());
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
            window.deinit();
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
    banners: Vec<BannerRect>,
    font_collection: Arc<FontCollection>,

    rect_size: RectSize,
    margin: Margin,

    compositor: Option<wl_compositor::WlCompositor>,
    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,

    shm: Option<wl_shm::WlShm>,
    shm_pool: Option<wl_shm_pool::WlShmPool>,
    buffer: Option<Buffer>,
    wl_buffer: Option<wl_buffer::WlBuffer>,

    configured: bool,
    pointer_state: PointerState,
}

impl Window {
    fn init(font_collection: Arc<FontCollection>) -> Self {
        Self {
            banners: vec![],
            font_collection,

            rect_size: RectSize::new(
                CONFIG.general().width().into(),
                CONFIG.general().height().into(),
            ),
            margin: Margin::new(),

            compositor: None,
            surface: None,
            layer_surface: None,

            shm: None,
            shm_pool: None,
            buffer: None,
            wl_buffer: None,

            configured: false,
            pointer_state: Default::default(),
        }
    }

    fn deinit(&self) {
        if let Some(layer_surface) = self.layer_surface.as_ref() {
            layer_surface.destroy();
        }

        if let Some(surface) = self.surface.as_ref() {
            surface.destroy();
        }

        if let Some(buffer) = self.wl_buffer.as_ref() {
            buffer.destroy();
        }

        if let Some(shm_pool) = self.shm_pool.as_ref() {
            shm_pool.destroy()
        }
    }

    fn is_empty(&self) -> bool {
        self.banners.is_empty()
    }

    fn update_banners(&mut self, mut notifications: Vec<Notification>) {
        self.replace_by_indices(&mut notifications);

        self.banners
            .extend(notifications.into_iter().map(|notification| {
                let mut banner_rect = BannerRect::init(notification);
                banner_rect.draw(&self.font_collection);
                banner_rect
            }));
        self.banners
            .sort_by(CONFIG.general().sorting().get_cmp::<BannerRect>());
    }

    fn replace_by_indices(&mut self, notifications: &mut Vec<Notification>) {
        let matching_indices: Vec<(usize, usize)> = notifications
            .iter()
            .enumerate()
            .filter_map(|(i, notification)| {
                self.banners
                    .iter()
                    .position(|rect| rect.notification().id == notification.id)
                    .map(|stack_index| (i, stack_index))
            })
            .collect();

        for (notification_index, stack_index) in matching_indices.into_iter().rev() {
            let notification = notifications.remove(notification_index);
            let rect = &mut self.banners[stack_index];
            rect.update_data(notification);
            rect.draw(&self.font_collection);
        }
    }

    fn remove_banners(&mut self, indices_to_remove: &[usize]) -> Vec<Notification> {
        indices_to_remove
            .iter()
            .sorted()
            .rev()
            .map(|id| self.banners.remove(*id).destroy_and_get_notification())
            .collect()
    }

    fn remove_banners_by_id(&mut self, notification_indices: &[u32]) -> Vec<Notification> {
        let indices_to_remove: Vec<usize> = self
            .banners
            .iter()
            .enumerate()
            .filter_map(|(i, banner_rect)| {
                notification_indices
                    .contains(&banner_rect.notification().id)
                    .then(|| i)
            })
            .collect();

        if indices_to_remove.is_empty() {
            vec![]
        } else {
            self.remove_banners(&indices_to_remove)
        }
    }

    fn remove_expired_banners(&mut self) -> Vec<Notification> {
        let indices_to_remove: Vec<usize> = self
            .banners
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
            .collect();

        if indices_to_remove.is_empty() {
            vec![]
        } else {
            self.remove_banners(&indices_to_remove)
        }
    }

    fn handle_click(&mut self) -> Vec<RendererMessage> {
        if let PrioritiedPressState::Unpressed = self.pointer_state.press_state {
            return vec![];
        }
        self.pointer_state.press_state.clear();

        let rect_height = CONFIG.general().height() as usize;
        let gap = CONFIG.general().gap() as usize;
        let anchor = CONFIG.general().anchor();

        if let Some(i) = (0..self.rect_size.height as usize)
            .step_by(rect_height + gap)
            .enumerate()
            .take(self.banners.len())
            .find(|&(_, rect_top)| {
                let rect_bottom = rect_top + rect_height;
                (rect_top as f64..rect_bottom as f64).contains(&(self.pointer_state.y))
            })
            .map(|(i, _)| {
                if anchor.is_top() {
                    self.banners.len() - i - 1
                } else {
                    i
                }
            })
        {
            if anchor.is_bottom() {
                self.pointer_state.y -=
                    CONFIG.general().height() as f64 + CONFIG.general().gap() as f64;
            }

            return self
                .remove_banners(&[i])
                .into_iter()
                .map(|notification| RendererMessage::ClosedNotification {
                    id: notification.id,
                    reason: crate::data::dbus::ClosingReason::DismissedByUser,
                })
                .collect();
        }

        vec![]
    }

    fn draw(&mut self, qhandle: &QueueHandle<Window>) {
        let gap = CONFIG.general().gap();

        self.resize(RectSize::new(
            self.rect_size.width,
            self.banners.len() * CONFIG.general().height() as usize
                + self.banners.len().saturating_sub(1) * gap as usize,
        ));

        let gap_buffer = self.allocate_gap_buffer(gap);

        self.create_buffer(qhandle);
        self.write_banners_to_buffer(CONFIG.general().anchor(), &gap_buffer);
        self.build_buffer(qhandle);
    }

    fn resize(&mut self, rect_size: RectSize) {
        self.rect_size = rect_size;

        let layer_surface = unsafe { self.layer_surface.as_ref().unwrap_unchecked() };
        layer_surface.set_size(self.rect_size.width as u32, self.rect_size.height as u32);
    }

    fn allocate_gap_buffer(&self, gap: u8) -> Vec<u8> {
        let rowstride = self.rect_size.width as usize * 4;
        let gap_size = gap as usize * rowstride;
        vec![0; gap_size as usize]
    }

    fn write_banners_to_buffer(&mut self, anchor: &config::Anchor, gap_buffer: &[u8]) {
        fn write(buffer: Option<&mut Buffer>, data: &[u8]) {
            unsafe { buffer.unwrap_unchecked() }.push(data);
        }

        let writer = |(i, rect): (usize, &BannerRect)| {
            write(self.buffer.as_mut(), rect.framebuffer());

            if i < self.banners.len().saturating_sub(1) {
                write(self.buffer.as_mut(), gap_buffer);
            }
        };

        if anchor.is_top() {
            self.banners.iter().rev().enumerate().for_each(writer)
        } else {
            self.banners.iter().enumerate().for_each(writer)
        }
    }

    fn create_buffer(&mut self, qhandle: &QueueHandle<Window>) {
        if self.buffer.is_some() {
            let buffer = unsafe { self.buffer.as_mut().unwrap_unchecked() };
            buffer.reset();
            return;
        }

        let buffer = Buffer::new();

        if let None = self.shm_pool {
            self.shm_pool = Some(self.shm.as_ref().unwrap().create_pool(
                buffer.as_fd(),
                self.rect_size.area() as i32 * 4,
                qhandle,
                (),
            ));
        }

        self.buffer = Some(buffer);
    }

    fn build_buffer(&mut self, qhandle: &QueueHandle<Window>) {
        if let Some(wl_buffer) = self.wl_buffer.as_ref() {
            wl_buffer.destroy();
        }

        assert!(
            self.shm_pool.is_some() && self.buffer.is_some(),
            "The buffer must be created before build!"
        );

        assert!(
            self.buffer
                .as_ref()
                .is_some_and(|buffer| buffer.size() >= self.rect_size.area() * 4),
            "Buffer size must be greater or equal to window size!"
        );

        let shm_pool = unsafe { self.shm_pool.as_ref().unwrap_unchecked() };
        //INFO: The Buffer size only growth and it guarantee that shm_pool never shrinks
        shm_pool
            .resize(unsafe { self.buffer.as_ref().map(Buffer::size).unwrap_unchecked() } as i32);

        self.wl_buffer = Some(shm_pool.create_buffer(
            0,
            self.rect_size.width as i32,
            self.rect_size.height as i32,
            self.rect_size.width as i32 * 4,
            wl_shm::Format::Argb8888,
            qhandle,
            (),
        ));
    }

    fn frame(&self, qhandle: &QueueHandle<Window>) {
        let surface = unsafe { self.surface.as_ref().unwrap_unchecked() };
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.frame(qhandle, ());
        surface.attach(self.wl_buffer.as_ref(), 0, 0);
    }

    fn commit(&self) {
        let surface = unsafe { self.surface.as_ref().unwrap_unchecked() };
        surface.commit();
    }
}

struct Buffer {
    file: File,
    cursor: u64,
    size: usize,
}

impl Buffer {
    fn new() -> Self {
        Self {
            file: tempfile::tempfile().expect("The tempfile must be created"),
            cursor: 0,
            size: 0,
        }
    }

    fn reset(&mut self) {
        self.cursor = 0;
    }

    fn push(&mut self, data: &[u8]) {
        self.file
            .write_all_at(data, self.cursor)
            .expect("Must be possibility to write into file!");
        self.cursor += data.len() as u64;

        self.size = std::cmp::max(self.size, self.cursor as usize);
    }

    fn size(&self) -> usize {
        self.size
    }

    fn as_fd(&self) -> BorrowedFd {
        self.file.as_fd()
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

    fn from_anchor(x: i32, y: i32, anchor: &config::Anchor) -> Self {
        let mut margin = Margin::new();

        if anchor.is_top() {
            margin.top = y;
        }
        if anchor.is_bottom() {
            margin.bottom = y;
        }
        if anchor.is_left() {
            margin.left = x;
        }
        if anchor.is_right() {
            margin.right = x;
        }

        margin
    }

    fn apply(&self, layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1) {
        layer_surface.set_margin(self.top, self.right, self.bottom, self.left);
    }
}

#[derive(Default)]
struct PointerState {
    x: f64,
    y: f64,

    press_state: PrioritiedPressState,
}

/// Mouse button press state which have priority (LMB > RMB > MMB) if any is set at least,
/// otherwise sets the 'unpressed' state.
#[derive(Default)]
enum PrioritiedPressState {
    #[default]
    Unpressed,
    LMB,
    RMB,
    MMB,
}

impl PrioritiedPressState {
    fn update(&mut self, new_state: PrioritiedPressState) {
        match self {
            PrioritiedPressState::LMB => (),
            PrioritiedPressState::RMB => match &new_state {
                PrioritiedPressState::LMB => *self = new_state,
                _ => (),
            },
            PrioritiedPressState::MMB => match &new_state {
                PrioritiedPressState::LMB | PrioritiedPressState::RMB => *self = new_state,
                _ => (),
            },
            PrioritiedPressState::Unpressed => *self = new_state,
        }
    }

    fn clear(&mut self) {
        *self = PrioritiedPressState::Unpressed;
    }
}

impl PointerState {
    const LEFT_BTN: u32 = 272;
    const RIGHT_BTN: u32 = 273;
    const MIDDLE_BTN: u32 = 274;

    fn relocate(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }

    fn press(&mut self, button: u32) {
        match button {
            PointerState::LEFT_BTN => self.press_state.update(PrioritiedPressState::LMB),
            PointerState::RIGHT_BTN => self.press_state.update(PrioritiedPressState::RMB),
            PointerState::MIDDLE_BTN => self.press_state.update(PrioritiedPressState::MMB),
            _ => (),
        }
    }
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
                    state.compositor = Some(registry.bind::<wl_compositor::WlCompositor, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
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

                    let surface = state.compositor.as_ref()
                        .expect(
                            "The wl_compositor protocol must be before than the zwlr-layer-shell-v1 protocol.\
                            If it is not correct, please contact to developers with this information"
                        ).create_surface(qhandle, ());

                    let layer_surface = layer_shell.get_layer_surface(
                        &surface,
                        None,
                        zwlr_layer_shell_v1::Layer::Overlay,
                        "noti-app".to_string(),
                        qhandle,
                        (),
                    );

                    let general_cfg = CONFIG.general();
                    let anchor_cfg = general_cfg.anchor();

                    let (x, y) = CONFIG.general().offset();
                    state.margin = Margin::from_anchor(x as i32, y as i32, anchor_cfg);

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

                    layer_surface
                        .set_size(state.rect_size.width as u32, state.rect_size.height as u32);
                    layer_surface.set_keyboard_interactivity(
                        zwlr_layer_surface_v1::KeyboardInteractivity::None,
                    );
                    surface.commit();

                    state.surface = Some(surface);
                    state.layer_surface = Some(layer_surface);
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
delegate_noop!(Window: ignore wl_callback::WlCallback);

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
            } => state.pointer_state.relocate(surface_x, surface_y),
            wl_pointer::Event::Leave { .. } => (),
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => state.pointer_state.relocate(surface_x, surface_y),
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(ButtonState::Pressed),
                ..
            } => state.pointer_state.press(button),
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
                state.rect_size.width = general_cfg.width().into();
                state.rect_size.height = general_cfg.height().into();
            } else {
                state.rect_size.width = width as usize;
                state.rect_size.height = height as usize;
            }

            state.configured = true;
        }
    }
}
