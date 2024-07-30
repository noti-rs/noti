use std::{fs::File, io::Write, time};

use crate::{
    config::{spacing::Spacing, Colors, DisplayConfig, CONFIG},
    data::notification::Notification,
};

use super::{
    border::BorderBuilder, color::Bgra, font::FontCollection, image::Image, text::TextRect,
    types::Offset,
};

#[derive(Clone)]
pub(super) struct Coverage(pub(super) f32);

#[derive(Clone)]
pub(super) enum DrawColor {
    Replace(Bgra),
    Overlay(Bgra),
    OverlayWithCoverage(Bgra, Coverage),
}

pub(super) trait Draw {
    fn draw<Output: FnMut(usize, usize, DrawColor)>(&self, output: Output);
}

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
    }

    #[inline]
    pub(crate) fn write_to_file(&self, file: &mut File) {
        file.write_all(&self.framebuffer).unwrap();
    }

    pub(crate) fn draw(&mut self, font_collection: &FontCollection) {
        let (mut width, mut height) = (
            CONFIG.general().width() as usize,
            CONFIG.general().height() as usize,
        );

        let display = CONFIG.display_by_app(&self.data.app_name);
        let colors = display.colors().by_urgency(&self.data.hints.urgency);

        let background: Bgra = colors.background().into();

        self.init_framebuffer(width, height, &background);
        self.stride = width as usize * 4;

        let border_spacing = Spacing::all_directional(display.border().size());
        let padding = display.padding() + border_spacing;
        padding.shrink(&mut width, &mut height);

        let mut offset: Offset = padding.into();

        let image = self.draw_image(offset.clone(), width, height, &background, display);
        let img_width = image
            .map(|img| img.width().unwrap_or_default())
            .unwrap_or_default();
        width -= img_width;

        offset.x += img_width;

        let summary = self.draw_summary(
            offset.clone(),
            width,
            height,
            colors,
            font_collection,
            display,
        );

        height -= summary.height();
        offset.y += summary.height();

        let _ = self.draw_text(offset, width, height, colors, font_collection, display);

        self.draw_border(
            CONFIG.general().width().into(),
            CONFIG.general().height().into(),
            &background,
            display,
        );
    }

    fn init_framebuffer(&mut self, width: usize, height: usize, background: &Bgra) {
        self.framebuffer = vec![background.clone(); width as usize * height as usize]
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

        border.draw(|x, y, color| self.put_color_at(x, y, Self::convert_color(color, background)));
    }

    fn draw_image(
        &mut self,
        mut offset: Offset,
        _width: usize,
        height: usize,
        background: &Bgra,
        display: &DisplayConfig,
    ) -> Option<Image> {
        let image =
            Image::from_image_data(self.data.hints.image_data.as_ref(), display.image_size())
                .or_svg(
                    self.data
                        .hints
                        .image_path
                        .as_deref()
                        .or(Some(self.data.app_icon.as_str())),
                    display.image_size(),
                );

        if !image.exists() {
            return None;
        }

        let img_height = image.height();

        if img_height.is_some_and(|img_height| {
            img_height <= height as usize - display.border().size() as usize * 2
        }) {
            offset.y += img_height
                .map(|img_height| height as usize / 2 - img_height / 2)
                .unwrap_or_default();

            image.draw(|x, y, color| {
                self.put_color_at(
                    x + offset.x,
                    y + offset.y,
                    Self::convert_color(color, &background),
                );
            });
            Some(image)
        } else {
            eprintln!(
                "Image height exceeds the possible height!\n\
                Please set a higher value of height or decrease the value of image_size in config.toml."
            );
            None
        }
    }

    fn draw_summary(
        &mut self,
        offset: Offset,
        width: usize,
        height: usize,
        colors: &Colors,
        font_collection: &FontCollection,
        display: &DisplayConfig,
    ) -> TextRect {
        let title_cfg = display.title();

        let foreground: Bgra = colors.foreground().into();
        let background: Bgra = colors.background().into();

        let mut summary = TextRect::from_str(
            &self.data.summary,
            CONFIG.general().font().size() as f32,
            font_collection,
        );

        summary.set_margin(title_cfg.margin());
        summary.set_line_spacing(title_cfg.line_spacing() as usize);
        summary.set_foreground(foreground);
        summary.set_ellipsize_at(display.ellipsize_at());
        summary.set_alignment(title_cfg.alignment());

        summary.compile(width, height);

        summary.draw(|x, y, color| {
            self.put_color_at(
                x as usize + offset.x,
                y as usize + offset.y,
                Self::convert_color(color, &background),
            );
        });

        summary
    }

    fn draw_text(
        &mut self,
        offset: Offset,
        width: usize,
        height: usize,
        colors: &Colors,
        font_collection: &FontCollection,
        display: &DisplayConfig,
    ) -> TextRect {
        let body_cfg = display.body();
        let font_size = CONFIG.general().font().size() as f32;
        let foreground: Bgra = colors.foreground().into();
        let background: Bgra = colors.background().into();

        let mut text = if display.markup() {
            TextRect::from_text(&self.data.body, font_size, font_collection)
        } else {
            TextRect::from_str(&self.data.body.body, font_size, font_collection)
        };

        text.set_margin(body_cfg.margin());
        text.set_line_spacing(body_cfg.line_spacing() as usize);
        text.set_foreground(foreground);
        text.set_ellipsize_at(display.ellipsize_at());
        text.set_alignment(body_cfg.alignment());
        text.compile(width, height);

        text.draw(|x, y, color| {
            self.put_color_at(
                x as usize + offset.x,
                y as usize + offset.y,
                Self::convert_color(color, &background),
            );
        });

        text
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
