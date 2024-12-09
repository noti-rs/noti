use std::time;

use config::{Border, Config, DisplayConfig};
use dbus::notification::Notification;
use log::{debug, trace};

use render::{
    color::Bgra,
    drawer::Drawer,
    font::FontCollection,
    types::RectSize,
    widget::{
        self, Alignment, Draw, FlexContainerBuilder, Position, WImage, WText, WTextKind, Widget,
        WidgetConfiguration,
    },
};

use crate::cache::{CachedLayout, CachedLayouts};

pub struct BannerRect {
    data: Notification,
    created_at: time::Instant,

    framebuffer: Vec<u8>,
}

impl BannerRect {
    pub(crate) fn init(notification: Notification) -> Self {
        debug!("Banner (id={}): Created", notification.id);

        Self {
            data: notification,
            created_at: time::Instant::now(),

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

    pub(crate) fn draw(
        &mut self,
        font_collection: &FontCollection,
        config: &Config,
        cached_layouts: &CachedLayouts,
    ) {
        debug!("Banner (id={}): Beginning of draw", self.data.id);

        let rect_size = RectSize::new(
            config.general().width as usize,
            config.general().height as usize,
        );

        let display = config.display_by_app(&self.data.app_name);
        let mut drawer = Drawer::new(Bgra::new(), rect_size.clone());

        let mut layout = match &display.layout {
            config::Layout::Default => Self::default_layout(display),
            config::Layout::FromPath { path_buf } => cached_layouts
                .get(path_buf)
                .and_then(CachedLayout::layout)
                .cloned()
                .unwrap_or_else(|| Self::default_layout(display)),
        };

        let font_size = config.general().font.size as f32;

        layout.compile(
            rect_size,
            &WidgetConfiguration {
                display_config: display,
                notification: &self.data,
                font_collection,
                font_size,
                override_properties: display.layout.is_default(),
            },
        );

        layout.draw(&mut drawer);
        self.framebuffer = drawer.into();

        debug!("Banner (id={}): Complete draw", self.data.id);
    }

    fn default_layout(display_config: &DisplayConfig) -> Widget {
        FlexContainerBuilder::default()
            .spacing(display_config.padding.clone())
            .border(display_config.border.clone())
            .direction(widget::Direction::Horizontal)
            .alignment(Alignment::new(Position::Start, Position::Center))
            .children(vec![
                WImage::new().into(),
                FlexContainerBuilder::default()
                    .spacing(Default::default())
                    .border(Border::default())
                    .direction(widget::Direction::Vertical)
                    .alignment(Alignment::new(Position::Center, Position::Center))
                    .children(vec![
                        WText::new(WTextKind::Title).into(),
                        WText::new(WTextKind::Body).into(),
                    ])
                    .build()
                    .unwrap()
                    .into(),
            ])
            .build()
            .unwrap()
            .into()
    }
}

impl<'a> From<&'a BannerRect> for &'a Notification {
    fn from(value: &'a BannerRect) -> Self {
        &value.data
    }
}
