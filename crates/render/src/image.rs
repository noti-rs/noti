use log::{debug, warn};
use owned_ttf_parser::{RasterGlyphImage, RasterImageFormat};

use config::display::{ImageProperty, ResizingMethod};
use dbus::image::ImageData;

use crate::{color::Color, drawer::Drawer, types::RectSize};

use super::{
    border::{Border, BorderBuilder},
    color::{Bgra, Rgba},
    types::Offset,
    widget::{Coverage, Draw, DrawColor},
};

#[derive(Clone)]
pub enum Image {
    Exists {
        data: ImageData,
        border: Option<Border>,
    },
    Unknown,
}

impl Image {
    pub fn from_image_data(
        image_data: ImageData,
        image_property: &ImageProperty,
        max_size: &RectSize,
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

        let resized_image = if image_data.has_alpha {
            image::RgbaImage::from_vec(origin_width, origin_height, image_data.data)
                .map(image::DynamicImage::from)
        } else {
            image::RgbImage::from_vec(origin_width, origin_height, image_data.data)
                .map(image::DynamicImage::from)
        }
        .map(|image| {
            image::imageops::resize(
                &image,
                width as u32,
                height as u32,
                image_property.resizing_method.to_filter_type(),
            )
            .to_vec()
        });

        let Some(resized_image) = resized_image else {
            warn!("Image doesn't fits into its size");
            return Image::Unknown;
        };

        debug!("Image: Created from 'image_data'");

        Image::Exists {
            data: ImageData {
                width,
                height,
                rowstride: width * 4,
                has_alpha: true,
                bits_per_sample: image_data.bits_per_sample,
                channels: 4,
                data: resized_image,
            },
            border: Some(Self::border_with_rounding(
                width,
                height,
                image_property.rounding,
            )),
        }
    }

    pub fn from_raster_glyph_image(
        RasterGlyphImage {
            width,
            height,
            format,
            data,
            ..
        }: RasterGlyphImage,
        size: u32,
    ) -> Option<Self> {
        let rgba_image = match format {
            RasterImageFormat::PNG => {
                image::load_from_memory_with_format(data, image::ImageFormat::Png)
                    .ok()?
                    .to_rgba8()
            }
            RasterImageFormat::BitmapMono
            | RasterImageFormat::BitmapMonoPacked
            | RasterImageFormat::BitmapGray2
            | RasterImageFormat::BitmapGray2Packed
            | RasterImageFormat::BitmapGray4
            | RasterImageFormat::BitmapGray4Packed
            | RasterImageFormat::BitmapGray8 => {
                image::load_from_memory_with_format(data, image::ImageFormat::Bmp)
                    .ok()?
                    .to_rgba8()
            }
            RasterImageFormat::BitmapPremulBgra32 => image::RgbaImage::from_vec(
                width as u32,
                height as u32,
                data.chunks_exact(4)
                    .flat_map(|chunk| {
                        Bgra::from(
                            TryInto::<&[u8; 4]>::try_into(chunk)
                                .expect("The image should have 4 channels"),
                        )
                        .into_rgba()
                        .into_slice()
                    })
                    .collect::<Vec<u8>>(),
            )?,
        };

        let (mut width, mut height) = (width as i32, height as i32);
        Self::limit_size(&mut width, &mut height, size as u16);

        let rgba_image = image::imageops::resize(
            &rgba_image,
            width as u32,
            width as u32,
            image::imageops::FilterType::Gaussian,
        );

        Some(Image::Exists {
            data: ImageData {
                width,
                height,
                rowstride: width * 4,
                has_alpha: true,
                bits_per_sample: 8,
                channels: 4,
                data: rgba_image.to_vec(),
            },
            border: None,
        })
    }

    pub fn from_svg(image_path: &str, image_property: &ImageProperty, max_size: &RectSize) -> Self {
        if image_path.is_empty() {
            return Image::Unknown;
        }

        let data = match std::fs::read(image_path) {
            Ok(data) => data,
            Err(err) => {
                match err.kind() {
                    std::io::ErrorKind::NotFound => {
                        warn!("Not found SVG image in path: {}", image_path)
                    }
                    std::io::ErrorKind::PermissionDenied => warn!(
                        "Permission to read SVG image in path is denied: {}",
                        image_path
                    ),
                    _ => warn!(
                        "Something wrong happened during reading SVG image in path: {}",
                        image_path
                    ),
                }

                return Image::Unknown;
            }
        };

        let tree = match resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()) {
            Ok(tree) => tree,
            Err(err) => {
                match err {
                    resvg::usvg::Error::MalformedGZip => {
                        warn!("Malformed gzip format of SVG image in path: {}", image_path)
                    }
                    resvg::usvg::Error::NotAnUtf8Str => warn!(
                        "The SVG image file contains non-UTF-8 string in path: {}",
                        image_path
                    ),
                    _ => warn!("Something wrong with SVG image in path: {}", image_path),
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

        debug!("Image: Created image from svg by path {image_path}");

        Image::Exists {
            data: ImageData {
                data: pixmap.data().to_vec(),
                width,
                height,
                rowstride: width * 4,
                has_alpha: true,
                bits_per_sample: 8,
                channels: 4,
            },
            border: Some(Self::border_with_rounding(
                width,
                height,
                image_property.rounding,
            )),
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
            Image::Exists { data, .. } => Some(data.width as usize),
            Image::Unknown => None,
        }
    }

    pub fn height(&self) -> Option<usize> {
        match self {
            Image::Exists { data, .. } => Some(data.height as usize),
            Image::Unknown => None,
        }
    }

    fn try_fit_into_restricted_space(
        mut width: i32,
        mut height: i32,
        image_property: &ImageProperty,
        max_size: &RectSize,
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

    fn border_with_rounding(width: i32, height: i32, rounding_radius: u16) -> Border {
        BorderBuilder::default()
            .color(Color::default())
            .size(0_usize)
            .radius(rounding_radius)
            .frame_width(width as usize)
            .frame_height(height as usize)
            .compile()
            .expect("Create Border for image rounding")
    }

    fn converter(has_alpha: bool) -> fn(&[u8]) -> Rgba {
        //SAFETY: it always safe way while the framebuffer have ARGB format and gives the correct
        //postiton.
        if has_alpha {
            |chunk: &[u8]| unsafe {
                Rgba::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap_unchecked())
            }
        } else {
            |chunk: &[u8]| unsafe {
                Rgba::from(TryInto::<&[u8; 3]>::try_into(chunk).unwrap_unchecked())
            }
        }
    }
}

impl Draw for Image {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        let Image::Exists { data, border } = self else {
            return;
        };

        let mut chunks = data
            .data
            .chunks_exact(data.channels as usize)
            .map(Self::converter(data.has_alpha));

        for y in 0..data.height as usize {
            for x in 0..data.width as usize {
                let border_coverage =
                    match border.as_ref().and_then(|border| border.get_color_at(x, y)) {
                        Some(DrawColor::Transparent(Coverage(factor))) => factor,
                        None => 1.0,
                        _ => unreachable!(),
                    };

                let color = unsafe { chunks.next().unwrap_unchecked() }.into_bgra();
                drawer.draw_color(
                    x + offset.x,
                    y + offset.y,
                    if border_coverage == 1.0 {
                        DrawColor::Overlay(color)
                    } else {
                        DrawColor::OverlayWithCoverage(color, Coverage(border_coverage))
                    },
                );
            }
        }
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
