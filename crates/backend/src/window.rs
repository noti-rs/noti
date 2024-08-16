use itertools::*;
use std::{
    fs::File,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::FileExt,
    },
    sync::Arc,
};

use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_callback, wl_compositor,
        wl_pointer::{self, ButtonState},
        wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Dispatch, QueueHandle, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, Anchor},
};

use super::internal_messages::RendererMessage;
use config::{self, Config};
use dbus::notification::{self, Notification};

use super::render::{BannerRect, FontCollection, RectSize};

pub(super) struct Window {
    banners: Vec<BannerRect>,
    font_collection: Arc<FontCollection>,

    rect_size: RectSize,
    margin: Margin,

    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,

    shm: Option<wl_shm::WlShm>,
    shm_pool: Option<wl_shm_pool::WlShmPool>,
    buffer: Option<Buffer>,
    wl_buffer: Option<wl_buffer::WlBuffer>,

    configuration_state: ConfigurationState,
    pointer_state: PointerState,
}

pub(super) enum ConfigurationState {
    NotConfiured,
    Ready,
    Configured,
}

impl Window {
    pub(super) fn init(font_collection: Arc<FontCollection>, config: &Config) -> Self {
        Self {
            banners: vec![],
            font_collection,

            rect_size: RectSize::new(
                config.general().width.into(),
                config.general().height.into(),
            ),
            margin: Margin::new(),

            compositor: None,
            layer_shell: None,
            surface: None,
            layer_surface: None,

            shm: None,
            shm_pool: None,
            buffer: None,
            wl_buffer: None,

            configuration_state: ConfigurationState::NotConfiured,
            pointer_state: Default::default(),
        }
    }

    pub(super) fn deinit(&self) {
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

    pub(super) fn configure(&mut self, qhandle: &QueueHandle<Window>, config: &Config) {
        let Some(layer_shell) = self.layer_shell.as_ref() else {
            eprintln!("Tried to configure window when it doesn't have zwlr_layer_shell_v1!");
            return;
        };

        let surface = self.compositor.as_ref()
                        .expect(
                            "The wl_compositor protocol must be before than the zwlr-layer-shell-v1 protocol.\
                            If it is not correct, please contact to developers with this information"
                        ).create_surface(qhandle, ());

        self.layer_surface = Some(layer_shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "noti-app".to_string(),
            qhandle,
            (),
        ));

        self.relocate(config.general().offset, &config.general().anchor);

        {
            let layer_surface = unsafe { self.layer_surface.as_ref().unwrap_unchecked() };
            layer_surface.set_size(self.rect_size.width as u32, self.rect_size.height as u32);
            layer_surface
                .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        }
        surface.commit();

        self.surface = Some(surface);
    }

    pub(super) fn reconfigure(&mut self, config: &Config) {
        self.relocate(config.general().offset, &config.general().anchor);
        self.banners
            .sort_by(config.general().sorting.get_cmp::<BannerRect>());
    }

    fn relocate(&mut self, (x, y): (u8, u8), anchor_cfg: &config::Anchor) {
        if let Some(layer_surface) = self.layer_surface.as_ref() {
            self.margin = Margin::from_anchor(x as i32, y as i32, anchor_cfg);

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
            self.margin.apply(&layer_surface);
        }
    }

    pub(super) fn configuration_state(&self) -> &ConfigurationState {
        &self.configuration_state
    }

    pub(super) fn is_empty(&self) -> bool {
        self.banners.is_empty()
    }

    pub(super) fn update_banners(&mut self, mut notifications: Vec<Notification>, config: &Config) {
        self.replace_by_indices(&mut notifications, config);

        self.banners
            .extend(notifications.into_iter().map(|notification| {
                let mut banner_rect = BannerRect::init(notification);
                banner_rect.draw(&self.font_collection, config);
                banner_rect
            }));
        self.banners
            .sort_by(config.general().sorting.get_cmp::<BannerRect>());
    }

    pub(super) fn replace_by_indices(
        &mut self,
        notifications: &mut Vec<Notification>,
        config: &Config,
    ) {
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
            rect.draw(&self.font_collection, config);
        }
    }

    pub(super) fn remove_banners(&mut self, indices_to_remove: &[usize]) -> Vec<Notification> {
        indices_to_remove
            .iter()
            .sorted()
            .rev()
            .map(|id| self.banners.remove(*id).destroy_and_get_notification())
            .collect()
    }

    pub(super) fn remove_banners_by_id(
        &mut self,
        notification_indices: &[u32],
    ) -> Vec<Notification> {
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

    pub(super) fn remove_expired_banners(&mut self, config: &Config) -> Vec<Notification> {
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
                    let timeout = config.display_by_app(&rect.notification().app_name).timeout;
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

    pub(super) fn handle_hover(&mut self, config: &Config) {
        if let Some(index) = self.get_hovered_banner(config) {
            self.banners[index].update_timeout();
        }
    }

    pub(super) fn handle_click(&mut self, config: &Config) -> Vec<RendererMessage> {
        if let PrioritiedPressState::Unpressed = self.pointer_state.press_state {
            return vec![];
        }
        self.pointer_state.press_state.clear();

        if let Some(i) = self.get_hovered_banner(config) {
            if config.general().anchor.is_bottom() {
                self.pointer_state.y -=
                    config.general().height as f64 + config.general().gap as f64;
            }

            return self
                .remove_banners(&[i])
                .into_iter()
                .map(|notification| RendererMessage::ClosedNotification {
                    id: notification.id,
                    reason: dbus::actions::ClosingReason::DismissedByUser,
                })
                .collect();
        }

        vec![]
    }

    fn get_hovered_banner(&self, config: &Config) -> Option<usize> {
        if !self.pointer_state.entered {
            return None;
        }

        let rect_height = config.general().height as usize;
        let gap = config.general().gap as usize;

        (0..self.rect_size.height as usize)
            .step_by(rect_height + gap)
            .enumerate()
            .take(self.banners.len())
            .find(|&(_, rect_top)| {
                let rect_bottom = rect_top + rect_height;
                (rect_top as f64..rect_bottom as f64).contains(&(self.pointer_state.y))
            })
            .map(|(i, _)| {
                if config.general().anchor.is_top() {
                    self.banners.len() - i - 1
                } else {
                    i
                }
            })
    }

    pub(super) fn redraw(&mut self, qhandle: &QueueHandle<Window>, config: &Config) {
        self.banners
            .iter_mut()
            .for_each(|banner| banner.draw(&self.font_collection, config));

        self.draw(qhandle, config);
    }

    pub(super) fn draw(&mut self, qhandle: &QueueHandle<Window>, config: &Config) {
        let gap = config.general().gap;

        self.resize(RectSize::new(
            config.general().width.into(),
            self.banners.len() * config.general().height as usize
                + self.banners.len().saturating_sub(1) * gap as usize,
        ));

        let gap_buffer = self.allocate_gap_buffer(gap);

        self.create_buffer(qhandle);
        self.write_banners_to_buffer(&config.general().anchor, &gap_buffer);
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
            self.shm_pool = Some(
                self.shm
                    .as_ref()
                    .expect("Must be wl_shm protocol to use create wl_shm_pool")
                    .create_pool(
                        buffer.as_fd(),
                        self.rect_size.area() as i32 * 4,
                        qhandle,
                        (),
                    ),
            );
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

    pub(super) fn frame(&self, qhandle: &QueueHandle<Window>) {
        let surface = unsafe { self.surface.as_ref().unwrap_unchecked() };
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.frame(qhandle, ());
        surface.attach(self.wl_buffer.as_ref(), 0, 0);
    }

    pub(super) fn commit(&self) {
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

    entered: bool,
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

    fn leave(&mut self) {
        self.entered = false;
    }

    fn enter_and_relocate(&mut self, x: f64, y: f64) {
        self.entered = true;
        self.relocate(x, y)
    }

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
                    state.layer_shell = Some(
                        registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                            name,
                            version,
                            qhandle,
                            (),
                        ),
                    );

                    state.configuration_state = ConfigurationState::Ready;
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
            } => state.pointer_state.enter_and_relocate(surface_x, surface_y),
            wl_pointer::Event::Leave { .. } => state.pointer_state.leave(),
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

            if width != 0 || height != 0 {
                state.rect_size.width = width as usize;
                state.rect_size.height = height as usize;
            }

            state.configuration_state = ConfigurationState::Configured
        }
    }
}
