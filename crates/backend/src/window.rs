use indexmap::{indexmap, IndexMap};
use log::{debug, error, trace};
use shared::cached_data::CachedData;
use std::{
    cmp::Ordering,
    fs::File,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::FileExt,
    },
    path::PathBuf,
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

use crate::{banner::BannerRect, cache::CachedLayout};
use render::{font::FontCollection, types::RectSize};

pub(super) struct Window {
    banners: IndexMap<u32, BannerRect>,
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
        debug!("Window: Initialized");

        Self {
            banners: indexmap! {},
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

        debug!("Window: Deinitialized");
    }

    pub(super) fn configure(&mut self, qhandle: &QueueHandle<Window>, config: &Config) {
        let Some(layer_shell) = self.layer_shell.as_ref() else {
            error!("Tried to configure window when it doesn't have zwlr_layer_shell_v1");
            return;
        };

        let surface = self.compositor.as_ref()
                        .expect(
                            "The wl_compositor protocol must be before than the zwlr-layer-shell-v1 protocol.\
                            If it is not correct, please contact to developers with this information"
                        ).create_surface(qhandle, ());
        debug!("Window: Created surface");

        self.layer_surface = Some(layer_shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "noti-app".to_string(),
            qhandle,
            (),
        ));
        debug!("Window: Created layer surface");

        self.relocate(config.general().offset, &config.general().anchor);

        {
            let layer_surface = unsafe { self.layer_surface.as_ref().unwrap_unchecked() };
            layer_surface.set_size(self.rect_size.width as u32, self.rect_size.height as u32);
            layer_surface
                .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        }
        surface.commit();

        self.surface = Some(surface);

        debug!("Window: Configured");
    }

    pub(super) fn reconfigure(&mut self, config: &Config) {
        self.relocate(config.general().offset, &config.general().anchor);
        self.banners
            .sort_by_values(config.general().sorting.get_cmp::<BannerRect>());
        debug!("Window: Re-sorted the notification banners");

        debug!("Window: Reconfigured by updated config");
    }

    fn relocate(&mut self, (x, y): (u8, u8), anchor_cfg: &config::general::Anchor) {
        if let Some(layer_surface) = self.layer_surface.as_ref() {
            debug!("Window: Relocate to anchor {anchor_cfg:?} with offsets x - {x} and y - {y}");
            self.margin = Margin::from_anchor(x as i32, y as i32, anchor_cfg);

            let anchor = match anchor_cfg {
                config::general::Anchor::Top => Anchor::Top,
                config::general::Anchor::TopLeft => Anchor::Top.union(Anchor::Left),
                config::general::Anchor::TopRight => Anchor::Top.union(Anchor::Right),
                config::general::Anchor::Bottom => Anchor::Bottom,
                config::general::Anchor::BottomLeft => Anchor::Bottom.union(Anchor::Left),
                config::general::Anchor::BottomRight => Anchor::Bottom.union(Anchor::Right),
                config::general::Anchor::Left => Anchor::Left,
                config::general::Anchor::Right => Anchor::Right,
            };

            layer_surface.set_anchor(anchor);
            self.margin.apply(layer_surface);
        }
    }

    pub(super) fn configuration_state(&self) -> &ConfigurationState {
        &self.configuration_state
    }

    pub(super) fn is_empty(&self) -> bool {
        self.banners.is_empty()
    }

    pub(super) fn update_banners(
        &mut self,
        mut notifications: Vec<Notification>,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) {
        self.replace_by_indices(&mut notifications, config, cached_layouts);

        self.banners
            .extend(notifications.into_iter().map(|notification| {
                let mut banner_rect = BannerRect::init(notification);
                banner_rect.draw(&self.font_collection, config, cached_layouts);
                (banner_rect.notification().id, banner_rect)
            }));

        self.banners
            .sort_by_values(config.general().sorting.get_cmp::<BannerRect>());
        debug!("Window: Sorted the notification banners");

        debug!("Window: Completed update the notification banners")
    }

    pub(super) fn replace_by_indices(
        &mut self,
        notifications: &mut Vec<Notification>,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) {
        let matching_indices: Vec<usize> = notifications
            .iter()
            .enumerate()
            .filter_map(|(i, notification)| self.banners.get(&notification.id).map(|_| i))
            .collect();

        for notification_index in matching_indices.into_iter().rev() {
            let notification = notifications.remove(notification_index);

            let rect = &mut self.banners[&notification.id];
            rect.update_data(notification);
            rect.draw(&self.font_collection, config, cached_layouts);

            debug!(
                "Window: Replaced notification by id {}",
                rect.notification().id
            );
        }
    }

    pub(super) fn remove_banners_by_id(
        &mut self,
        notification_indices: &[u32],
    ) -> Vec<Notification> {
        debug!("Window: Remove banners by id");

        notification_indices
            .iter()
            .filter_map(|notification_id| {
                self.banners
                    .shift_remove(notification_id)
                    .map(|banner| banner.destroy_and_get_notification())
            })
            .collect()
    }

    pub(super) fn remove_expired_banners(&mut self, config: &Config) -> Vec<Notification> {
        let indices_to_remove: Vec<u32> = self
            .banners
            .values()
            .filter_map(|rect| match &rect.notification().expire_timeout {
                notification::Timeout::Millis(millis) => {
                    if rect.created_at().elapsed().as_millis() > *millis as u128 {
                        Some(rect.notification().id)
                    } else {
                        None
                    }
                }
                notification::Timeout::Never => None,
                notification::Timeout::Configurable => {
                    let notification = rect.notification();
                    let timeout = config
                        .display_by_app(&notification.app_name)
                        .timeout
                        .by_urgency(&notification.hints.urgency);
                    if timeout != 0 && rect.created_at().elapsed().as_millis() > timeout as u128 {
                        Some(rect.notification().id)
                    } else {
                        None
                    }
                }
            })
            .collect();

        if indices_to_remove.is_empty() {
            vec![]
        } else {
            debug!("Window: Remove expired banners by indices: {indices_to_remove:?}");
            self.remove_banners_by_id(&indices_to_remove)
        }
    }

    pub(super) fn handle_hover(&mut self, config: &Config) {
        if let Some(index) = self.get_hovered_banner(config) {
            self.banners[&index].update_timeout();

            // INFO: because of every tracking pointer position, it emits very frequently and it's
            // annoying. So moved to 'TRACE' level for specific situations.
            trace!("Window: Updated timeout of hovered notification banner with id {index}");
        }
    }

    pub(super) fn handle_click(&mut self, config: &Config) -> Vec<RendererMessage> {
        if let PrioritiedPressState::Unpressed = self.pointer_state.press_state {
            return vec![];
        }
        self.pointer_state.press_state.clear();

        if let Some(id) = self.get_hovered_banner(config) {
            if config.general().anchor.is_bottom() {
                self.pointer_state.y -=
                    config.general().height as f64 + config.general().gap as f64;
            }

            debug!("Window: Clicked to notification banner with id {id}");

            return self
                .remove_banners_by_id(&[id])
                .into_iter()
                .map(|notification| RendererMessage::ClosedNotification {
                    id: notification.id,
                    reason: dbus::actions::ClosingReason::DismissedByUser,
                })
                .collect();
        }

        vec![]
    }

    fn get_hovered_banner(&self, config: &Config) -> Option<u32> {
        if !self.pointer_state.entered {
            return None;
        }

        let rect_height = config.general().height as usize;
        let gap = config.general().gap as usize;

        let region_iter = (0..self.rect_size.height).step_by(rect_height + gap);
        let finder = |(banner, rect_top): (&BannerRect, usize)| {
            let rect_bottom = rect_top + rect_height;
            (rect_top as f64..rect_bottom as f64)
                .contains(&(self.pointer_state.y))
                .then(|| banner.notification().id)
        };

        if config.general().anchor.is_top() {
            self.banners
                .values()
                .rev()
                .zip(region_iter)
                .find_map(finder)
        } else {
            self.banners.values().zip(region_iter).find_map(finder)
        }
    }

    pub(super) fn redraw(
        &mut self,
        qhandle: &QueueHandle<Window>,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) {
        self.banners
            .values_mut()
            .for_each(|banner| banner.draw(&self.font_collection, config, cached_layouts));

        self.draw(qhandle, config);

        debug!("Window: Redrawed banners");
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

        debug!(
            "Window: Resized to width - {}, height - {}",
            self.rect_size.width, self.rect_size.height
        );
    }

    fn allocate_gap_buffer(&self, gap: u8) -> Vec<u8> {
        let rowstride = self.rect_size.width * 4;
        let gap_size = gap as usize * rowstride;
        vec![0; gap_size]
    }

    fn write_banners_to_buffer(&mut self, anchor: &config::general::Anchor, gap_buffer: &[u8]) {
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
            self.banners.values().rev().enumerate().for_each(writer)
        } else {
            self.banners.values().enumerate().for_each(writer)
        }

        debug!("Window: Writed banners to buffer");
    }

    fn create_buffer(&mut self, qhandle: &QueueHandle<Window>) {
        if self.buffer.is_some() {
            let buffer = unsafe { self.buffer.as_mut().unwrap_unchecked() };
            buffer.reset();
            return;
        }

        let buffer = Buffer::new();

        if self.shm_pool.is_none() {
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

        debug!("Window: Created buffer");
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

        debug!("Window: Builded buffer");
    }

    pub(super) fn frame(&self, qhandle: &QueueHandle<Window>) {
        let surface = unsafe { self.surface.as_ref().unwrap_unchecked() };
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.frame(qhandle, ());
        surface.attach(self.wl_buffer.as_ref(), 0, 0);

        debug!("Window: Requested a frame to the Wayland compositor");
    }

    pub(super) fn commit(&self) {
        let surface = unsafe { self.surface.as_ref().unwrap_unchecked() };
        surface.commit();

        debug!("Window: Commited")
    }
}

trait SortByValues<K, V> {
    fn sort_by_values(&mut self, cmp: for<'a> fn(&'a V, &'a V) -> Ordering);
}

impl<K, V> SortByValues<K, V> for IndexMap<K, V> {
    fn sort_by_values(&mut self, cmp: for<'a> fn(&'a V, &'a V) -> Ordering) {
        self.sort_by(|_, lhs, _, rhs| cmp(lhs, rhs));
    }
}

struct Buffer {
    file: File,
    cursor: u64,
    size: usize,
}

impl Buffer {
    fn new() -> Self {
        debug!("Buffer: Trying to create");
        Self {
            file: tempfile::tempfile().expect("The tempfile must be created"),
            cursor: 0,
            size: 0,
        }
    }

    fn reset(&mut self) {
        self.cursor = 0;
        debug!("Buffer: Reset");
    }

    fn push(&mut self, data: &[u8]) {
        self.file
            .write_all_at(data, self.cursor)
            .expect("Must be possibility to write into file!");
        self.cursor += data.len() as u64;

        self.size = std::cmp::max(self.size, self.cursor as usize);

        debug!("Buffer: Received a data to write")
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

    fn from_anchor(x: i32, y: i32, anchor: &config::general::Anchor) -> Self {
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
    Lmb,
    Rmb,
    Mmb,
}

impl PrioritiedPressState {
    fn update(&mut self, new_state: PrioritiedPressState) {
        match self {
            PrioritiedPressState::Lmb => (),
            PrioritiedPressState::Rmb => {
                if let PrioritiedPressState::Lmb = &new_state {
                    *self = new_state
                }
            }
            PrioritiedPressState::Mmb => match &new_state {
                PrioritiedPressState::Lmb | PrioritiedPressState::Rmb => *self = new_state,
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

        debug!("Pointer: Left");
    }

    fn enter_and_relocate(&mut self, x: f64, y: f64) {
        self.entered = true;
        debug!("Pointer: Entered");

        self.relocate(x, y);
    }

    fn relocate(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;

        // INFO: Pointer state updates very frequently so in 'DEBUG' level rows will be filled with
        // useless information about pointer. So moved into 'TRACE' level.
        trace!("Pointer: Relocate to x - {x}, y - {y}")
    }

    fn press(&mut self, button: u32) {
        debug!("Pointer: Pressed button {button}");
        match button {
            PointerState::LEFT_BTN => self.press_state.update(PrioritiedPressState::Lmb),
            PointerState::RIGHT_BTN => self.press_state.update(PrioritiedPressState::Rmb),
            PointerState::MIDDLE_BTN => self.press_state.update(PrioritiedPressState::Mmb),
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
                    debug!("Window: Bound the wl_compositor");
                }
                "wl_shm" => {
                    state.shm =
                        Some(registry.bind::<wl_shm::WlShm, _, _>(name, version, qhandle, ()));
                    debug!("Window: Bound the wl_shm");
                }
                "wl_seat" => {
                    registry.bind::<wl_seat::WlSeat, _, _>(name, version, qhandle, ());
                    debug!("Window: Bound the wl_seat");
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
                    debug!("Window: Bound the zwlr_layer_shell_v1");

                    state.configuration_state = ConfigurationState::Ready;
                    debug!("Window: Ready to configure")
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
                debug!("Window: Received a pointer");
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

            state.configuration_state = ConfigurationState::Configured;
            debug!("Window: Configured layer surface")
        }
    }
}
