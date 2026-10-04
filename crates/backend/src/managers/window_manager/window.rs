use super::{banner_stack::BannerStack, CachedLayout};
use crate::{dispatcher::Dispatcher, managers::window_manager::banner_stack::Banner, EglState};
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
    context::{Context, DebugOptions, Tick, WidgetTreeCreation},
    events::{MouseButton, RawEvent, RawEventKind},
    make_widget,
    stage::{draw::Drawer, measure::Constraints},
    types::{extent::Extent, offset::Offset, InputBehavior, Point, Spacing},
    widget::FlexBox,
    WidgetSystem,
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
    widget_system: WidgetSystem,

    actual_size: Extent<usize>,
    anchor: Anchor,
    margin: Margin,

    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    egl_window: Option<WlEglSurface>,
    egl_surface: Option<khronos_egl::Surface>,
    egl_state: EglState,

    gr_context: skia_safe::gpu::DirectContext,
    config: Data<Config, Borrowed>,
    #[allow(unused)]
    cached_layouts: Data<CachedData<PathBuf, CachedLayout>, Borrowed>,

    pointer: WlPointer,
    cursor_device: WpCursorShapeDeviceV1,
    mouse_position: Point<f32>,
    // pointer_state: PointerState,
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
    NotConfigured,
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

        // TODO: create styles for the widget system

        let egl_state: &EglState = gpu.as_ref();
        let gr_context: &DirectContext = gpu.as_ref();
        let mut state = WindowState {
            banner_stack: BannerStack::new(),
            widget_system: WidgetSystem::new(Context::new(font_collection)),

            actual_size,
            anchor,
            margin,

            surface,
            layer_surface,
            egl_window: None,
            egl_surface: None,
            egl_state: egl_state.clone(),

            gr_context: gr_context.clone(),
            config: config.clone(),
            cached_layouts: cached_layouts.clone(),

            cursor_device,
            pointer,
            mouse_position: Point::default(),

            has_requested_frame: false,
            last_presented_time_ns: None,
            configuration_state: ConfigurationState::NotConfigured,
        };

        while let ConfigurationState::NotConfigured = state.configuration_state {
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

        for (local_coord, extent) in self.widget_system.collect_input_regions() {
            region.add(
                local_coord.x as i32,
                local_coord.y as i32,
                extent.width as i32,
                extent.height as i32,
            );
        }

        self.state.surface.set_input_region(Some(&region));
        region.destroy();
    }

    /// Requests a frame for the current `Window` from the Wayland compositor.
    /// This allows the compositor to schedule the redraw at the correct time, ensuring smooth
    /// rendering with VSync.
    pub(super) fn frame(&mut self) {
        if self.egl_surface.is_some() {
            self.state.surface.damage(0, 0, i32::MAX, i32::MAX);
            self.state.surface.frame(&self.event_queue.handle(), ());
            self.state.has_requested_frame = true;
            debug!("Window: Requested a frame to the Wayland compositor");
        }
    }

    /// Sends a `commit` message to the Wayland compositor to apply previously requested actions.
    pub(super) fn commit(&self, wp_presentation: &wp_presentation::WpPresentation) {
        self.state.surface.commit();

        if self.egl_surface.is_some() {
            wp_presentation.feedback(&self.state.surface, &self.event_queue.handle(), ());
        }
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
        self.widget_system.update_debug_options(DebugOptions {
            show_layout_bounds: config.general().debug.show_layout_bounds,
        });

        self.relocate(config.general().offset, &config.general().anchor);
        self.banner_stack
            .configure(&config, &mut self.widget_system.context);
        self.config = config.clone();

        self.rebuild_widget_tree();

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
            &self.config,
            &mut self.widget_system.context,
        );
        self.rebuild_widget_tree();
    }

    pub(super) fn replace_by_indices(&mut self, notifications: &mut VecDeque<Notification>) {
        self.banner_stack.replace_by_keys(
            notifications,
            &self.config,
            &mut self.widget_system.context,
        );
        self.rebuild_widget_tree();
    }

    pub(super) fn close_banners_by_id(&mut self, notification_indices: &[u32]) {
        for notification_id in notification_indices {
            if let Some(banner) = self.banner_stack.get_mut(notification_id) {
                banner.close(ClosingReason::CallCloseNotification)
            }
        }
        self.rebuild_widget_tree();
    }

    pub(super) fn remove_closed_banners(&mut self) -> Vec<(Notification, ClosingReason)> {
        let closed_banners = self.banner_stack.remove_closed(&self.widget_system.context);
        self.rebuild_widget_tree();

        closed_banners
    }

    pub(super) fn reset_timeouts(&mut self) {
        self.banner_stack
            .banners_mut()
            .for_each(|banner| banner.reset_timeout(&mut self.widget_system.context));
    }

    fn rebuild_widget_tree(&mut self) {
        let context = &mut self.widget_system.context;
        let flexbox = make_widget!(context <== FlexBox(
            direction: widgets::types::Direction::Vertical,
            input_behavior: InputBehavior::PassesToChildren,
            margin: self.margin.into(),
        ));

        let iterator = |banner: &Banner| {
            let banner_widget_subtree = banner.build_widget_tree(context, &self.config);
            context.append_child(flexbox, banner_widget_subtree);
        };

        if self.config.general().anchor.is_top() {
            self.banner_stack.banners().for_each(iterator);
        } else {
            self.banner_stack.banners().rev().for_each(iterator);
        }

        context.set_pending_root(flexbox);
        self.widget_system
            .set_constraints(Constraints::new_soft(Extent::new_square(f32::MAX)));
        self.widget_system.update();
    }

    fn draw_surface(&mut self) {
        self.has_requested_frame = false;

        self.use_current_egl_surface()
            .expect("The EGL surface must be available to make current and use it");

        let new_size = Extent::new(
            self.widget_system.width() as usize,
            self.widget_system.height() as usize,
        );
        self.resize(new_size);

        let mut sk_surface = self
            .create_drawing_surface()
            .expect("The skia's surface must be correct and created without issues");
        sk_surface.canvas().clear(Color::from_argb(0, 0, 0, 0));

        let mut drawer = Drawer::use_surface(sk_surface.clone());
        self.widget_system.draw(&Offset::default(), &mut drawer);

        self.gr_context
            .flush_and_submit_surface(&mut sk_surface, skia_safe::gpu::SyncCpu::No);

        if let Some(egl_surface) = self.egl_surface {
            self.egl_state
                .instance
                .swap_interval(self.egl_state.display, 0)
                .and_then(|_| {
                    self.egl_state
                        .instance
                        .swap_buffers(self.egl_state.display, egl_surface)
                })
                .expect("The buffer swapping must be errorless");
        }
    }

    fn use_current_egl_surface(&self) -> anyhow::Result<()> {
        if let Err(err) = self.egl_state.instance.make_current(
            self.egl_state.display,
            self.egl_surface,
            self.egl_surface,
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
            self.actual_size = logical_size;
        }

        let Extent { width, height } = self.actual_size;
        self.layer_surface.set_size(width as u32, height as u32);
        let (dx, dy) = (0, 0);
        if let Some(egl_window) = &self.egl_window {
            egl_window.resize(width as i32, height as i32, dx, dy);
        }

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
        if let Some(egl_surface) = self.egl_surface {
            if let Err(err) = self
                .egl_state
                .instance
                .destroy_surface(self.egl_state.display, egl_surface)
            {
                error!("Failed to destroy EGL surface! Further application work won't guaranteed to be normal! Error: {err}.")
            }
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
#[derive(Clone, Copy)]
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
}

impl From<Margin> for Spacing {
    fn from(value: Margin) -> Self {
        Spacing {
            top: value.top,
            right: value.right,
            bottom: value.bottom,
            left: value.left,
        }
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
            state.draw_surface();
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

                    state.widget_system.tick(delta_time_ns as u128);
                    state.banner_stack.banners_mut().for_each(|banner| banner.update(&mut state.widget_system.context));

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
        const LEFT_BTN: u32 = 272;
        const RIGHT_BTN: u32 = 273;
        const MIDDLE_BTN: u32 = 274;

        fn get_button(button: u32) -> Option<MouseButton> {
            match button {
                LEFT_BTN => Some(MouseButton::Left),
                RIGHT_BTN => Some(MouseButton::Right),
                MIDDLE_BTN => Some(MouseButton::Right),
                _ => None,
            }
        }

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

                state.mouse_position = Point {
                    x: surface_x as f32,
                    y: surface_y as f32,
                };

                state.widget_system.dispatch_event(RawEvent {
                    kind: RawEventKind::MouseMove,
                    local_coord: state.mouse_position,
                });
            }
            wl_pointer::Event::Leave { serial, .. } => {
                state
                    .cursor_device
                    .set_shape(serial, wp_cursor_shape_device_v1::Shape::Default);

                state.widget_system.dispatch_event(RawEvent {
                    kind: RawEventKind::MouseLeave,
                    local_coord: Point::default(),
                });
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.mouse_position = Point {
                    x: surface_x as f32,
                    y: surface_y as f32,
                };

                state.widget_system.dispatch_event(RawEvent {
                    kind: RawEventKind::MouseMove,
                    local_coord: state.mouse_position,
                });
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(ButtonState::Pressed),
                ..
            } => {
                if let Some(kind) = get_button(button).map(RawEventKind::MouseDown) {
                    state.widget_system.dispatch_event(RawEvent {
                        kind,
                        local_coord: state.mouse_position,
                    });
                }
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(ButtonState::Released),
                ..
            } => {
                if let Some(kind) = get_button(button).map(RawEventKind::MouseUp) {
                    state.widget_system.dispatch_event(RawEvent {
                        kind,
                        local_coord: state.mouse_position,
                    });
                }
            }
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

            if state.egl_surface.is_none() {
                let (egl_window, egl_surface) =
                    Window::make_egl_surface(&state.surface, &state.egl_state, &state.actual_size)
                        .unwrap();

                state.egl_window = Some(egl_window);
                state.egl_surface = Some(egl_surface);
                state.draw_surface();
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
