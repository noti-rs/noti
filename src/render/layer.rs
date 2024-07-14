use std::{fs::File, io::Write, os::fd::AsFd, sync::Arc};

use crate::{
    config::{self, CONFIG},
    data::{
        aliases::Result,
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
    Connection, Dispatch, EventQueue, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, Anchor},
};

use super::{
    color::Bgra,
    font::FontCollection,
    image::Image,
    text::{self, TextRect},
};

pub(crate) struct NotificationStack {
    connection: Connection,
    font_collection: Arc<FontCollection>,

    stack: Vec<(EventQueue<NotificationRect>, NotificationRect)>,
}

impl NotificationStack {
    pub(crate) fn init() -> Result<Self> {
        let connection = Connection::connect_to_env()?;
        let font_collection = Arc::new(FontCollection::load_by_font_name(
            CONFIG.general().font().name(),
        )?);

        Ok(Self {
            connection,
            font_collection,
            stack: vec![],
        })
    }

    pub(crate) fn create_notification_rect(&mut self, notification: Notification) {
        let event_queue = self.connection.new_event_queue();
        let qh = event_queue.handle();
        let display = self.connection.display();

        display.get_registry(&qh, ());
        let mut notification_rect = NotificationRect::init(notification);
        notification_rect.font_collection = Some(self.font_collection.clone());

        self.stack.push((event_queue, notification_rect));
    }

    pub(crate) fn dispatch(&mut self) {
        self.stack
            .iter_mut()
            .for_each(|(event_queue, notification_rect)| {
                let dispatched_count = event_queue
                    .dispatch_pending(notification_rect)
                    .expect("Successful dispatch");

                if dispatched_count > 0 {
                    return;
                }

                event_queue.flush().expect("Successful event queue flush");
                let guard = event_queue.prepare_read().expect("Get read events guard");
                let _ = guard.read();
            });
    }
}

struct NotificationRect {
    width: i32,
    height: i32,
    margin: Margin,

    data: Notification,
    font_collection: Option<Arc<FontCollection>>,

    shm: Option<wl_shm::WlShm>,
    buffer: Option<wl_buffer::WlBuffer>,

    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,

    configured: bool,
    alive: bool,

    pointer_state: PointerState,
}

impl NotificationRect {
    fn init(notification: Notification) -> Self {
        let general_cfg = CONFIG.general();
        Self {
            width: general_cfg.width() as i32,
            height: general_cfg.height() as i32,
            margin: Margin::new(),
            data: notification,
            font_collection: None,

            shm: None,
            buffer: None,

            surface: None,
            layer_surface: None,

            configured: false,
            alive: true,

            pointer_state: Default::default(),
        }
    }

    fn draw(&self, tmp: &mut File) {
        let display = CONFIG.display_by_app(&self.data.app_name);
        let colors = match self.data.hints.urgency {
            notification::Urgency::Low => display.colors().low(),
            notification::Urgency::Normal => display.colors().normal(),
            notification::Urgency::Critical => display.colors().critical(),
        };

        let background: Bgra = colors.background().into();
        let foreground: Bgra = colors.foreground().into();

        let mut buf: Vec<u8> = vec![background.clone(); self.width as usize * self.height as usize]
            .into_iter()
            .flat_map(|bgra| bgra.to_slice())
            .collect();

        let border_cfg = display.border();
        let stride = self.width as usize * 4;

        let border = BorderBuilder::default()
            .width(border_cfg.size() as usize)
            .radius(display.rounding() as usize)
            .color(border_cfg.color().into())
            .background_color(background.clone())
            .frame_width(self.width as usize)
            .frame_height(self.height as usize)
            .build()
            .unwrap();

        border.draw(|x, y, bgra| unsafe {
            let position = y * stride + x * 4;
            *TryInto::<&mut [u8; 4]>::try_into(&mut buf[position..position + 4])
                .unwrap_unchecked() = bgra.to_slice()
        });

        let padding = display.padding() as usize + border_cfg.size() as usize;

        let image =
            Image::from_image_data(self.data.hints.image_data.as_ref(), display.image_size())
                .or_svg(self.data.hints.image_path.as_deref(), display.image_size());

        // INFO: img_width is need for further render (Summary and Text rendering)
        let mut img_width = image.width();
        let img_height = image.height();

        if img_height
            .is_some_and(|heigth| heigth <= self.height as usize - border_cfg.size() as usize * 2)
        {
            let y_offset = img_height.map(|height| self.height as usize / 2 - height / 2);

            image.draw(
                padding,
                y_offset.unwrap_or_default(),
                stride,
                |position, bgra| unsafe {
                    *TryInto::<&mut [u8; 4]>::try_into(&mut buf[position..position + 4])
                        .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
                },
            );
        } else {
            eprintln!("Image height exceeds the possible height!\n\
                Please set a higher value of height or decrease the value of image_size in config.toml.");
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
            self.width as usize
                - img_width
                    .map(|width| width + padding * 2)
                    .unwrap_or_default(),
            self.height as usize,
            display.title().alignment(),
            |x, y, bgra| {
                let position = (y * stride as isize + x_offset as isize + x * 4) as usize;
                unsafe {
                    *TryInto::<&mut [u8; 4]>::try_into(&mut buf[position..position + 4])
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
            self.width as usize
                - img_width
                    .map(|width| width + padding * 2)
                    .unwrap_or_default(),
            self.height as usize - y_offset,
            display.body().alignment(),
            |x, y, bgra| {
                let position = ((y + y_offset as isize) * stride as isize
                    + x_offset as isize
                    + x * 4) as usize;
                unsafe {
                    *TryInto::<&mut [u8; 4]>::try_into(&mut buf[position..position + 4])
                        .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
                }
            },
        );

        tmp.write_all(&buf).unwrap();
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
            right: 25,
            top: 25,
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
}

impl Dispatch<wl_registry::WlRegistry, ()> for NotificationRect {
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
            match &interface[..] {
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

delegate_noop!(NotificationRect: ignore wl_compositor::WlCompositor);
delegate_noop!(NotificationRect: ignore wl_surface::WlSurface);
delegate_noop!(NotificationRect: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(NotificationRect: ignore wl_shm_pool::WlShmPool);
delegate_noop!(NotificationRect: ignore wl_buffer::WlBuffer);

impl Dispatch<wl_shm::WlShm, ()> for NotificationRect {
    fn event(
        state: &mut Self,
        shm: &wl_shm::WlShm,
        _event: <wl_shm::WlShm as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        let mut file = tempfile::tempfile().unwrap();
        state.draw(&mut file);
        let pool = shm.create_pool(file.as_fd(), state.width * state.height * 4, qhandle, ());
        state.buffer = Some(pool.create_buffer(
            0,
            state.width,
            state.height,
            state.width * 4,
            wl_shm::Format::Argb8888,
            qhandle,
            (),
        ));
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for NotificationRect {
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

impl Dispatch<wl_pointer::WlPointer, ()> for NotificationRect {
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
            } => {
                const LEFT_BTN: u32 = 272;
                const RIGHT_BTN: u32 = 273;
                const MIDDLE_BTN: u32 = 274;

                match button {
                    LEFT_BTN => {
                        println!("Pressed!")
                    }
                    RIGHT_BTN => {
                        state
                            .layer_surface
                            .as_ref()
                            .unwrap()
                            .set_margin(175, 25, 0, 0);
                        state.surface.as_ref().unwrap().commit();
                    }
                    MIDDLE_BTN => (),
                    _ => (),
                }
            }
            // wl_pointer::Event::Frame => todo!(),
            _ => (),
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for NotificationRect {
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

            if let Some(surface) = state.surface.as_ref() {
                surface.attach(state.buffer.as_ref(), 0, 0);
                surface.commit();
            }
        }
    }
}
