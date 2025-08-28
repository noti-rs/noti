use super::{
    banner_stack::{Banner, BannerStack},
    CachedLayout,
};
use crate::{dispatcher::Dispatcher, EglState};
use config::{self, Config};
use dbus::{actions::Signal, notification::Notification};
use log::{debug, error, trace};
use widgets::types::{Offset, RectSize};
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
        wl_seat::WlSeat,
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_egl::WlEglSurface;
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, ZwlrLayerSurfaceV1},
};

pub(super) struct Window {
    event_queue: EventQueue<WindowState>,
    state: WindowState,
}

pub(super) struct WindowState {
    banner_stack: BannerStack<u32>,

    rect_size: RectSize<usize>,
    anchored_margin: AnchoredMargin,

    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    egl_window: WlEglSurface,
    egl_surface: khronos_egl::Surface,
    egl_state: EglState,

    font_collection: skia_safe::textlayout::FontCollection,
    gr_context: skia_safe::gpu::DirectContext,
    config: Data<Config, Borrowed>,
    cached_layouts: Data<CachedData<PathBuf, CachedLayout>, Borrowed>,

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
        let mut event_queue = wayland_connection.new_event_queue();

        let rect_size = RectSize::new(
            config.general().width.into(),
            config.general().height.into(),
        );

        let (surface, layer_surface) = Self::make_surface(protocols, &event_queue.handle());
        let (pointer, cursor_device) = Self::make_pointer(protocols, &event_queue.handle());
        let (egl_window, egl_surface) = Self::make_egl_surface(&surface, gpu, &rect_size)?;

        let anchored_margin = Self::make_anchored_margin(&config);
        anchored_margin.relocate_layer_surface(&layer_surface);

        layer_surface.set_size(rect_size.width as u32, rect_size.height as u32);

        surface.commit();

        let egl_state: &EglState = gpu.as_ref();
        let gr_context: &DirectContext = gpu.as_ref();
        let mut state = WindowState {
            banner_stack: BannerStack::new(),

            rect_size,
            anchored_margin,

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

            configuration_state: ConfigurationState::NotConfiured,
        };

        while let ConfigurationState::NotConfiured = state.configuration_state {
            event_queue.blocking_dispatch(&mut state)?;
        }

        debug!("Window: Initialized");

        Ok(Self { event_queue, state })
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
            env!("APP_NAME").to_string(),
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

    fn make_egl_surface<Gpu>(
        surface: &WlSurface,
        gpu: &Gpu,
        rect_size: &RectSize<usize>,
    ) -> anyhow::Result<(WlEglSurface, khronos_egl::Surface)>
    where
        Gpu: AsRef<EglState>,
    {
        let egl_window = WlEglSurface::new(
            surface.id(),
            rect_size.width as i32,
            rect_size.height as i32,
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

    pub(super) fn frame(&mut self) {
        self.state.surface.damage(0, 0, i32::MAX, i32::MAX);
        self.state.surface.frame(&self.event_queue.handle(), ());

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
    pub(super) fn reconfigure(&mut self, config: Data<Config, Borrowed>) {
        self.relocate(config.general().offset, &config.general().anchor);
        self.banner_stack.configure(&config);
        self.config = config.clone();

        debug!("Window: Reconfigured by updated config");
    }

    fn relocate(&mut self, (x, y): (u8, u8), anchor_cfg: &config::general::Anchor) {
        self.anchored_margin.update(x as i32, y as i32, anchor_cfg);
        self.anchored_margin
            .relocate_layer_surface(&self.layer_surface);
    }

    pub(super) fn total_banners(&self) -> usize {
        self.banner_stack.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.banner_stack.is_empty()
    }

    pub(super) fn add_banners(&mut self, notifications: Vec<Notification>) {
        self.banner_stack
            .extend_from(notifications.into_iter(), &self.config);
    }

    pub(super) fn replace_by_indices(&mut self, notifications: &mut VecDeque<Notification>) {
        self.banner_stack
            .replace_by_keys(notifications, &self.config);
    }

    pub(super) fn remove_banners_by_id(
        &mut self,
        notification_indices: &[u32],
    ) -> Vec<Notification> {
        self.banner_stack.remove_by_keys(notification_indices)
    }

    pub(super) fn remove_expired_banners(&mut self) -> Vec<Notification> {
        self.banner_stack.remove_expired(&self.config)
    }

    pub(super) fn handle_hover(&mut self) {
        if let Some(index) = self.get_hovered_banner() {
            self.banner_stack[&index].reset_timeout();

            // INFO: because of every tracking pointer position, it emits very frequently and it's
            // annoying. So moved to 'TRACE' level for specific situations.
            trace!("Window: Updated timeout of hovered notification banner with id {index}");
        }
    }

    pub(super) fn reset_timeouts(&mut self) {
        self.banner_stack
            .banners_mut()
            .for_each(Banner::reset_timeout);
    }

    pub(super) fn handle_click(&mut self) -> Option<Signal> {
        if let PrioritiedPressState::Unpressed = self.pointer_state.press_state {
            return None;
        }
        let _press_state = self.pointer_state.press_state.take();

        if let Some(id) = self.get_hovered_banner() {
            if self.config.general().anchor.is_bottom() {
                self.pointer_state.y -=
                    self.banner_stack[&id].height() as f64 + self.config.general().gap as f64;

                // INFO: the compositor may wrongly relocate to previous position and it will cause
                // of incorrect pointer positioning for next click in row. So need to ignore and
                // left remaining.
                self.pointer_state.ignore_first_relocate();
            }

            debug!("Window: Clicked to notification banner with id {id}");

            return self.banner_stack.remove(id).map(|notification| {
                let notification_id = notification.id;
                Signal::NotificationClosed {
                    notification_id,
                    reason: dbus::actions::ClosingReason::DismissedByUser,
                }
            });
        }

        None
    }

    fn get_hovered_banner(&self) -> Option<u32> {
        if !self.pointer_state.entered {
            return None;
        }

        let mut offset = 0.0;
        let gap = self.config.general().gap as f64;

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

        if self.config.general().anchor.is_top() {
            self.banner_stack.banners().find_map(finder)
        } else {
            self.banner_stack.banners().rev().find_map(finder)
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
            (self.rect_size.width as i32, self.rect_size.height as i32),
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

    fn resize(&mut self, rect_size: RectSize<usize>) {
        self.rect_size = rect_size;

        let (width, height) = (self.rect_size.width, self.rect_size.height);
        self.layer_surface.set_size(width as u32, height as u32);
        let (dx, dy) = (0, 0);
        self.egl_window.resize(width as i32, height as i32, dx, dy);

        debug!(
            "Window: Resized to width - {}, height - {}",
            self.rect_size.width, self.rect_size.height
        );
    }

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

        if let Err(_err) = self.sync() {
            error!("Window: Failed to sync during deinitialization.")
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
    ignore_first_relocate: bool,
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

    fn ignore_first_relocate(&mut self) {
        self.ignore_first_relocate = true;
    }

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
        if self.ignore_first_relocate {
            debug!("Pointer: Forced to ignore first relocate.");

            self.ignore_first_relocate = false;
            return;
        }

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
            state
                .use_current_egl_surface()
                .expect("The EGL surface must be available to make current and use it");

            state.banner_stack.banners_mut().for_each(|banner| {
                banner.compile(
                    &state.config,
                    state.font_collection.clone(),
                    &state.cached_layouts,
                )
            });

            let gap = state.config.general().gap as usize;
            state.resize(RectSize::new(
                state.banner_stack.width(),
                state.banner_stack.total_height_with_gap(gap),
            ));

            let mut sk_surface = state
                .create_drawing_surface()
                .expect("The skia's surface must be correct and created without issues");
            sk_surface.canvas().clear(Color::from_argb(0, 0, 0, 0));

            let mut offset = Offset::default();
            let writer = |banner: &Banner| {
                banner.draw(&offset, &mut sk_surface);
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
