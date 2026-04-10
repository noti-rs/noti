use super::{
    banner_stack::{Banner, BannerStack},
    CachedLayout,
};
use crate::{dispatcher::Dispatcher, EglState};
use config::{self, Config};
use dbus::{actions::ClosingReason, notification::Notification};
use log::{debug, error, trace};
use shared::{
    cached_data::CachedData,
    data::{Borrowed, Data},
};
use skia_safe::{
    gpu::{gl::FramebufferInfo, DirectContext},
    Color,
};
use std::{collections::VecDeque, path::PathBuf};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_callback::WlCallback,
        wl_compositor::WlCompositor,
        wl_pointer::{self, ButtonState, WlPointer},
        wl_region::WlRegion,
        wl_seat::WlSeat,
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_egl::WlEglSurface;
use wayland_protocols::wp::{
    cursor_shape::v1::client::{
        wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
        wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
    },
    presentation_time::client::{
        wp_presentation, wp_presentation_feedback::WpPresentationFeedback,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, ZwlrLayerSurfaceV1},
};
use widgets::{
    context::Tick,
    events::{Event, EventKind, MouseButton},
    types::{extent::Extent, offset::Offset, Point},
};

/// Wraps a [WindowState] and holds an event queue used only for dispatching.
///
/// The inner window state can be accessed via `Deref`.
pub(super) struct Window {
    event_queue: EventQueue<WindowState>,
    state: WindowState,
}

/// Represents the state of a `Window`, holding notifications, window properties, Wayland objects,
/// shared rendering data, and a track of user actions.
///
/// Instead of creating a separate Wayland window for each notification, all notifications are drawn
/// as banners inside a single window. This approach avoids the complexity and instability of using
/// multiple windows, especially since many Wayland protocols needed for that are still unstable or
/// not well maintained. Managing everything in one window makes updates, user interactions, and
/// redrawing much easier.
pub(super) struct WindowState {
    banner_stack: BannerStack<u32>,

    actual_size: Extent<usize>,
    anchor: Anchor,
    margin: Margin,

    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    egl_window: WlEglSurface,
    egl_surface: khronos_egl::Surface,
    egl_state: EglState,

    font_collection: skia_safe::textlayout::FontCollection,
    gr_context: skia_safe::gpu::DirectContext,
    config: Data<Config, Borrowed>,
    #[allow(unused)]
    cached_layouts: Data<CachedData<PathBuf, CachedLayout>, Borrowed>,

    pointer: WlPointer,
    cursor_device: WpCursorShapeDeviceV1,
    pointer_state: PointerState,

    has_requested_frame: bool,
    last_presented_time_ns: Option<u64>,
    configuration_state: ConfigurationState,
}

/// Represents the configuration state of a `Window`, focusing on careful resource management.
///
/// A `Window` can request resources from the Wayland compositor, but they are not always granted
/// immediately. Until the compositor provides them, further resource management would be invalid,
/// so this state helps track and wait for permission before continuing.
pub(super) enum ConfigurationState {
    NotConfiured,
    Configured,
}

impl Window {
    /// Initializes a `Window` with its event queue and state.
    ///
    /// Initialization is eager, ensuring that all important data is loaded before it finishes.
    /// Because of this, it may take a little more time to complete.
    pub(super) fn init<P, Gpu>(
        wayland_connection: &Connection,
        protocols: &P,
        gpu: &Gpu,
        config: Data<Config, Borrowed>,
        font_collection: skia_safe::textlayout::FontCollection,
        cached_layouts: Data<CachedData<PathBuf, CachedLayout>, Borrowed>,
    ) -> anyhow::Result<Self>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>,
        Gpu: AsRef<EglState> + AsRef<DirectContext>,
    {
        // Note for developers:
        // To simplify `Window` initialization, the process is divided into three steps:
        // 1. Create a `wl_surface` and `zwlr_layer_surface_v1` from it.
        // 2. Request a `wl_pointer` with `wp_cursor_shape_device`.
        // 3. Create a `wl_egl_surface` from the `wl_surface` and an EGL surface via the EGL instance.
        //
        // Additional step:
        // - Ensure the surface has the correct size and position.

        let mut event_queue = wayland_connection.new_event_queue();

        let actual_size = Extent::new(
            config.general().width.into(),
            config.general().height.into(),
        );

        let (surface, layer_surface) = Self::make_surface(protocols, &event_queue.handle());
        let (pointer, cursor_device) = Self::make_pointer(protocols, &event_queue.handle());
        let (egl_window, egl_surface) = Self::make_egl_surface(&surface, gpu, &actual_size)?;

        let (x_offset, y_offset) = config.general().offset;
        let margin = Margin::with_anchor(
            x_offset as usize,
            y_offset as usize,
            &config.general().anchor,
        );
        let anchor = config.general().anchor.to_layer_shell_anchor();
        layer_surface.set_anchor(anchor);

        layer_surface.set_size(actual_size.width as u32, actual_size.height as u32);

        surface.commit();

        let egl_state: &EglState = gpu.as_ref();
        let gr_context: &DirectContext = gpu.as_ref();
        let mut state = WindowState {
            banner_stack: BannerStack::new(),

            actual_size,
            anchor,
            margin,

            surface,
            layer_surface,
            egl_window,
            egl_surface,
            egl_state: egl_state.clone(),

            font_collection,
            gr_context: gr_context.clone(),
            config: config.clone(),
            cached_layouts: cached_layouts.clone(),

            pointer_state: Default::default(),
            cursor_device,
            pointer,

            has_requested_frame: false,
            last_presented_time_ns: None,
            configuration_state: ConfigurationState::NotConfiured,
        };

        while let ConfigurationState::NotConfiured = state.configuration_state {
            event_queue.blocking_dispatch(&mut state)?;
        }

        debug!("Window: Initialized");

        Ok(Self { event_queue, state })
    }

    /// Creates a `wl_surface` with the `zwlr_layer_surface_v1` role, applying minimal properties.
    ///
    /// By default, the layer surface uses the `Overlay` layer and has no keyboard interactivity.
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
            env!("APP_NAME").to_string(),
            qhandle,
            (),
        );
        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);

        (surface, layer_surface)
    }

    /// Requests a pointer from the Wayland compositor using `wp_cursor_shape_device_v1`
    /// for the specified pointer.
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

    /// Requests a pointer to a `wl_egl_surface` from the `wl_surface` and uses it to create a native EGL surface.
    /// This is important for GPU-accelerated rendering.
    fn make_egl_surface<Gpu>(
        surface: &WlSurface,
        gpu: &Gpu,
        actual_size: &Extent<usize>,
    ) -> anyhow::Result<(WlEglSurface, khronos_egl::Surface)>
    where
        Gpu: AsRef<EglState>,
    {
        let egl_window = WlEglSurface::new(
            surface.id(),
            actual_size.width as i32,
            actual_size.height as i32,
        )?;

        let egl_state: &EglState = gpu.as_ref();
        let egl_surface = unsafe {
            egl_state.instance.create_window_surface(
                egl_state.display,
                egl_state.config,
                egl_window.ptr() as khronos_egl::NativeWindowType,
                None,
            )?
        };

        Ok((egl_window, egl_surface))
    }

    /// Returns `true` if a frame has already been requested from the compositor.
    ///
    /// This flag is used to prevent duplicate frame requests while the previous
    /// frame callback has not yet been processed. Avoiding repeated requests
    /// prevents unnecessary load on the Wayland compositor and keeps rendering
    /// efficient.
    pub(super) fn has_requested_frame(&self) -> bool {
        self.state.has_requested_frame
    }

    pub(super) fn update_input_regions(&mut self, compositor: &WlCompositor) {
        let region = compositor.create_region(&self.event_queue.handle(), ());

        let mut offset = Offset::new(self.state.margin.left, self.state.margin.top);
        let gap = self.state.config.general().gap as usize;

        let iterator = |banner: &Banner| {
            region.add(
                offset.x as i32,
                offset.y as i32,
                banner.width() as i32,
                banner.height() as i32,
            );
            offset.y += banner.height() + gap;
        };

        if self.state.config.general().anchor.is_top() {
            self.state.banner_stack.banners().for_each(iterator)
        } else {
            self.state.banner_stack.banners().rev().for_each(iterator)
        }

        self.state.surface.set_input_region(Some(&region));
        region.destroy();
    }

    /// Requests a frame for the current `Window` from the Wayland compositor.
    /// This allows the compositor to schedule the redraw at the correct time, ensuring smooth
    /// rendering with VSync.
    pub(super) fn frame(&mut self) {
        self.state.surface.damage(0, 0, i32::MAX, i32::MAX);
        self.state.surface.frame(&self.event_queue.handle(), ());
        self.state.has_requested_frame = true;

        debug!("Window: Requested a frame to the Wayland compositor");
    }

    /// Sends a `commit` message to the Wayland compositor to apply previously requested actions.
    pub(super) fn commit(&self, wp_presentation: &wp_presentation::WpPresentation) {
        self.state.surface.commit();
        wp_presentation.feedback(&self.state.surface, &self.event_queue.handle(), ());
        debug!("Window: Commited")
    }

    /// Synchronizes with the Wayland compositor, blocking the current thread until all requested
    /// actions have been completed.
    ///
    /// This ensures that all pending requests are processed immediately.
    pub(super) fn sync(&mut self) -> anyhow::Result<()> {
        self.event_queue.roundtrip(&mut self.state)?;
        Ok(())
    }
}

impl WindowState {
    /// Applies a new user configuration to an existing `Window` state, updating it with the new values.
    pub(super) fn reconfigure(&mut self, config: Data<Config, Borrowed>) {
        self.relocate(config.general().offset, &config.general().anchor);
        self.banner_stack.configure(&config);
        self.config = config.clone();

        debug!("Window: Reconfigured by updated config");
    }

    fn relocate(&mut self, (x, y): (u8, u8), anchor_cfg: &config::general::Anchor) {
        self.margin = Margin::with_anchor(x as usize, y as usize, anchor_cfg);
        self.anchor = anchor_cfg.to_layer_shell_anchor();
        self.layer_surface.set_anchor(self.anchor);
    }

    pub(super) fn total_banners(&self) -> usize {
        self.banner_stack.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.banner_stack.is_empty()
    }

    pub(super) fn add_banners(&mut self, notifications: Vec<Notification>) {
        self.banner_stack.extend_from(
            notifications.into_iter(),
            self.font_collection.clone(),
            &self.config,
        );
    }

    pub(super) fn replace_by_indices(&mut self, notifications: &mut VecDeque<Notification>) {
        self.banner_stack
            .replace_by_keys(notifications, &self.config);
    }

    pub(super) fn close_banners_by_id(&mut self, notification_indices: &[u32]) {
        for notification_id in notification_indices {
            if let Some(banner) = self.banner_stack.get_mut(notification_id) {
                banner.close(ClosingReason::CallCloseNotification)
            }
        }
    }

    pub(super) fn remove_closed_banners(&mut self) -> Vec<(Notification, ClosingReason)> {
        self.banner_stack.remove_closed()
    }

    pub(super) fn reset_timeouts(&mut self) {
        self.banner_stack
            .banners_mut()
            .for_each(Banner::reset_timeout);
    }

    pub(super) fn handle_user_actions(&mut self) {
        if !self.pointer_state.entered && self.pointer_state.events.is_empty() {
            return;
        }

        let mut banners = if self.config.general().anchor.is_top() {
            self.banner_stack.banners_mut().collect::<Vec<_>>()
        } else {
            self.banner_stack.banners_mut().rev().collect::<Vec<_>>()
        };

        while let Some(event) = self.pointer_state.events.pop_front() {
            let mut offset = Offset::new(self.margin.left as f64, self.margin.top as f64);
            let gap = self.config.general().gap as f64;

            for banner in &mut banners {
                let banner_height = banner.height() as f64;

                let bottom = offset.y + banner_height;
                let right = offset.x + banner.width() as f64;

                if (offset.y..bottom).contains(&event.y) {
                    if !(offset.x..right).contains(&event.x) {
                        break;
                    }

                    let event = Event {
                        kind: event.kind.clone().into(),
                        local_coord: Point {
                            x: (event.x - offset.x) as f32,
                            y: (event.y - offset.y) as f32,
                        },
                    };

                    banner.dispatch_event(event);
                    break;
                } else {
                    offset.y += banner_height + gap;
                }
            }
        }
    }

    fn use_current_egl_surface(&self) -> anyhow::Result<()> {
        if let Err(err) = self.egl_state.instance.make_current(
            self.egl_state.display,
            Some(self.egl_surface),
            Some(self.egl_surface),
            Some(self.egl_state.context),
        ) {
            anyhow::bail!(err);
        }

        Ok(())
    }

    fn create_drawing_surface(&mut self) -> anyhow::Result<skia_safe::Surface> {
        let samples = 0;
        let stencil_bits = 8;
        let backend_rt = skia_safe::gpu::backend_render_targets::make_gl(
            (
                self.actual_size.width as i32,
                self.actual_size.height as i32,
            ),
            samples,
            stencil_bits,
            FramebufferInfo {
                fboid: 0,
                format: skia_safe::gpu::gl::Format::RGBA8.into(),
                protected: skia_safe::gpu::Protected::No,
            },
        );

        match skia_safe::gpu::surfaces::wrap_backend_render_target(
            &mut self.gr_context,
            &backend_rt,
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        ) {
            Some(surface) => Ok(surface),
            None => anyhow::bail!("Failed to make drawing surface for layer surface"),
        }
    }

    fn resize(&mut self, logical_size: Extent<usize>) {
        if logical_size.width == 0 && logical_size.height == 0 {
            // INFO: the Wayland compositor may call the callback after destroying a last
            // notification and because of this the width and height of surface equals to 0x0. To
            // avoid this need to set dummy size. It's always last frames before disappearing.
            self.actual_size = Extent::new(1, 1);
        } else {
            self.actual_size = self.margin.apply_to_size(logical_size);
        }

        let Extent { width, height } = self.actual_size;
        self.layer_surface.set_size(width as u32, height as u32);
        let (dx, dy) = (0, 0);
        self.egl_window.resize(width as i32, height as i32, dx, dy);

        debug!(
            "Window: Resized to width - {}, height - {}",
            self.actual_size.width, self.actual_size.height
        );
    }

    /// Sends requests to the Wayland compositor to destroy objects belonging to the current
    /// `Window` state. The [Drop] trait cannot be used for this, because specific requests must
    /// be sent to the Wayland compositor.
    fn destroy(&self) {
        self.layer_surface.destroy();
        self.surface.destroy();
        self.cursor_device.destroy();
        self.pointer.release();
        if let Err(err) = self
            .egl_state
            .instance
            .destroy_surface(self.egl_state.display, self.egl_surface)
        {
            error!("Failed to destroy EGL surface! Further application work won't guaranteed to be normal! Error: {err}.")
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.state.destroy();

        if let Err(err) = self.sync() {
            error!("Window: Failed to sync during deinitialization. Error: {err}")
        }

        debug!("Window: Deinitialized");
    }
}

impl std::ops::Deref for Window {
    type Target = WindowState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl std::ops::DerefMut for Window {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
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

// Represents gaps between window and screen boundaries to which the window is anchored.
//
// Actually it uses for differing the logical and actual window positions and sizes instead of
// setting at Wayland level. Some animations may require appearing from edges of screen including
// gaps, and because of this we here use specific margin.
struct Margin {
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
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

    /// Uses the specified anchor to determine the correct directions for the offsets.
    fn with_anchor(x: usize, y: usize, anchor: &config::general::Anchor) -> Self {
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

    fn apply_to_size(&self, mut extent: Extent<usize>) -> Extent<usize> {
        extent.width += self.left + self.right;
        extent.height += self.top + self.bottom;
        extent
    }
}

/// Represents the state of the user pointer.
///
/// The `state` tracks events that are useful for handling interactions with notification banners.
#[derive(Default)]
struct PointerState {
    events: VecDeque<PointerEvent>,
    x: f64,
    y: f64,

    entered: bool,
}

struct PointerEvent {
    x: f64,
    y: f64,
    kind: PointerEventKind,
}

#[derive(Clone, PartialEq, Eq)]
enum PointerEventKind {
    Hover,
    MouseDown { button: MouseButton },
    MouseUp { button: MouseButton },
}

impl From<PointerEventKind> for EventKind {
    fn from(value: PointerEventKind) -> Self {
        match value {
            PointerEventKind::Hover => EventKind::MouseHover,
            PointerEventKind::MouseDown { button } => EventKind::MouseDown(button),
            PointerEventKind::MouseUp { button } => EventKind::MouseUp(button),
        }
    }
}

impl PointerState {
    const LEFT_BTN: u32 = 272;
    const RIGHT_BTN: u32 = 273;
    const MIDDLE_BTN: u32 = 274;

    /// Updates the current pointer state to indicate that it has left the window frame.
    fn leave(&mut self) {
        self.entered = false;

        debug!("Pointer: Left");
    }

    fn enter(&mut self, x: f64, y: f64) {
        self.entered = true;
        self.update_or_push_hover(x, y);
    }

    /// Updates the current pointer state to reflect movement to a new position.
    ///
    /// Behavior may differ if `ignore_first_relocate` is enabled.
    fn relocate(&mut self, x: f64, y: f64) {
        // if self.ignore_first_relocate {
        //     debug!("Pointer: Forced to ignore first relocate.");
        //
        //     self.ignore_first_relocate = false;
        //     return;
        // }
        self.update_or_push_hover(x, y);

        // INFO: Pointer state updates very frequently so in 'DEBUG' level rows will be filled with
        // useless information about pointer. So moved into 'TRACE' level.
        trace!("Pointer: Relocate to x - {x}, y - {y}")
    }

    fn update_or_push_hover(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;

        if let Some(event) = self
            .events
            .back_mut()
            .take_if(|event| event.kind == PointerEventKind::Hover)
        {
            event.x = x;
            event.y = y;
        } else {
            self.events.push_back(PointerEvent {
                x,
                y,
                kind: PointerEventKind::Hover,
            });
        }
    }

    /// Updates the current pointer state to reflect a user’s mouse button click.
    fn press(&mut self, button: u32) {
        let Some(button) = Self::get_button(button) else {
            return;
        };
        debug!("Pointer: Pressed button {button:?}");

        self.events.push_back(PointerEvent {
            x: self.x,
            y: self.y,
            kind: PointerEventKind::MouseDown { button },
        });
    }

    fn release(&mut self, button: u32) {
        let Some(button) = Self::get_button(button) else {
            return;
        };
        debug!("Pointer: Released button {button:?}");

        self.events.push_back(PointerEvent {
            x: self.x,
            y: self.y,
            kind: PointerEventKind::MouseUp { button },
        });
    }

    fn get_button(button: u32) -> Option<MouseButton> {
        Some(match button {
            PointerState::LEFT_BTN => MouseButton::Left,
            PointerState::RIGHT_BTN => MouseButton::Right,
            PointerState::MIDDLE_BTN => MouseButton::Middle,
            _ => return None,
        })
    }
}

delegate_noop!(WindowState: ignore WlSurface);
delegate_noop!(WindowState: ignore WlRegion);
delegate_noop!(WindowState: ignore WlShmPool);
delegate_noop!(WindowState: ignore WlBuffer);
delegate_noop!(WindowState: ignore WpCursorShapeDeviceV1);

impl Dispatch<WlCallback, ()> for WindowState {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_callback::Event::Done { .. } = event {
            state.has_requested_frame = false;

            state
                .use_current_egl_surface()
                .expect("The EGL surface must be available to make current and use it");

            // TODO: correctly resize for specific animation
            let gap = state.config.general().gap as usize;
            let logical_size = Extent::new(
                state.banner_stack.width(),
                state.banner_stack.total_height_with_gap(gap),
            );
            state.resize(logical_size);

            let mut sk_surface = state
                .create_drawing_surface()
                .expect("The skia's surface must be correct and created without issues");
            sk_surface.canvas().clear(Color::from_argb(0, 0, 0, 0));

            let mut offset = Offset::new(state.margin.left, state.margin.top);
            let writer = |banner: &Banner| {
                banner.draw(&offset.into(), &mut sk_surface);
                offset.y += banner.height() + gap;
            };

            if state.config.general().anchor.is_top() {
                state.banner_stack.banners().for_each(writer)
            } else {
                state.banner_stack.banners().rev().for_each(writer)
            }

            state
                .gr_context
                .flush_and_submit_surface(&mut sk_surface, skia_safe::gpu::SyncCpu::No);

            state
                .egl_state
                .instance
                .swap_interval(state.egl_state.display, 0)
                .and_then(|_| {
                    state
                        .egl_state
                        .instance
                        .swap_buffers(state.egl_state.display, state.egl_surface)
                })
                .expect("The buffer swapping must be errorless");
        }
    }
}

impl Dispatch<WpPresentationFeedback, ()> for WindowState {
    fn event(
        state: &mut Self,
        _proxy: &WpPresentationFeedback,
        event: <WpPresentationFeedback as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wayland_protocols::wp::presentation_time::client::wp_presentation_feedback::Event::Presented { tv_sec_hi, tv_sec_lo, tv_nsec, .. } => {
                let sec = ((tv_sec_hi as u64) << 32) | (tv_sec_lo as u64);
                let time_ns = sec * 1_000_000_000 + tv_nsec as u64;

                if let Some(last_presented_time_ns) = &mut state.last_presented_time_ns {
                    let delta_time_ns = time_ns - *last_presented_time_ns;
                    state.banner_stack.banners_mut().for_each(|banner| banner.tick(delta_time_ns as u128));

                    trace!("Window Presentation: Current FPS — {}", 1_000_000_000.0 / delta_time_ns as f64);
                }

                state.last_presented_time_ns = Some(time_ns);
            },
            wayland_protocols::wp::presentation_time::client::wp_presentation_feedback::Event::Discarded => {
                // Frame has never shown — skip
            },
            _ => (),
        }
    }
}

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

                state.pointer_state.enter(surface_x, surface_y);
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
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(ButtonState::Released),
                ..
            } => state.pointer_state.release(button),
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
                state.actual_size.width = width as usize;
                state.actual_size.height = height as usize;
            }

            state.configuration_state = ConfigurationState::Configured;
            debug!("WindowState: Configured layer surface")
        }
    }
}

/// Anchors from the [config] crate and the wlr-protocols use different types.
/// This trait allows converting between them while preserving the underlying logic.
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
