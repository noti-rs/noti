use std::{fs::File, io::Write, sync::Arc, time};

use crate::{
    config::CONFIG,
    data::notification::{self, Notification},
};

use super::{
    border::BorderBuilder, color::Bgra, font::FontCollection, image::Image, text::TextRect,
};

pub struct BannerRect {
    data: Notification,
    created_at: time::Instant,
    framebuffer: Vec<u8>,
    font_collection: Option<Arc<FontCollection>>,
}

impl BannerRect {
    pub(crate) fn init(notification: Notification) -> Self {
        Self {
            data: notification,
            created_at: time::Instant::now(),
            framebuffer: vec![],
            font_collection: None,
        }
    }

    pub(crate) fn set_font_collection(&mut self, font_collection: Arc<FontCollection>) {
        self.font_collection = Some(font_collection);
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

    pub(crate) fn draw(&mut self) {
        let (mut width, mut height) = (
            CONFIG.general().width() as usize,
            CONFIG.general().height() as usize,
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

        let padding = display.padding();
        padding.shrink(&mut width, &mut height);

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
                padding.left() as usize,
                padding.top() as usize + y_offset.unwrap_or_default(),
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
            .map(|width| (width + padding.left() as usize) * 4)
            .unwrap_or_default();

        let title_cfg = display.title();

        summary.set_margin(title_cfg.margin());
        summary.set_line_spacing(title_cfg.line_spacing() as usize);
        summary.set_foreground(foreground.clone());
        summary.set_ellipsize_at(display.ellipsize());
        summary.compile(width - img_width.unwrap_or_default(), height);
        height -= summary.height();

        summary.draw(display.title().alignment(), |x, y, bgra| {
            let position = ((y + padding.top() as isize) * stride as isize
                + x_offset as isize
                + x * 4) as usize;
            unsafe {
                *TryInto::<&mut [u8; 4]>::try_into(&mut self.framebuffer[position..position + 4])
                    .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
            }
        });

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

        let body_cfg = display.body();

        text.set_margin(body_cfg.margin());
        text.set_line_spacing(body_cfg.line_spacing() as usize);
        text.set_foreground(foreground);
        text.set_ellipsize_at(display.ellipsize());
        text.compile(width - img_width.unwrap_or_default(), height);

        let y_offset = padding.top() as usize + summary.height();

        text.draw(display.body().alignment(), |x, y, bgra| {
            let position =
                ((y + y_offset as isize) * stride as isize + x_offset as isize + x * 4) as usize;
            unsafe {
                *TryInto::<&mut [u8; 4]>::try_into(&mut self.framebuffer[position..position + 4])
                    .unwrap_unchecked() = bgra.overlay_on(&background).to_slice()
            }
        });
    }

    pub(crate) fn update_data(&mut self, notification: Notification) {
        self.data = notification;
    }

    #[inline]
    pub(crate) fn write_to_file(&self, file: &mut File) {
        file.write_all(&self.framebuffer).unwrap();
    }
}

impl<'a> From<&'a BannerRect> for &'a Notification {
    fn from(value: &'a BannerRect) -> Self {
        &value.data
    }
}
