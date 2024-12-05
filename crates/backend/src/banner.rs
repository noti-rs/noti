use std::time;

use config::{spacing::Spacing, Config, DisplayConfig};
use dbus::notification::Notification;
use log::{debug, trace};

use render::{
    border::BorderBuilder,
    color::Bgra,
    font::FontCollection,
    types::RectSize,
    widget::{
        self, Alignment, Coverage, Draw, DrawColor, FlexContainerBuilder, Position, WImage, WText,
        WTextKind, WidgetConfiguration,
    },
};

pub struct BannerRect {
    data: Notification,
    created_at: time::Instant,

    stride: usize,
    framebuffer: Vec<u8>,
}

impl BannerRect {
    pub(crate) fn init(notification: Notification) -> Self {
        debug!("Banner (id={}): Created", notification.id);

        Self {
            data: notification,
            created_at: time::Instant::now(),

            stride: 0,
            framebuffer: vec![],
        }
    }

    pub(crate) fn notification(&self) -> &Notification {
        &self.data
    }

    pub(crate) fn destroy_and_get_notification(self) -> Notification {
        debug!("Banner (id={}): Destroyed", self.data.id);
        self.data
    }

    pub(crate) fn created_at(&self) -> &time::Instant {
        &self.created_at
    }

    pub(crate) fn update_timeout(&mut self) {
        self.created_at = time::Instant::now();

        // INFO: because of every tracking pointer position, it emits very frequently and it's
        // annoying. So moved to 'TRACE' level for specific situations.
        trace!("Banner (id={}): Updated timeout", self.data.id);
    }

    pub(crate) fn update_data(&mut self, notification: Notification) {
        self.data = notification;
        self.created_at = time::Instant::now();
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.data.id
        );
    }

    #[inline]
    pub(crate) fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub(crate) fn draw(&mut self, font_collection: &FontCollection, config: &Config) {
        debug!("Banner (id={}): Beginning of draw", self.data.id);

        let rect_size = RectSize::new(
            config.general().width as usize,
            config.general().height as usize,
        );

        let display = config.display_by_app(&self.data.app_name);
        let colors = display.colors.by_urgency(&self.data.hints.urgency);

        let background: Bgra = Bgra::from(&colors.background);

        self.init_framebuffer(&rect_size, &background);
        self.stride = rect_size.width * 4;

        let border_spacing = Spacing::all_directional(display.border.size);
        let padding = &display.padding + border_spacing;

        let font_size = config.general().font.size as f32;

        let mut container = FlexContainerBuilder::default()
            .spacing(padding)
            .direction(widget::Direction::Horizontal)
            .alignment(Alignment::new(Position::Start, Position::Center))
            .elements(vec![
                WImage::new().into(),
                FlexContainerBuilder::default()
                    .spacing(Default::default())
                    .direction(widget::Direction::Vertical)
                    .alignment(Alignment::new(Position::Center, Position::Center))
                    .elements(vec![
                        WText::new(WTextKind::Title).into(),
                        WText::new(WTextKind::Body).into(),
                    ])
                    .build()
                    .unwrap()
                    .into(),
            ])
            .build()
            .unwrap();

        container.compile(
            rect_size,
            &WidgetConfiguration {
                display_config: display,
                notification: &self.data,
                font_collection,
                font_size,
            },
        );
        container.draw(&mut |x, y, color| {
            self.put_color_at(x, y, Self::convert_color(color, self.get_color_at(x, y)))
        });

        self.draw_border(
            config.general().width.into(),
            config.general().height.into(),
            display,
        );

        debug!("Banner (id={}): Complete draw", self.data.id);
    }

    fn init_framebuffer(&mut self, rect_size: &RectSize, background: &Bgra) {
        self.framebuffer = vec![background.clone(); rect_size.area()]
            .into_iter()
            .flat_map(|bgra| bgra.into_slice())
            .collect();

        debug!("Banner (id={}): Initialized framebuffer", self.data.id);
    }

    fn draw_border(&mut self, width: usize, height: usize, display: &DisplayConfig) {
        let border_cfg = &display.border;

        BorderBuilder::default()
            .size(border_cfg.size as usize)
            .radius(border_cfg.radius as usize)
            .color(Bgra::from(&border_cfg.color))
            .frame_width(width)
            .frame_height(height)
            .compile()
            .expect("Create Border for banner rounding")
            .draw(&mut |x, y, color| {
                self.put_color_at(x, y, Self::convert_color(color, self.get_color_at(x, y)))
            });
    }

    fn convert_color(color: DrawColor, background: Bgra) -> Bgra {
        match color {
            DrawColor::Replace(color) => color,
            DrawColor::Overlay(foreground) => foreground.overlay_on(&background),
            DrawColor::OverlayWithCoverage(foreground, Coverage(factor)) => {
                foreground.linearly_interpolate(&background, factor)
            }
            DrawColor::Transparent(Coverage(factor)) => background * factor,
        }
    }

    fn get_color_at(&self, x: usize, y: usize) -> Bgra {
        let position = y * self.stride + x * 4;
        unsafe {
            TryInto::<&[u8; 4]>::try_into(&self.framebuffer[position..position + 4])
                .unwrap_unchecked()
                .into()
        }
    }

    fn put_color_at(&mut self, x: usize, y: usize, color: Bgra) {
        let position = y * self.stride + x * 4;
        unsafe {
            *TryInto::<&mut [u8; 4]>::try_into(&mut self.framebuffer[position..position + 4])
                .unwrap_unchecked() = color.into_slice()
        }
    }
}

impl<'a> From<&'a BannerRect> for &'a Notification {
    fn from(value: &'a BannerRect) -> Self {
        &value.data
    }
}
