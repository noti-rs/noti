use std::{fs::File, io::Write, os::fd::AsFd, sync::Arc, time};

use crate::{
    config::{self, CONFIG},
    data::{
        aliases::Result,
        internal_messages::RendererMessage,
        notification::{self, Notification},
    },
    render::border::BorderBuilder,
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

use super::{color::Bgra, font::FontCollection, image::Image, text::TextRect};

pub(crate) struct NotificationStack {
    connection: Connection,
    event_queue: Option<EventQueue<Window>>,
    qhandle: Option<QueueHandle<Window>>,
    window: Option<Window>,

    font_collection: Arc<FontCollection>,

    stack: Vec<NotificationRect>,
    events: Vec<RendererMessage>,
}

impl NotificationStack {
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

            stack: vec![],
            events: vec![],
        })
    }

    pub(crate) fn create_notifications(&mut self, mut notifications: Vec<Notification>) {
        let init = self.init_window();

        self.replace_by_indices(&mut notifications);

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

        let mut rects: Vec<NotificationRect> = notifications
            .into_iter()
            .map(|notification| {
                let mut notification_rect = NotificationRect::init(notification);
                notification_rect.font_collection = Some(self.font_collection.clone());
                notification_rect
            })
            .collect();

        if !init {
            let height = CONFIG.general().height() as i32;
            window.height += height * rects.len() as i32;
        }

        let mut file = tempfile::tempfile().unwrap();
        let anchor = CONFIG.general().anchor();

        if anchor.is_top() {
            rects.iter_mut().for_each(|rect| {
                rect.draw();
                rect.write_to_file(&mut file);
            });
            Self::write_stack_to_file(&self.stack, anchor, &mut file);
        } else {
            Self::write_stack_to_file(&self.stack, anchor, &mut file);
            rects.iter_mut().rev().for_each(|rect| {
                rect.draw();
                rect.write_to_file(&mut file);
            });
        }

        window.create_buffer(file, qhandle);
        window.resize_layer_surface();

        self.full_commit();

        self.stack.extend(rects.into_iter().rev());
    }

    pub(crate) fn close_notifications(&mut self, indices: &[u32]) {
        let indices_to_remove: Vec<usize> = self
            .stack
            .iter()
            .enumerate()
            .filter(|(_, notification_rect)| indices.contains(&notification_rect.data.id))
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
        if self.stack.is_empty() {
            return;
        }

        let indices_to_remove: Vec<usize> = self
            .stack
            .iter()
            .enumerate()
            .filter_map(|(i, rect)| match &rect.data.expire_timeout {
                notification::Timeout::Millis(millis) => {
                    if rect.created_at.elapsed().as_millis() > *millis as u128 {
                        Some(i)
                    } else {
                        None
                    }
                }
                notification::Timeout::Never => None,
                notification::Timeout::Configurable => {
                    let timeout = CONFIG.display_by_app(&rect.data.app_name).timeout();
                    if timeout != 0 && rect.created_at.elapsed().as_millis() > timeout as u128 {
                        Some(i)
                    } else {
                        None
                    }
                }
            })
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
                let count = window.height as usize / rect_height;

                if let Some((i, _)) = (0..window.height as usize)
                    .step_by(rect_height)
                    .enumerate()
                    .take(count)
                    .find(|&(_, height)| {
                        (height..height + rect_height).contains(&(window.pointer_state.y as usize))
                    })
                {
                    let notifications = self.remove_rects(&[count - i - 1]);
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
                self.stack
                    .iter()
                    .position(|rect| rect.data.id == notification.id)
                    .map(|stack_index| (i, stack_index))
            })
            .collect();

        for (notification_index, stack_index) in matching_indices.into_iter().rev() {
            let notification = notifications.remove(notification_index);
            let rect = &mut self.stack[stack_index];
            rect.update_data(notification);
            rect.draw();
        }
    }

    fn remove_rects(&mut self, indices_to_remove: &[usize]) -> Vec<Notification> {
        let notifications = indices_to_remove
            .iter()
            .map(|id| self.stack.remove(*id).data)
            .collect();

        if self.stack.len() == 0 {
            self.deinit_window();
            return notifications;
        }

        let mut file = tempfile::tempfile().unwrap();
        Self::write_stack_to_file(&self.stack, CONFIG.general().anchor(), &mut file);

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        window.height -= CONFIG.general().height() as i32 * notifications.len() as i32;

        let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };
        window.create_buffer(file, qhandle);
        window.resize_layer_surface();

        self.full_commit();

        notifications
    }

    fn write_stack_to_file(stack: &[NotificationRect], anchor: &config::Anchor, file: &mut File) {
        if anchor.is_top() {
            stack.iter()
        } else {
            stack.iter()
        }
        .rev()
        .for_each(|rect| rect.write_to_file(file));
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

struct NotificationRect {
    data: Notification,
    created_at: time::Instant,
    framebuffer: Vec<u8>,
    font_collection: Option<Arc<FontCollection>>,
}

impl NotificationRect {
    fn init(notification: Notification) -> Self {
        Self {
            data: notification,
            created_at: time::Instant::now(),
            framebuffer: vec![],
            font_collection: None,
        }
    }

    fn draw(&mut self) {
        let (width, height) = (
            CONFIG.general().width() as i32,
            CONFIG.general().height() as i32,
        );

        let display = CONFIG.display_by_app(&self.data.app_name);
        let colors = match self.data.hints.urgency {
            notification::Urgency::Low => display.colors().low(),
            notification::Urgency::Normal => display.colors().normal(),
            notification::Urgency::Critical => display.colors().critical(),
        };

        let background: Bgra = colors.background().into();
        let foreground: Bgra = colors.foreground().into();

        self.framebuffer = vec![background.clone(); width as usize * height as usize]
            .into_iter()
            .flat_map(|bgra| bgra.to_slice())
            .collect();

        let border_cfg = display.border();
        let stride = width as usize * 4;

        let border = BorderBuilder::default()
            .size(border_cfg.size() as usize)
            .radius(border_cfg.radius() as usize)
            .color(border_cfg.color().into())
            .background_color(background.clone())
            .frame_width(width as usize)
            .frame_height(height as usize)
            .build()
            .unwrap();

        border.draw(|x, y, bgra| unsafe {
            let position = y * stride + x * 4;
            *TryInto::<&mut [u8; 4]>::try_into(&mut self.framebuffer[position..position + 4])
                .unwrap_unchecked() = bgra.to_slice()
        });

        let padding = display.padding() as usize + border_cfg.size() as usize;

        let image =
            Image::from_image_data(self.data.hints.image_data.as_ref(), display.image_size())
                .or_svg(self.data.hints.image_path.as_deref(), display.image_size());

        // INFO: img_width is need for further render (Summary and Text rendering)
        let mut img_width = image.width();
        let img_height = image.height();

        if img_height.is_some_and(|img_height| {
            img_height <= height as usize - border_cfg.size() as usize * 2
        }) {
            let y_offset = img_height.map(|img_height| height as usize / 2 - img_height / 2);

            image.draw(
                padding,
                y_offset.unwrap_or_default(),
                stride,
                |position, bgra| unsafe {
                    *TryInto::<&mut [u8; 4]>::try_into(
                        &mut self.framebuffer[position..position + 4],
                    )
                    .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
                },
            );
        } else {
            eprintln!(
                "Image height exceeds the possible height!\n\
                Please set a higher value of height or decrease the value of image_size in config.toml."
            );
            img_width = None;
        }

        let font_size = CONFIG.general().font().size() as f32;

        let mut summary = TextRect::from_str(
            &self.data.summary,
            font_size,
            self.font_collection.as_ref().cloned().unwrap(),
        );

        let x_offset = img_width
            .map(|width| (width + padding * 2) * 4)
            .unwrap_or_default();
        summary.set_padding(padding);
        summary.set_line_spacing(display.title().line_spacing() as usize);
        summary.set_foreground(foreground.clone());

        let y_offset = summary.draw(
            width as usize
                - img_width
                    .map(|width| width + padding * 2)
                    .unwrap_or_default(),
            height as usize,
            display.title().alignment(),
            |x, y, bgra| {
                let position = (y * stride as isize + x_offset as isize + x * 4) as usize;
                unsafe {
                    *TryInto::<&mut [u8; 4]>::try_into(
                        &mut self.framebuffer[position..position + 4],
                    )
                    .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
                }
            },
        );

        let mut text = if display.markup() {
            TextRect::from_text(
                &self.data.body,
                font_size,
                self.font_collection.as_ref().cloned().unwrap(),
            )
        } else {
            TextRect::from_str(
                &self.data.body.body,
                font_size,
                self.font_collection.as_ref().cloned().unwrap(),
            )
        };

        text.set_padding(padding);
        text.set_line_spacing(display.body().line_spacing() as usize);
        text.set_foreground(foreground);
        text.draw(
            width as usize
                - img_width
                    .map(|width| width + padding * 2)
                    .unwrap_or_default(),
            height as usize - y_offset,
            display.body().alignment(),
            |x, y, bgra| {
                let position = ((y + y_offset as isize) * stride as isize
                    + x_offset as isize
                    + x * 4) as usize;
                unsafe {
                    *TryInto::<&mut [u8; 4]>::try_into(
                        &mut self.framebuffer[position..position + 4],
                    )
                    .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
                }
            },
        );
    }

    fn update_data(&mut self, notification: Notification) {
        self.data = notification;
    }

    #[inline]
    pub(crate) fn write_to_file(&self, file: &mut File) {
        file.write_all(&self.framebuffer).unwrap();
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
