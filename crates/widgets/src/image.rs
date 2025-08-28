use std::io::{Read, Seek, Write};

use log::{debug, error, warn};

use config::display::{ImageProperty, MipmapMode, ResizingMethod};
use dbus::image::ImageData;
use shared::file_descriptor::FileDescriptor;

use crate::{drawer::Drawer, types::RectSize};

use super::{types::Offset, Draw};

#[derive(Clone)]
pub enum Image {
    Exists {
        // INFO: the image storage always store image in png format
        file_descriptor: FileDescriptor,
        origin_size: RectSize<i32>,
        resized_size: RectSize<i32>,
        rounding_radius: f32,
        filter_mode: skia_safe::FilterMode,
        mipmap_mode: skia_safe::MipmapMode,
    },
    Unknown,
}

impl Image {
    pub fn from_image_data(
        image_data: ImageData,
        image_property: &ImageProperty,
        max_size: &RectSize<usize>,
    ) -> Self {
        let origin_width = image_data.width as u32;
        let origin_height = image_data.height as u32;

        let Some((width, height)) = Self::try_fit_into_restricted_space(
            image_data.width,
            image_data.height,
            image_property,
            max_size,
        ) else {
            warn!("The margins for image is very large! The image will not rendered!");
            return Image::Unknown;
        };

        if image_data.has_alpha {
            return Image::Exists {
                file_descriptor: image_data.image_file_descriptor,
                origin_size: RectSize::new(image_data.width, image_data.height),
                resized_size: RectSize::new(width, height),
                rounding_radius: image_property.rounding as f32,
                filter_mode: image_property.resizing_method.to_skia_value(),
                mipmap_mode: image_property.mipmap_mode.to_skia_value(),
            };
        }

        let rgb_image = {
            let mut image_buffer = vec![];
            let mut file = image_data.image_file_descriptor.get_file();
            file.seek(std::io::SeekFrom::Start(0))
                .expect("The file must be able to seek");
            file.read_to_end(&mut image_buffer)
                .expect("The file must be able to read");
            image_buffer
        };

        let mut rgba_image = vec![255; origin_width as usize * origin_height as usize * 4];
        for (i, chunk) in rgb_image.chunks_exact(3).enumerate() {
            let offset = i * 4;
            rgba_image[offset] = chunk[0];
            rgba_image[offset + 1] = chunk[1];
            rgba_image[offset + 2] = chunk[2];
        }

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        if let Err(err) = file.write_all(&rgba_image) {
            Self::print_readable_fs_error(err, None);
            return Image::Unknown;
        }

        debug!("Image: Created from 'image_data'");

        Image::Exists {
            file_descriptor: file.into(),
            origin_size: RectSize::new(image_data.width, image_data.height),
            resized_size: RectSize::new(width, height),
            rounding_radius: image_property.rounding as f32,
            filter_mode: image_property.resizing_method.to_skia_value(),
            mipmap_mode: image_property.mipmap_mode.to_skia_value(),
        }
    }

    pub fn from_path(
        image_path: &std::path::Path,
        image_property: &ImageProperty,
        max_size: &RectSize<usize>,
    ) -> Image {
        let data = match std::fs::read(image_path) {
            Ok(data) => data,
            Err(err) => {
                Self::print_readable_fs_error(err, image_path);
                return Image::Unknown;
            }
        };

        let format = match image::guess_format(&data) {
            Ok(format) => format,
            Err(err) => {
                warn!(
                    "Cannot guess the format of image at {image_path}. \
                    Error: {err}. Maybe it's SVG, trying to parse.",
                    image_path = image_path.display()
                );
                return Self::from_svg(image_path, image_property, max_size);
            }
        };

        let image = match image::load_from_memory_with_format(&data, format) {
            Ok(image) => image,
            Err(err) => {
                error!(
                    "Cannot load the image at {image_path}. Error: {err}",
                    image_path = image_path.display()
                );
                return Image::Unknown;
            }
        };

        let Some((width, height)) = Self::try_fit_into_restricted_space(
            image.width() as i32,
            image.height() as i32,
            image_property,
            max_size,
        ) else {
            warn!("The margins for image is very large! The image will not rendered!");
            return Image::Unknown;
        };

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        if let Err(err) = file.write_all(&image.to_rgba8().into_vec()) {
            Self::print_readable_fs_error(err, image_path);
        }

        Image::Exists {
            file_descriptor: file.into(),
            origin_size: RectSize::new(image.width() as i32, image.height() as i32),
            resized_size: RectSize::new(width, height),
            rounding_radius: image_property.rounding as f32,
            filter_mode: image_property.resizing_method.to_skia_value(),
            mipmap_mode: image_property.mipmap_mode.to_skia_value(),
        }
    }

    pub fn from_svg(
        image_path: &std::path::Path,
        image_property: &ImageProperty,
        max_size: &RectSize<usize>,
    ) -> Self {
        if !image_path.is_file() {
            return Image::Unknown;
        }

        let data = match std::fs::read(image_path) {
            Ok(data) => data,
            Err(err) => {
                Self::print_readable_fs_error(err, image_path);
                return Image::Unknown;
            }
        };

        let tree = match resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()) {
            Ok(tree) => tree,
            Err(err) => {
                let image_path = image_path.display();
                match err {
                    resvg::usvg::Error::MalformedGZip => {
                        warn!("Malformed gzip format of SVG image in path: {image_path}")
                    }
                    resvg::usvg::Error::NotAnUtf8Str => {
                        warn!("The SVG image file contains non-UTF-8 string in path: {image_path}")
                    }
                    _ => warn!("Something wrong with SVG image in path: {image_path}"),
                }
                return Image::Unknown;
            }
        };

        let tree_size = tree.size();
        let Some((width, height)) = Self::try_fit_into_restricted_space(
            tree_size.width().round() as i32,
            tree_size.height().round() as i32,
            image_property,
            max_size,
        ) else {
            warn!("The margins for image is very large! The image will not rendered!");
            return Image::Unknown;
        };

        let scale = if width > height {
            width as f32 / tree_size.width()
        } else {
            height as f32 / tree_size.height()
        };

        let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(width as u32, height as u32) else {
            warn!("The SVG Image width or height is equal to zero!");
            return Image::Unknown;
        };

        resvg::render(
            &tree,
            resvg::usvg::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

        debug!(
            "Image: Created image from svg by path {image_path}",
            image_path = image_path.display()
        );

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        if let Err(err) = file.write_all(pixmap.data()) {
            Self::print_readable_fs_error(err, image_path);
            return Image::Unknown;
        }

        Image::Exists {
            file_descriptor: file.into(),
            origin_size: RectSize::new(width, height),
            resized_size: RectSize::new(width, height),
            rounding_radius: image_property.rounding as f32,
            filter_mode: image_property.resizing_method.to_skia_value(),
            mipmap_mode: image_property.mipmap_mode.to_skia_value(),
        }
    }

    fn print_readable_fs_error<'a, I>(error: std::io::Error, image_path: I)
    where
        I: Into<Option<&'a std::path::Path>>,
    {
        let image_path = image_path
            .into()
            .and_then(std::path::Path::to_str)
            .unwrap_or("Hidden path");

        match error.kind() {
            std::io::ErrorKind::NotFound => {
                warn!("Not found SVG image in path: {image_path}")
            }
            std::io::ErrorKind::PermissionDenied => {
                warn!("Permission to read SVG image in path is denied: {image_path}")
            }
            _ => warn!("Something wrong happened during reading SVG image in path: {image_path}"),
        }
    }

    pub fn or(self, other: Self) -> Self {
        if self.is_exists() {
            self
        } else {
            other
        }
    }

    pub fn or_else<F: FnOnce() -> Self>(self, other: F) -> Self {
        if self.is_exists() {
            self
        } else {
            other()
        }
    }

    pub fn is_exists(&self) -> bool {
        matches!(self, Image::Exists { .. })
    }

    pub fn width(&self) -> Option<usize> {
        match self {
            Image::Exists { resized_size, .. } => Some(resized_size.width as usize),
            Image::Unknown => None,
        }
    }

    pub fn height(&self) -> Option<usize> {
        match self {
            Image::Exists { resized_size, .. } => Some(resized_size.height as usize),
            Image::Unknown => None,
        }
    }

    fn try_fit_into_restricted_space(
        mut width: i32,
        mut height: i32,
        image_property: &ImageProperty,
        max_size: &RectSize<usize>,
    ) -> Option<(i32, i32)> {
        Self::limit_size(&mut width, &mut height, image_property.max_size);
        let (horizontal_spacing, vertical_spacing) = {
            let spacing = &image_property.margin;
            (spacing.horizontal() as usize, spacing.vertical() as usize)
        };

        if width as usize + horizontal_spacing > max_size.width {
            width -= horizontal_spacing as i32;
        }
        if height as usize + vertical_spacing > max_size.height {
            height -= vertical_spacing as i32;
        }

        if width <= 0 || height <= 0 {
            None
        } else {
            Some((width, height))
        }
    }

    fn limit_size(width: &mut i32, height: &mut i32, max_size: u16) {
        let swap = height > width;
        if swap {
            std::mem::swap(width, height);
        }

        if *width > max_size as i32 {
            let factor = max_size as f32 / *width as f32;
            *width = max_size as i32;
            *height = (factor * *height as f32).round() as i32;
        }

        if swap {
            std::mem::swap(width, height);
        }
    }
}

impl Draw for Image {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let Image::Exists {
            file_descriptor,
            origin_size,
            resized_size,
            rounding_radius,
            filter_mode,
            mipmap_mode,
        } = self
        else {
            return;
        };

        let mut file = file_descriptor.get_file();
        file.seek(std::io::SeekFrom::Start(0))
            .expect("The temp file should be seekable");

        let mut buffer = vec![];
        if let Err(err) = file.read_to_end(&mut buffer) {
            Self::print_readable_fs_error(err, None);
        }

        let data = skia_safe::Data::new_copy(&buffer);
        let image_info = skia_safe::ImageInfo::new(
            (origin_size.width, origin_size.height),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        let image =
            skia_safe::images::raster_from_data(&image_info, data, origin_size.width as usize * 4)
                .expect("Image must be valid");

        let src_rect =
            skia_safe::Rect::from_xywh(0., 0., image.width() as f32, image.height() as f32);

        let correct_offset: Offset<f32> = (*offset).into();
        let dst_rect = skia_safe::Rect::from_xywh(
            correct_offset.x,
            correct_offset.y,
            resized_size.width as f32,
            resized_size.height as f32,
        );

        let corner_radius = (std::cmp::min(resized_size.width, resized_size.height) as f32 / 2.0)
            .min(*rounding_radius);
        let rrect = skia_safe::RRect::new_rect_xy(dst_rect, corner_radius, corner_radius);
        let canvas = drawer.surface.canvas();

        canvas.save();
        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        let sampling = skia_safe::SamplingOptions::new(*filter_mode, *mipmap_mode);

        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);

        canvas.draw_image_rect_with_sampling_options(
            image,
            Some((&src_rect, skia_safe::canvas::SrcRectConstraint::Fast)),
            dst_rect,
            sampling,
            &paint,
        );
        canvas.restore();
    }
}

trait ToSkiaValue<T> {
    fn to_skia_value(&self) -> T;
}

impl ToSkiaValue<skia_safe::FilterMode> for ResizingMethod {
    fn to_skia_value(&self) -> skia_safe::FilterMode {
        match self {
            ResizingMethod::Nearest => skia_safe::FilterMode::Nearest,
            ResizingMethod::Linear => skia_safe::FilterMode::Linear,
        }
    }
}

impl ToSkiaValue<skia_safe::MipmapMode> for MipmapMode {
    fn to_skia_value(&self) -> skia_safe::MipmapMode {
        match self {
            MipmapMode::None => skia_safe::MipmapMode::None,
            MipmapMode::Nearest => skia_safe::MipmapMode::Nearest,
            MipmapMode::Linear => skia_safe::MipmapMode::Linear,
        }
    }
}
