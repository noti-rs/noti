use indexmap::{indexmap, IndexMap};
use log::{debug, error, trace};
use shared::cached_data::CachedData;
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, VecDeque},
    fs::File,
    hash::Hash,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::FileExt,
    },
    path::PathBuf,
    rc::Rc,
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
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, Anchor},
};

use config::{self, Config};
use dbus::{
    actions::Signal,
    notification::{self, Notification},
};

use crate::{
    banner::{Banner, DrawState},
    cache::CachedLayout,
};
use render::{types::RectSize, PangoContext};

pub(super) struct Window {
    banners: IndexMap<u32, Banner>,
    pango_context: Rc<RefCell<PangoContext>>,

    rect_size: RectSize<usize>,
    margin: Margin,

    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,

    shm: Option<wl_shm::WlShm>,
    shm_pool: Option<wl_shm_pool::WlShmPool>,
    sloted_buffer: Option<SlotedBuffer<u32>>,
    wl_buffer: Option<wl_buffer::WlBuffer>,

    configuration_state: ConfigurationState,
    pointer_state: PointerState,
    cursor_manager: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
}

pub(super) enum ConfigurationState {
    NotConfiured,
    Ready,
    Configured,
}

impl Window {
    pub(super) fn init(pango_context: Rc<RefCell<PangoContext>>, config: &Config) -> Self {
        debug!("Window: Initialized");

        Self {
            banners: indexmap! {},
            pango_context,

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
            sloted_buffer: None,
            wl_buffer: None,

            configuration_state: ConfigurationState::NotConfiured,
            pointer_state: Default::default(),
            cursor_manager: None,
        }
    }

    pub(super) fn deinit(&self) {
        if let Some(layer_shell) = self.layer_shell.as_ref() {
            layer_shell.destroy();
        }

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
            "noti".to_string(),
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
            .sort_by_values(config.general().sorting.get_cmp::<Banner>());
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

    pub(super) fn total_banners(&self) -> usize {
        self.banners.len()
    }

    pub(super) fn configuration_state(&self) -> &ConfigurationState {
        &self.configuration_state
    }

    pub(super) fn is_empty(&self) -> bool {
        self.banners.is_empty()
    }

    pub(super) fn update_banners(
        &mut self,
        notifications: Vec<Notification>,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) -> Result<(), Vec<Notification>> {
        let mut failed_to_draw_banners = vec![];
        for notification in notifications {
            let mut banner = Banner::init(notification);
            match banner.draw(&self.pango_context.borrow(), config, cached_layouts) {
                crate::banner::DrawState::Success => {
                    self.banners.insert(banner.notification().id, banner);
                }
                crate::banner::DrawState::Failure => {
                    failed_to_draw_banners.push(banner.destroy_and_get_notification());
                }
            }
        }

        self.banners
            .sort_by_values(config.general().sorting.get_cmp::<Banner>());
        debug!("Window: Sorted the notification banners");

        debug!("Window: Completed update the notification banners");

        if failed_to_draw_banners.is_empty() {
            Ok(())
        } else {
            Err(failed_to_draw_banners)
        }
    }

    pub(super) fn replace_by_indices(
        &mut self,
        notifications: &mut VecDeque<Notification>,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) -> Result<(), Vec<Notification>> {
        let matching_indices: Vec<usize> = notifications
            .iter()
            .enumerate()
            .filter_map(|(i, notification)| self.banners.get(&notification.id).map(|_| i))
            .collect();

        let mut failed_to_redraw_banners = vec![];
        for notification_index in matching_indices.into_iter().rev() {
            let notification = notifications.remove(notification_index).unwrap();
            let id = notification.id;

            let banner = &mut self.banners[&id];
            banner.update_data(notification);

            match banner.draw(&self.pango_context.borrow(), config, cached_layouts) {
                DrawState::Success => {
                    debug!("Window: Replaced notification by id {id}",);
                }
                DrawState::Failure => {
                    failed_to_redraw_banners.push(
                        self.banners
                            .shift_remove(&id)
                            .expect("There is should be banner")
                            .destroy_and_get_notification(),
                    );
                }
            }
        }

        if failed_to_redraw_banners.is_empty() {
            Ok(())
        } else {
            Err(failed_to_redraw_banners)
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
            self.banners[&index].reset_timeout();

            // INFO: because of every tracking pointer position, it emits very frequently and it's
            // annoying. So moved to 'TRACE' level for specific situations.
            trace!("Window: Updated timeout of hovered notification banner with id {index}");
        }
    }

    pub(super) fn reset_timeouts(&mut self) {
        self.banners.values_mut().for_each(Banner::reset_timeout);
    }

    pub(super) fn handle_click(&mut self, config: &Config) -> Vec<Signal> {
        if let PrioritiedPressState::Unpressed = self.pointer_state.press_state {
            return vec![];
        }
        self.pointer_state.press_state.clear();

        if let Some(id) = self.get_hovered_banner(config) {
            if config.general().anchor.is_bottom() {
                self.pointer_state.y -=
                    self.banners[&id].height() as f64 + config.general().gap as f64;
            }

            debug!("Window: Clicked to notification banner with id {id}");

            return self
                .remove_banners_by_id(&[id])
                .into_iter()
                .map(|notification| {
                    let notification_id = notification.id;
                    Signal::NotificationClosed {
                        notification_id,
                        reason: dbus::actions::ClosingReason::DismissedByUser,
                    }
                })
                .collect();
        }

        vec![]
    }

    fn get_hovered_banner(&self, config: &Config) -> Option<u32> {
        if !self.pointer_state.entered {
            return None;
        }

        let mut offset = 0.0;
        let gap = config.general().gap as f64;

        let finder = |banner: &Banner| {
            let banner_height = banner.height() as f64;
            let bottom = offset + banner_height;
            if (offset..bottom).contains(&self.pointer_state.y) {
                Some(banner.notification().id)
            } else {
                offset += banner_height + gap;
                None
            }
        };

        if config.general().anchor.is_top() {
            self.banners.values().rev().find_map(finder)
        } else {
            self.banners.values().find_map(finder)
        }
    }

    pub(super) fn redraw(
        &mut self,
        qhandle: &QueueHandle<Window>,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) -> Result<(), Vec<Notification>> {
        let mut failed_to_redraw_banners = vec![];
        for (id, banner) in &mut self.banners {
            if let DrawState::Failure =
                banner.draw(&self.pango_context.borrow(), config, cached_layouts)
            {
                failed_to_redraw_banners.push(*id);
            }
        }

        let failed_to_redraw_banners: Vec<_> = failed_to_redraw_banners
            .into_iter()
            .map(|id| {
                self.banners
                    .shift_remove(&id)
                    .expect("There is should be a banner")
                    .destroy_and_get_notification()
            })
            .collect();

        self.draw(qhandle, config);

        debug!("Window: Redrawed banners");

        if failed_to_redraw_banners.is_empty() {
            Ok(())
        } else {
            Err(failed_to_redraw_banners)
        }
    }

    pub(super) fn draw(&mut self, qhandle: &QueueHandle<Window>, config: &Config) {
        let gap = config.general().gap;

        self.resize(RectSize::new(
            config.general().width.into(),
            self.banners
                .values()
                .map(|banner| banner.height())
                .sum::<usize>()
                + self.banners.len().saturating_sub(1) * gap as usize,
        ));

        let gap_buffer = self.allocate_gap_buffer(gap);

        self.banners.values_mut().for_each(|banner| {
            if banner.framebuffer().is_empty() {
                if let Some(framebuffer) = self
                    .sloted_buffer
                    .as_ref()
                    .and_then(|sb| sb.data_by_slot(&banner.notification().id))
                {
                    banner.set_framebuffer(framebuffer);
                }
            }
        });

        self.create_or_reset_buffer(qhandle);
        self.write_banners_to_buffer(&config.general().anchor, &gap_buffer);
        self.build_buffer(qhandle);
    }

    fn resize(&mut self, rect_size: RectSize<usize>) {
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

        let threshold = self.banners.len().saturating_sub(1);
        let mut total = 0;
        let writer = |banner: &mut Banner| {
            if let Some(sloted_buffer) = self.sloted_buffer.as_mut() {
                sloted_buffer.push(banner.notification().id, &banner.take_framebuffer())
            }

            if total < threshold {
                if let Some(sloted_buffer) = self.sloted_buffer.as_mut() {
                    sloted_buffer.push_without_slot(gap_buffer);
                }
            }

            total += 1;
        };

        if anchor.is_top() {
            self.banners.values_mut().rev().for_each(writer)
        } else {
            self.banners.values_mut().for_each(writer)
        }

        debug!("Window: Writed banners to buffer");
    }

    fn create_or_reset_buffer(&mut self, qhandle: &QueueHandle<Window>) {
        if let Some(sloted_buffer) = self.sloted_buffer.as_mut() {
            sloted_buffer.reset();
            return;
        }

        let sloted_buffer = SlotedBuffer::new();

        if self.shm_pool.is_none() {
            self.shm_pool = Some(
                self.shm
                    .as_ref()
                    .expect("Must be wl_shm protocol to use create wl_shm_pool")
                    .create_pool(
                        sloted_buffer.buffer.as_fd(),
                        self.rect_size.area() as i32 * 4,
                        qhandle,
                        (),
                    ),
            );
        }

        self.sloted_buffer = Some(sloted_buffer);

        debug!("Window: Created buffer");
    }

    fn build_buffer(&mut self, qhandle: &QueueHandle<Window>) {
        if let Some(wl_buffer) = self.wl_buffer.as_ref() {
            wl_buffer.destroy();
        }

        assert!(
            self.shm_pool.is_some() && self.sloted_buffer.is_some(),
            "The buffer must be created before build!"
        );

        assert!(
            self.sloted_buffer
                .as_ref()
                .is_some_and(|sb| sb.buffer.size() >= self.rect_size.area() * 4),
            "Buffer size must be greater or equal to window size. Buffer size: {}. Window area: {}.",
            self.sloted_buffer.as_ref().map(|sb| sb.buffer.size()).unwrap_or_default(),
            self.rect_size.area() * 4
        );

        let shm_pool = unsafe { self.shm_pool.as_ref().unwrap_unchecked() };
        //INFO: The Buffer size only growth and it guarantee that shm_pool never shrinks
        shm_pool.resize(unsafe {
            self.sloted_buffer
                .as_ref()
                .map(|sb| sb.buffer.size())
                .unwrap_unchecked()
        } as i32);

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

/// The wrapper for Buffer which can deal with Slots. It allows to store data by key for reading
/// later. Also SlotedBuffer allows write data without key if it's useless to read later.
struct SlotedBuffer<T>
where
    T: Hash + Eq,
{
    buffer: Buffer,
    slots: HashMap<T, Slot>,
}

struct Slot {
    offset: usize,
    len: usize,
}

impl<T> SlotedBuffer<T>
where
    T: Hash + Eq,
{
    fn new() -> Self {
        debug!("SlotedBuffer: Trying to create");
        let sb = Self {
            buffer: Buffer::new(),
            slots: HashMap::new(),
        };
        debug!("SlotedBuffer: Created!");
        sb
    }

    /// Clears the data of buffers and slots. But the size of buffer remains.
    fn reset(&mut self) {
        self.buffer.reset();
        self.slots.clear();
        debug!("SlotedBuffer: Reset")
    }

    /// Pushes the data with creating a slot into buffer.
    fn push(&mut self, key: T, data: &[u8]) {
        self.slots.insert(
            key,
            Slot {
                offset: self.buffer.filled_size(),
                len: data.len(),
            },
        );
        self.buffer.push(data);
        debug!("SlotedBuffer: Received data to create Slot")
    }

    /// Pushes the data wihtout creating a slot into buffer.
    fn push_without_slot(&mut self, data: &[u8]) {
        self.buffer.push(data);
        debug!("SlotedBuffer: Received data to write without slot.")
    }

    /// Retrieves a copy of data using specific slot by key.
    fn data_by_slot(&self, key: &T) -> Option<Vec<u8>> {
        self.slots.get(key).and_then(|slot| {
            let mut buffer = vec![0; slot.len];

            if let Err(err) = self.buffer.file.read_at(&mut buffer, slot.offset as u64) {
                error!("SlotedBuffer: Failed to read slot from buffer. The error: {err}");
                return None;
            }

            Some(buffer)
        })
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
        self.file
            .write_all_at(&vec![0; self.cursor as usize + 1], 0)
            .expect("Must be possibility to write into file");
        self.cursor = 0;
        debug!("Buffer: Reset");
    }

    fn push(&mut self, data: &[u8]) {
        self.file
            .write_all_at(data, self.cursor)
            .expect("Must be possibility to write into file");
        self.cursor += data.len() as u64;

        self.size = std::cmp::max(self.size, self.cursor as usize);

        debug!("Buffer: Received a data to write")
    }

    fn filled_size(&self) -> usize {
        self.cursor as usize
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
                "wp_cursor_shape_manager_v1" => {
                    state.cursor_manager = Some(
                        registry.bind::<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1, _, _>(
                            name,
                            version,
                            qhandle,
                            (),
                        ),
                    );

                    debug!("Window: Bound the wp_cursor_shape_manager_v1");
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
delegate_noop!(Window: ignore wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
delegate_noop!(Window: ignore wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);

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
        pointer: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                serial,
                ..
            } => {
                if let Some(cursor_manager) = &state.cursor_manager {
                    let cursor_shape = cursor_manager.get_pointer(pointer, qhandle, ());
                    cursor_shape.set_shape(serial, wp_cursor_shape_device_v1::Shape::Pointer);
                }

                state.pointer_state.enter_and_relocate(surface_x, surface_y);
            }
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
