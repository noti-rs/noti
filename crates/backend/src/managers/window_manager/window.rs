use super::{
    banner::{Banner, DrawState},
    CachedLayout,
};
use crate::dispatcher::Dispatcher;
use config::{self, Config};
use dbus::{
    actions::Signal,
    notification::{self, Notification},
};
use indexmap::{indexmap, IndexMap};
use log::{debug, error, trace};
use render::{types::RectSize, PangoContext};
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
        wl_buffer::WlBuffer,
        wl_callback::WlCallback,
        wl_compositor::WlCompositor,
        wl_pointer::{self, ButtonState, WlPointer},
        wl_seat::WlSeat,
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, ZwlrLayerSurfaceV1},
};

pub(super) struct Window {
    banners: IndexMap<u32, Banner>,
    pango_context: Rc<RefCell<PangoContext>>,

    event_queue: EventQueue<WindowState>,
    state: WindowState,

    sloted_buffer: SlotedBuffer<u32>,
    wl_buffer: Option<WlBuffer>,
}

pub(super) struct WindowState {
    rect_size: RectSize<usize>,
    anchored_margin: AnchoredMargin,

    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    shm_pool: WlShmPool,

    pointer: WlPointer,
    cursor_device: WpCursorShapeDeviceV1,
    pointer_state: PointerState,

    configuration_state: ConfigurationState,
}

pub(super) enum ConfigurationState {
    NotConfiured,
    Configured,
}

impl Window {
    pub(super) fn init<P>(
        wayland_connection: &Connection,
        protocols: &P,
        pango_context: Rc<RefCell<PangoContext>>,
        config: &Config,
    ) -> anyhow::Result<Self>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>,
    {
        let mut event_queue = wayland_connection.new_event_queue();

        let rect_size = RectSize::new(
            config.general().width.into(),
            config.general().height.into(),
        );

        let (surface, layer_surface) = Self::make_surface(protocols, &event_queue.handle());
        let (pointer, cursor_device) = Self::make_pointer(protocols, &event_queue.handle());
        let (sloted_buffer, shm_pool) =
            Self::make_buffer(protocols, &event_queue.handle(), &rect_size);

        let anchored_margin = Self::make_anchored_margin(config);
        anchored_margin.relocate_layer_surface(&layer_surface);

        layer_surface.set_size(rect_size.width as u32, rect_size.height as u32);
        surface.commit();

        let mut state = WindowState {
            rect_size,
            anchored_margin,

            surface,
            layer_surface,
            shm_pool,

            pointer_state: Default::default(),
            cursor_device,
            pointer,

            configuration_state: ConfigurationState::NotConfiured,
        };

        while let ConfigurationState::NotConfiured = state.configuration_state {
            event_queue.blocking_dispatch(&mut state)?;
        }

        debug!("Window: Initialized");

        Ok(Self {
            banners: indexmap! {},
            pango_context,

            event_queue,
            state,

            sloted_buffer,
            wl_buffer: None,
        })
    }

    fn make_surface<P>(
        protocols: &P,
        qhandle: &QueueHandle<WindowState>,
    ) -> (WlSurface, ZwlrLayerSurfaceV1)
    where
        P: AsRef<WlCompositor> + AsRef<ZwlrLayerShellV1>,
    {
        let surface = <P as AsRef<WlCompositor>>::as_ref(protocols).create_surface(qhandle, ());
        let layer_surface = <P as AsRef<ZwlrLayerShellV1>>::as_ref(protocols).get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "noti".to_string(),
            qhandle,
            (),
        );
        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);

        (surface, layer_surface)
    }

    fn make_anchored_margin(config: &Config) -> AnchoredMargin {
        let (x_offset, y_offset) = config.general().offset;
        AnchoredMargin::new(x_offset as i32, y_offset as i32, &config.general().anchor)
    }

    fn make_pointer<P>(
        protocols: &P,
        qhandle: &QueueHandle<WindowState>,
    ) -> (WlPointer, WpCursorShapeDeviceV1)
    where
        P: AsRef<WlSeat> + AsRef<WpCursorShapeManagerV1>,
    {
        let pointer = <P as AsRef<WlSeat>>::as_ref(protocols).get_pointer(qhandle, ());
        let cursor_device = <P as AsRef<WpCursorShapeManagerV1>>::as_ref(protocols).get_pointer(
            &pointer,
            qhandle,
            (),
        );
        (pointer, cursor_device)
    }

    fn make_buffer<P>(
        protocols: &P,
        qhandle: &QueueHandle<WindowState>,
        rect_size: &RectSize<usize>,
    ) -> (SlotedBuffer<u32>, WlShmPool)
    where
        P: AsRef<WlShm>,
    {
        let size = rect_size.area() * 4;
        let mut sloted_buffer = SlotedBuffer::new();
        sloted_buffer.push_without_slot(&vec![0; size]);
        let shm_pool = <P as AsRef<WlShm>>::as_ref(protocols).create_pool(
            sloted_buffer.buffer.as_fd(),
            size as i32,
            qhandle,
            (),
        );
        (sloted_buffer, shm_pool)
    }

    pub(super) fn reconfigure(&mut self, config: &Config) {
        self.relocate(config.general().offset, &config.general().anchor);
        self.banners
            .sort_by_values(config.general().sorting.get_cmp::<Banner>());
        debug!("Window: Re-sorted the notification banners");

        debug!("Window: Reconfigured by updated config");
    }

    fn relocate(&mut self, (x, y): (u8, u8), anchor_cfg: &config::general::Anchor) {
        self.state
            .anchored_margin
            .update(x as i32, y as i32, anchor_cfg);
        self.state
            .anchored_margin
            .relocate_layer_surface(&self.state.layer_surface);
    }

    pub(super) fn total_banners(&self) -> usize {
        self.banners.len()
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
                DrawState::Success => {
                    self.banners.insert(banner.notification().id, banner);
                }
                DrawState::Failure => {
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
        if let PrioritiedPressState::Unpressed = self.state.pointer_state.press_state {
            return vec![];
        }
        let _press_state = self.state.pointer_state.press_state.take();

        if let Some(id) = self.get_hovered_banner(config) {
            if config.general().anchor.is_bottom() {
                self.state.pointer_state.y -=
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
        if !self.state.pointer_state.entered {
            return None;
        }

        let mut offset = 0.0;
        let gap = config.general().gap as f64;

        let finder = |banner: &Banner| {
            let banner_height = banner.height() as f64;
            let bottom = offset + banner_height;
            if (offset..bottom).contains(&self.state.pointer_state.y) {
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

        self.draw(config);

        debug!("Window: Redrawed banners");

        if failed_to_redraw_banners.is_empty() {
            Ok(())
        } else {
            Err(failed_to_redraw_banners)
        }
    }

    pub(super) fn draw(&mut self, config: &Config) {
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
                if let Some(framebuffer) =
                    self.sloted_buffer.data_by_slot(&banner.notification().id)
                {
                    banner.set_framebuffer(framebuffer);
                }
            }
        });

        self.reset_buffer();
        self.write_banners_to_buffer(&config.general().anchor, &gap_buffer);
        self.build_buffer();
    }

    fn resize(&mut self, rect_size: RectSize<usize>) {
        self.state.rect_size = rect_size;

        self.state.layer_surface.set_size(
            self.state.rect_size.width as u32,
            self.state.rect_size.height as u32,
        );

        debug!(
            "Window: Resized to width - {}, height - {}",
            self.state.rect_size.width, self.state.rect_size.height
        );
    }

    fn allocate_gap_buffer(&self, gap: u8) -> Vec<u8> {
        let rowstride = self.state.rect_size.width * 4;
        let gap_size = gap as usize * rowstride;
        vec![0; gap_size]
    }

    fn write_banners_to_buffer(&mut self, anchor: &config::general::Anchor, gap_buffer: &[u8]) {
        let threshold = self.banners.len().saturating_sub(1);
        let mut total = 0;
        let writer = |banner: &mut Banner| {
            self.sloted_buffer
                .push(banner.notification().id, &banner.take_framebuffer());

            if total < threshold {
                self.sloted_buffer.push_without_slot(gap_buffer);
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

    fn reset_buffer(&mut self) {
        self.sloted_buffer.reset();
        debug!("Window: Buffer was reset");
    }

    fn build_buffer(&mut self) {
        if let Some(wl_buffer) = self.wl_buffer.as_ref() {
            wl_buffer.destroy();
        }

        assert!(
            self.sloted_buffer.buffer.size() >= self.state.rect_size.area() * 4,
            "Buffer size must be greater or equal to window size. Buffer size: {}. Window area: {}.",
            self.sloted_buffer.buffer.size(),
            self.state.rect_size.area() * 4
        );

        //INFO: The Buffer size only growth and it guarantee that shm_pool never shrinks
        self.state
            .shm_pool
            .resize(self.sloted_buffer.buffer.size() as i32);

        self.wl_buffer = Some(self.state.shm_pool.create_buffer(
            0,
            self.state.rect_size.width as i32,
            self.state.rect_size.height as i32,
            self.state.rect_size.width as i32 * 4,
            wl_shm::Format::Argb8888,
            &self.event_queue.handle(),
            (),
        ));

        debug!("Window: Builded buffer");
    }

    pub(super) fn frame(&self) {
        self.state.surface.damage(0, 0, i32::MAX, i32::MAX);
        self.state.surface.frame(&self.event_queue.handle(), ());
        self.state.surface.attach(self.wl_buffer.as_ref(), 0, 0);

        debug!("Window: Requested a frame to the Wayland compositor");
    }

    pub(super) fn commit(&self) {
        self.state.surface.commit();
        debug!("Window: Commited")
    }

    pub(super) fn sync(&mut self) -> anyhow::Result<()> {
        self.event_queue.roundtrip(&mut self.state)?;
        Ok(())
    }
}

impl WindowState {
    fn destroy(&self) {
        self.layer_surface.destroy();
        self.surface.destroy();
        self.shm_pool.destroy();
        self.cursor_device.destroy();
        self.pointer.release();
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(buffer) = self.wl_buffer.as_ref() {
            buffer.destroy();
        }

        self.state.destroy();

        if let Err(_err) = self.sync() {
            error!("Window: Failed to sync during deinitialization.")
        }

        debug!("Window: Deinitialized");
    }
}

impl Dispatcher for Window {
    type State = WindowState;
    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)> {
        Some((&mut self.event_queue, &mut self.state))
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

struct AnchoredMargin {
    margin: Margin,
    anchor: Anchor,
}

impl AnchoredMargin {
    fn new(x_offset: i32, y_offset: i32, anchor: &config::general::Anchor) -> Self {
        Self {
            margin: Margin::with_anchor(x_offset, y_offset, anchor),
            anchor: anchor.to_layer_shell_anchor(),
        }
    }

    fn update(&mut self, x_offset: i32, y_offset: i32, anchor: &config::general::Anchor) {
        *self = Self::new(x_offset, y_offset, anchor);
    }

    fn relocate_layer_surface(&self, layer_surface: &ZwlrLayerSurfaceV1) {
        layer_surface.set_anchor(self.anchor);
        self.margin.apply(layer_surface);
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

    fn with_anchor(x: i32, y: i32, anchor: &config::general::Anchor) -> Self {
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
#[derive(Default, Clone)]
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

    fn take(&mut self) -> Self {
        let current_state = self.clone();
        *self = PrioritiedPressState::Unpressed;
        current_state
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

delegate_noop!(WindowState: ignore WlSurface);
delegate_noop!(WindowState: ignore WlShmPool);
delegate_noop!(WindowState: ignore WlBuffer);
delegate_noop!(WindowState: ignore WlCallback);
delegate_noop!(WindowState: ignore WpCursorShapeDeviceV1);

impl Dispatch<WlPointer, ()> for WindowState {
    fn event(
        state: &mut Self,
        _pointer: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                serial,
                ..
            } => {
                state
                    .cursor_device
                    .set_shape(serial, wp_cursor_shape_device_v1::Shape::Pointer);

                state.pointer_state.enter_and_relocate(surface_x, surface_y);
            }
            wl_pointer::Event::Leave { serial, .. } => {
                state
                    .cursor_device
                    .set_shape(serial, wp_cursor_shape_device_v1::Shape::Default);
                state.pointer_state.leave()
            }
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

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WindowState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
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
            debug!("WindowState: Configured layer surface")
        }
    }
}

trait ToLayerShellAnchor {
    fn to_layer_shell_anchor(&self) -> Anchor;
}

impl ToLayerShellAnchor for config::general::Anchor {
    fn to_layer_shell_anchor(&self) -> Anchor {
        match self {
            config::general::Anchor::Top => Anchor::Top,
            config::general::Anchor::TopLeft => Anchor::Top.union(Anchor::Left),
            config::general::Anchor::TopRight => Anchor::Top.union(Anchor::Right),
            config::general::Anchor::Bottom => Anchor::Bottom,
            config::general::Anchor::BottomLeft => Anchor::Bottom.union(Anchor::Left),
            config::general::Anchor::BottomRight => Anchor::Bottom.union(Anchor::Right),
            config::general::Anchor::Left => Anchor::Left,
            config::general::Anchor::Right => Anchor::Right,
        }
    }
}
