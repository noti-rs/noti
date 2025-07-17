use std::io::{Read, Seek, Write};

use image::codecs::png::PngEncoder;
use log::{debug, error, warn};
use pangocairo::cairo::ImageSurface;

use config::display::{ImageProperty, ResizingMethod};
use dbus::image::ImageData;
use shared::file_descriptor::FileDescriptor;

use crate::{
    drawer::{Drawer, MakeRounding},
    types::RectSize,
    PangoContext,
};

use super::{types::Offset, widget::Draw};

#[derive(Clone)]
pub enum Image {
    Exists {
        // INFO: the image storage always store image in png format
        file_descriptor: FileDescriptor,
        width: i32,
        height: i32,
        has_alpha: bool,
        rounding_radius: f64,
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

        let image_buffer = {
            let mut image_buffer = vec![];
            let mut file = image_data.image_file_descriptor.get_file();
            file.seek(std::io::SeekFrom::Start(0))
                .expect("The file must be able to seek");
            file.read_to_end(&mut image_buffer)
                .expect("The file must be able to read");
            image_buffer
        };

        let resized_image = if image_data.has_alpha {
            image::RgbaImage::from_vec(origin_width, origin_height, image_buffer)
                .map(image::DynamicImage::from)
        } else {
            image::RgbImage::from_vec(origin_width, origin_height, image_buffer)
                .map(image::DynamicImage::from)
        }
        .map(|image| {
            image::imageops::resize(
                &image,
                width as u32,
                height as u32,
                image_property.resizing_method.to_filter_type(),
            )
        });

        let Some(resized_image) = resized_image else {
            warn!("Image doesn't fits into its size");
            return Image::Unknown;
        };

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        resized_image
            .write_with_encoder(PngEncoder::new(&mut file))
            .unwrap();

        debug!("Image: Created from 'image_data'");

        Image::Exists {
            file_descriptor: file.into(),
            width,
            height,
            has_alpha: true,
            rounding_radius: image_property.rounding as f64,
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
                warn!("Cannot guess the format of image at {image_path:?}. Error: {err}. Maybe it's SVG, trying to parse.");
                return Self::from_svg(image_path, image_property, max_size);
            }
        };

        let image = match image::load_from_memory_with_format(&data, format) {
            Ok(image) => image,
            Err(err) => {
                error!("Cannot laod the image at {image_path:?}. Error: {err}");
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
        image::imageops::resize(
            &image,
            width as u32,
            height as u32,
            image_property.resizing_method.to_filter_type(),
        )
        .write_with_encoder(PngEncoder::new(&mut file))
        .unwrap();

        Image::Exists {
            file_descriptor: file.into(),
            width,
            height,
            has_alpha: true,
            rounding_radius: image_property.rounding as f64,
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
                match err {
                    resvg::usvg::Error::MalformedGZip => {
                        warn!("Malformed gzip format of SVG image in path: {image_path:?}")
                    }
                    resvg::usvg::Error::NotAnUtf8Str => warn!(
                        "The SVG image file contains non-UTF-8 string in path: {image_path:?}"
                    ),
                    _ => warn!("Something wrong with SVG image in path: {image_path:?}"),
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

        debug!("Image: Created image from svg by path {image_path:?}");

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        file.write_all(&pixmap.encode_png().unwrap())
            .expect("The temp file must be able to write");

        Image::Exists {
            file_descriptor: file.into(),
            width,
            height,
            has_alpha: true,
            rounding_radius: image_property.rounding as f64,
        }
    }

    fn print_readable_fs_error(error: std::io::Error, image_path: &std::path::Path) {
        match error.kind() {
            std::io::ErrorKind::NotFound => {
                warn!("Not found SVG image in path: {image_path:?}")
            }
            std::io::ErrorKind::PermissionDenied => {
                warn!("Permission to read SVG image in path is denied: {image_path:?}")
            }
            _ => warn!("Something wrong happened during reading SVG image in path: {image_path:?}"),
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
            Image::Exists { width, .. } => Some(*width as usize),
            Image::Unknown => None,
        }
    }

    pub fn height(&self) -> Option<usize> {
        match self {
            Image::Exists { height, .. } => Some(*height as usize),
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
    fn draw_with_offset(
        &self,
        offset: &Offset<usize>,
        _pango_context: &PangoContext,
        drawer: &mut Drawer,
    ) -> pangocairo::cairo::Result<()> {
        let Image::Exists {
            file_descriptor,
            width,
            height,
            has_alpha,
            rounding_radius,
            ..
        } = self
        else {
            return Ok(());
        };
        debug_assert!(has_alpha);

        let mut file = file_descriptor.get_file();
        file.seek(std::io::SeekFrom::Start(0))
            .expect("The temp file should be seekable");
        let source_surface = match ImageSurface::create_from_png(&mut *file) {
            Ok(source_surface) => source_surface,
            Err(err) => match err {
                cairo::IoError::Cairo(error) => Err(error)?,
                cairo::IoError::Io(error) => {
                    error!("Happened something wrong with IO opertaion during image rendering. Error: {error}");
                    return Ok(());
                }
            },
        };

        drawer.context.make_rounding(
            (*offset).into(),
            RectSize::new(*width as f64, *height as f64),
            *rounding_radius,
            *rounding_radius,
        );
        drawer
            .context
            .set_source_surface(source_surface, offset.x as f64, offset.y as f64)?;

        drawer.context.fill()?;
        Ok(())
    }
}

trait ToFilterType {
    fn to_filter_type(&self) -> image::imageops::FilterType;
}

impl ToFilterType for ResizingMethod {
    fn to_filter_type(&self) -> image::imageops::FilterType {
        match self {
            ResizingMethod::Nearest => image::imageops::FilterType::Nearest,
            ResizingMethod::Triangle => image::imageops::FilterType::Triangle,
            ResizingMethod::CatmullRom => image::imageops::FilterType::CatmullRom,
            ResizingMethod::Gaussian => image::imageops::FilterType::Gaussian,
            ResizingMethod::Lanczos3 => image::imageops::FilterType::Lanczos3,
        }
    }
}
