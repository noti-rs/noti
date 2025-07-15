use config::{
    display::{Border, DisplayConfig},
    Config,
};
use dbus::notification::Notification;
use log::{debug, error, trace};
use render::{
    drawer::Drawer,
    types::RectSize,
    widget::{
        self, Alignment, Draw, FlexContainerBuilder, Position, WImage, WText, WTextKind, Widget,
        WidgetConfiguration,
    },
    PangoContext,
};
use shared::cached_data::CachedData;
use std::{path::PathBuf, time};

use super::CachedLayout;

pub(super) struct Banner {
    data: Notification,
    layout: Option<Widget>,
    created_at: time::Instant,

    framebuffer: Vec<u8>,
}

impl Banner {
    pub(super) fn init(notification: Notification) -> Self {
        debug!("Banner (id={}): Created", notification.id);

        Self {
            data: notification,
            layout: None,
            created_at: time::Instant::now(),

            framebuffer: vec![],
        }
    }

    pub(super) fn notification(&self) -> &Notification {
        &self.data
    }

    pub(super) fn destroy_and_get_notification(self) -> Notification {
        debug!("Banner (id={}): Destroyed", self.data.id);
        self.data
    }

    pub(super) fn created_at(&self) -> &time::Instant {
        &self.created_at
    }

    pub(super) fn reset_timeout(&mut self) {
        self.created_at = time::Instant::now();

        trace!("Banner (id={}): Timeout reset", self.data.id);
    }

    pub(super) fn update_data(&mut self, notification: Notification) {
        self.data = notification;
        self.created_at = time::Instant::now();
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.data.id
        );
    }

    // TODO: use it for resize
    #[allow(unused)]
    pub(super) fn width(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.width())
            .unwrap_or_default()
    }

    pub(super) fn height(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.height())
            .unwrap_or_default()
    }

    #[inline]
    pub(super) fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    #[inline]
    pub(super) fn take_framebuffer(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.framebuffer)
    }

    #[inline]
    pub(super) fn set_framebuffer(&mut self, framebuffer: Vec<u8>) {
        self.framebuffer = framebuffer
    }

    pub(super) fn draw(
        &mut self,
        pango_context: &PangoContext,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) -> DrawState {
        debug!("Banner (id={}): Beginning of draw", self.data.id);

        let rect_size = RectSize::new(
            config.general().width as usize,
            config.general().height as usize,
        );

        let display = config.display_by_app(&self.data.app_name);
        let mut drawer = match Drawer::create(rect_size) {
            Ok(drawer) => drawer,
            Err(err) => {
                error!("Failed to create drawer for Banner(id={}), avoided to draw banner. Error: {err}", self.data.id);
                return DrawState::Failure;
            }
        };

        let mut layout = match &display.layout {
            config::display::Layout::Default => Self::default_layout(display),
            config::display::Layout::FromPath { path_buf } => cached_layouts
                .get(path_buf)
                .and_then(CachedLayout::layout)
                .cloned()
                .unwrap_or_else(|| Self::default_layout(display)),
        };

        layout.compile(
            rect_size,
            &WidgetConfiguration {
                display_config: display,
                theme: config.theme_by_app(&self.data.app_name),
                notification: &self.data,
                pango_context,
                override_properties: display.layout.is_default(),
            },
        );

        if let Err(err) = layout.draw(pango_context, &mut drawer) {
            error!(
                "Failed to draw banner Banner(id={}). Error: {err}",
                self.data.id
            );
            return DrawState::Failure;
        }

        self.layout = Some(layout);
        self.framebuffer = match drawer.try_into() {
            Ok(val) => val,
            Err(err) => {
                error!(
                    "Failed to get data after drawing Banner(id={}). Error: {err}",
                    self.data.id
                );
                return DrawState::Failure;
            }
        };

        debug!("Banner (id={}): Complete draw", self.data.id);
        DrawState::Success
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
                    .transparent_background(true)
                    .children(vec![
                        WText::new(WTextKind::Summary).into(),
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

impl<'a> From<&'a Banner> for &'a Notification {
    fn from(value: &'a Banner) -> Self {
        &value.data
    }
}

pub(super) enum DrawState {
    Success,
    Failure,
}
