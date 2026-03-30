use std::io::{Read, Seek, Write};

use log::{debug, error, warn};

use shared::{error::ConversionError, file_descriptor::FileDescriptor, value::TryFromValue};

use crate::{
    drawer::Drawer,
    types::{extent::Extent2D, offset::Offset},
    widget::image::ImageConfiguration,
    Draw,
};

/// Represents a GPU-ready image used by `WImage` during rendering.
///
/// The `Image` type acts as a container for image data and its rendering
/// configuration. Rather than immediately rasterizing or resizing images,
/// it stores their file descriptor and size information, leaving the
/// final drawing and scaling work to the GPU.
///
/// This allows efficient rendering and flexible resizing while avoiding
/// unnecessary CPU work.  
///
/// The [`Image::Unknown`] variant indicates that the image could
/// not be loaded or was invalid (e.g. corrupt raw data or unsupported format),
/// allowing the caller to gracefully skip rendering instead of crashing.
#[derive(Clone, Default)]
pub enum Image {
    Exists {
        file_descriptor: FileDescriptor,
        origin_size: Extent2D<i32>,
        resized_size: Extent2D<i32>,
        rounding_radius: f32,
        filter_mode: skia_safe::FilterMode,
        mipmap_mode: skia_safe::MipmapMode,
    },
    #[default]
    Unknown,
}

impl Image {
    /// Creates an [`Image`] from raw image data received over D-Bus or another source.
    ///
    /// This method decodes the provided [`ImageData`] into a valid RGBA buffer,
    /// stores it in a temporary file (via [`FileDescriptor`]), and prepares its
    /// rendering metadata (sizes, filter mode, mipmap mode).
    ///
    /// Before creating the final image, it verifies that it can fit within
    /// the provided [`Extent2D`] constraints, optionally scaling it down
    /// according to [`ImageConfiguration`] settings.  
    ///
    /// Returns [`Image::Unknown`] if the data cannot be decoded or does not fit.
    pub fn from_image_data(
        image_data: &ImageData,
        image_configuration: &ImageConfiguration,
        max_size: &Extent2D<usize>,
    ) -> Self {
        let origin_width = image_data.width as u32;
        let origin_height = image_data.height as u32;

        let Some((width, height)) = Self::try_fit_into_restricted_space(
            image_data.width,
            image_data.height,
            image_configuration,
            max_size,
        ) else {
            warn!("The margins for image is very large! The image will not rendered!");
            return Image::Unknown;
        };

        if image_data.has_alpha {
            return Image::Exists {
                file_descriptor: image_data.image_file_descriptor.clone(),
                origin_size: Extent2D::new(image_data.width, image_data.height),
                resized_size: Extent2D::new(width, height),
                rounding_radius: image_configuration.rounding as f32,
                filter_mode: image_configuration.resizing_method.to_skia_value(),
                mipmap_mode: image_configuration.mipmap_mode.to_skia_value(),
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
            origin_size: Extent2D::new(image_data.width, image_data.height),
            resized_size: Extent2D::new(width, height),
            rounding_radius: image_configuration.rounding as f32,
            filter_mode: image_configuration.resizing_method.to_skia_value(),
            mipmap_mode: image_configuration.mipmap_mode.to_skia_value(),
        }
    }

    /// Creates an [`Image`] from an image file at the given path.
    ///
    /// This method supports any file format handled by `image-rs`
    /// (PNG, JPEG, WEBP, etc.) and will load, decode, and store the
    /// result as a temporary file descriptor for GPU-backed rendering.
    ///
    /// Like [`Self::from_image_data`], it respects the
    /// size restrictions and scaling rules provided in [`ImageConfiguration`].
    ///
    /// Returns [`Image::Unknown`] if the file is missing, unreadable,
    /// or cannot be decoded.
    pub fn from_path(
        image_path: &std::path::Path,
        image_configuration: &ImageConfiguration,
        max_size: &Extent2D<usize>,
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
                return Self::from_svg(image_path, image_configuration, max_size);
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
            image_configuration,
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
            origin_size: Extent2D::new(image.width() as i32, image.height() as i32),
            resized_size: Extent2D::new(width, height),
            rounding_radius: image_configuration.rounding as f32,
            filter_mode: image_configuration.resizing_method.to_skia_value(),
            mipmap_mode: image_configuration.mipmap_mode.to_skia_value(),
        }
    }

    /// Creates an [`Image`] from an SVG file.
    ///
    /// Unlike [`Self::from_path`], this method rasterizes
    /// the SVG immediately into a bitmap image before storing it
    /// as a file descriptor.  
    ///
    /// This ensures the resulting image is GPU-friendly while preserving
    /// correct vector scaling and aspect ratio within the given
    /// [`Extent2D`] constraints.
    pub fn from_svg(
        image_path: &std::path::Path,
        image_configuration: &ImageConfiguration,
        max_size: &Extent2D<usize>,
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
            image_configuration,
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
            origin_size: Extent2D::new(width, height),
            resized_size: Extent2D::new(width, height),
            rounding_radius: image_configuration.rounding as f32,
            filter_mode: image_configuration.resizing_method.to_skia_value(),
            mipmap_mode: image_configuration.mipmap_mode.to_skia_value(),
        }
    }

    /// Prints a human-readable description of a file-system error
    /// that occurred while working with the image.
    ///
    /// This is used when reading, writing, or opening the temporary
    /// file descriptor fails, making it easier to debug I/O issues.
    ///
    /// It is intended for logging and diagnostics, not for user-facing
    /// error messages.
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

    /// Checks whether the image is exists or not.
    pub fn is_exists(&self) -> bool {
        matches!(self, Image::Exists { .. })
    }

    /// Returns the current width of the image in pixels.
    ///
    /// This reflects the `resized_size.width` if the image was scaled
    /// during creation.  
    ///  
    /// For [`Image::Unknown`], this will typically return [None].
    pub fn width(&self) -> Option<usize> {
        match self {
            Image::Exists { resized_size, .. } => Some(resized_size.width as usize),
            Image::Unknown => None,
        }
    }

    /// Returns the current height of the image in pixels.
    ///
    /// For [`Image::Unknown`], this will typically return [`None`].
    pub fn height(&self) -> Option<usize> {
        match self {
            Image::Exists { resized_size, .. } => Some(resized_size.height as usize),
            Image::Unknown => None,
        }
    }

    /// Attempts to fit the image into the provided rectangular space.
    ///
    /// This method calculates a scaled width and height that preserve
    /// the image's aspect ratio while respecting margins and constraints
    /// from [`ImageConfiguration`].
    ///
    /// Returns `Some((width, height))` if the image can fit into the
    /// specified space, or `None` if it cannot be displayed at all.
    fn try_fit_into_restricted_space(
        mut width: i32,
        mut height: i32,
        image_configuration: &ImageConfiguration,
        max_size: &Extent2D<usize>,
    ) -> Option<(i32, i32)> {
        Self::limit_size(&mut width, &mut height, image_configuration.max_size);
        let (horizontal_spacing, vertical_spacing) = {
            let spacing = &image_configuration.margin;
            (spacing.horizontal(), spacing.vertical())
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

    /// Scales the image dimensions down to not exceed `max_size`,
    /// preserving its aspect ratio.
    ///
    /// This is typically used to cap very large images to a reasonable
    /// size before rendering.
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

        let correct_offset: Offset<f32> = offset.into();
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

#[derive(Debug, Clone)]
pub struct ImageData {
    pub width: i32,
    pub height: i32,
    pub has_alpha: bool,

    /// File descriptor containing the image data in RGB byte order.
    ///
    /// To avoid storing large data in RAM, the image is stored in a temporary file.
    /// The file is automatically removed when no handles reference it.
    pub image_file_descriptor: FileDescriptor,
}

/// The algorithm used to calculate new pixels when an image is scaled.
///
/// This determines the visual quality of the image when it is made
/// larger or smaller than its original size.
#[derive(Debug, Default, Clone)]
pub enum ResizingMethod {
    /// Faster but "blocky" scaling. Picks the closest pixel color
    /// without blending. Best for pixel art.
    Nearest,
    /// Smoother scaling. Blends neighboring pixels together to
    /// prevent jagged edges. Standard for photos and icons.
    #[default]
    Linear,
}

impl TryFromValue for ResizingMethod {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "nearest" => ResizingMethod::Nearest,
            "linear" => ResizingMethod::Linear,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "nearest or linear",
                actual: value,
            })?,
        })
    }
}

/// The technique used for choosing between different pre-scaled
/// versions of an image.
///
/// Mipmapping helps prevent "shimmering" or visual noise when an
/// image is shrunk significantly. It tells Skia how to sample
/// from pre-calculated lower-resolution versions of the image.
#[derive(Debug, Default, Clone)]
pub enum MipmapMode {
    /// No pre-scaled versions are used; the image is scaled directly
    /// from the original.
    None,
    /// Picks the single best-fitting pre-scaled version.
    Nearest,
    /// Blends between the two best-fitting pre-scaled versions for
    /// the smoothest possible transition.
    #[default]
    Linear,
}

impl TryFromValue for MipmapMode {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "none" => MipmapMode::None,
            "nearest" => MipmapMode::Nearest,
            "linear" => MipmapMode::Linear,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "none, nearest or linear",
                actual: value,
            })?,
        })
    }
}

/// A helper trait to convert application-specific configuration
/// values into their Skia equivalents.
///
/// This trait is implemented for types like filter mode or mipmap
/// mode that exist in user configuration but must be translated
/// into `skia_safe` enums before rendering.
///
/// By using this trait, `Image` and other drawing code remain
/// decoupled from configuration details, making the conversion
/// process uniform and easy to extend.
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
