use std::{fs::File, io::Write, time};

use crate::{
    config::{spacing::Spacing, Alignment, Config, DisplayConfig, Position},
    data::notification::Notification,
};

use super::{
    border::BorderBuilder,
    color::Bgra,
    font::FontCollection,
    types::RectSize,
    widget::{self, ContainerBuilder, Coverage, Draw, DrawColor, WImage, WText},
};

pub struct BannerRect {
    data: Notification,
    created_at: time::Instant,

    stride: usize,
    framebuffer: Vec<u8>,
}

impl BannerRect {
    pub(crate) fn init(notification: Notification) -> Self {
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
        self.data
    }

    pub(crate) fn created_at(&self) -> &time::Instant {
        &self.created_at
    }

    pub(crate) fn update_data(&mut self, notification: Notification) {
        self.data = notification;
        self.created_at = time::Instant::now();
    }

    #[inline]
    pub(crate) fn write_to_file(&self, file: &mut File) {
        file.write_all(&self.framebuffer).unwrap();
    }

    #[inline]
    pub(crate) fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub(crate) fn draw(&mut self, font_collection: &FontCollection, config: &Config) {
        let rect_size = RectSize::new(
            config.general().width() as usize,
            config.general().height() as usize,
        );

        let display = config.display_by_app(&self.data.app_name);
        let colors = display.colors().by_urgency(&self.data.hints.urgency);

        let background: Bgra = colors.background().into();

        self.init_framebuffer(&rect_size, &background);
        self.stride = rect_size.width as usize * 4;

        let border_spacing = Spacing::all_directional(display.border().size());
        let padding = display.padding() + border_spacing;

        let font_size = config.general().font().size() as f32;

        let mut container = ContainerBuilder::default()
            .spacing(padding)
            .direction(widget::Direction::Horizontal)
            .alignment(Alignment::new(Position::Start, Position::Center))
            .elements(vec![
                WImage::new(&self.data, display).into(),
                ContainerBuilder::default()
                    .spacing(Default::default())
                    .direction(widget::Direction::Vertical)
                    .alignment(Alignment::new(Position::Center, Position::Center))
                    .elements(vec![
                        WText::new_title(&self.data, font_collection, font_size, display).into(),
                        WText::new_body(&self.data, font_collection, font_size, display).into(),
                    ])
                    .build()
                    .unwrap()
                    .into(),
            ])
            .build()
            .unwrap();

        container.compile(rect_size);
        container.draw(&mut |x, y, color| {
            self.put_color_at(x, y, Self::convert_color(color, &background))
        });

        self.draw_border(
            config.general().width().into(),
            config.general().height().into(),
            &background,
            display,
        );
    }

    fn init_framebuffer(&mut self, rect_size: &RectSize, background: &Bgra) {
        self.framebuffer = vec![background.clone(); rect_size.area()]
            .into_iter()
            .flat_map(|bgra| bgra.to_slice())
            .collect();
    }

    fn draw_border(
        &mut self,
        width: usize,
        height: usize,
        background: &Bgra,
        display: &DisplayConfig,
    ) {
        let border_cfg = display.border();

        let border = BorderBuilder::default()
            .size(border_cfg.size() as usize)
            .radius(border_cfg.radius() as usize)
            .color(border_cfg.color().into())
            .background_color(background.clone())
            .frame_width(width as usize)
            .frame_height(height as usize)
            .build()
            .unwrap();

        border.draw(&mut |x, y, color| {
            self.put_color_at(x, y, Self::convert_color(color, background))
        });
    }

    fn convert_color(color: DrawColor, background: &Bgra) -> Bgra {
        match color {
            DrawColor::Replace(color) => color,
            DrawColor::Overlay(foreground) => foreground.overlay_on(background),
            DrawColor::OverlayWithCoverage(foreground, Coverage(factor)) => {
                foreground.linearly_interpolate(background, factor)
            }
        }
    }

    fn put_color_at(&mut self, x: usize, y: usize, color: Bgra) {
        unsafe {
            let position = y * self.stride + x * 4;
            *TryInto::<&mut [u8; 4]>::try_into(&mut self.framebuffer[position..position + 4])
                .unwrap_unchecked() = color.to_slice()
        }
    }
}

impl<'a> From<&'a BannerRect> for &'a Notification {
    fn from(value: &'a BannerRect) -> Self {
        &value.data
    }
}
