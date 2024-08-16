use owned_ttf_parser::{RasterGlyphImage, RasterImageFormat};

use config::{ImageProperty, ResizingMethod};
use dbus::image::ImageData;

use super::{
    border::{Border, BorderBuilder},
    color::{Bgra, Rgba},
    types::Offset,
    widget::{Coverage, Draw, DrawColor},
};

#[derive(Clone)]
pub(crate) enum Image {
    Exists {
        data: ImageData,
        border: Option<Border>,
    },
    Unknown,
}

impl Image {
    pub(crate) fn from_image_data(
        image_data: Option<&ImageData>,
        image_property: &ImageProperty,
    ) -> Self {
        image_data
            .map(|image_data| {
                let mut width = image_data.width;
                let mut height = image_data.height;
                let has_alpha = image_data.has_alpha;

                let image = if has_alpha {
                    let Some(rgba_image) = image::RgbaImage::from_vec(
                        width as u32,
                        height as u32,
                        image_data.data.clone(),
                    ) else {
                        return Image::Unknown;
                    };

                    image::DynamicImage::from(rgba_image)
                } else {
                    let Some(rgb_image) = image::RgbImage::from_vec(
                        width as u32,
                        height as u32,
                        image_data.data.clone(),
                    ) else {
                        return Image::Unknown;
                    };

                    image::DynamicImage::from(rgb_image)
                };

                Self::resize(&mut width, &mut height, image_property.max_size);
                let rowstride = width * 4;

                let image = image::imageops::resize(
                    &image,
                    width as u32,
                    height as u32,
                    image_property.resizing_method.to_filter_type(),
                );

                Image::Exists {
                    data: ImageData {
                        width,
                        height,
                        rowstride,
                        has_alpha: true,
                        bits_per_sample: image_data.bits_per_sample,
                        channels: 4,
                        data: image.to_vec(),
                    },
                    border: Some(Self::border_with_rounding(
                        width,
                        height,
                        image_property.rounding,
                    )),
                }
            })
            .unwrap_or(Image::Unknown)
    }

    pub(crate) fn exists(&self) -> bool {
        if let Image::Exists { .. } = self {
            true
        } else {
            false
        }
    }

    pub(crate) fn from_raster_glyph_image(from: RasterGlyphImage, size: u32) -> Option<Self> {
        let RasterGlyphImage {
            width,
            height,
            format,
            data,
            ..
        } = from;

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
                        Bgra::from(TryInto::<&[u8; 4]>::try_into(chunk).expect("Current chunk is not correct. Please contact to developer with this information."))
                            .to_rgba()
                            .to_slice()
                    })
                    .collect::<Vec<u8>>(),
            )
            .expect("Can't parse image data of emoji. Please contact to developer with this information."),
        };

        let factor = size as f32 / width as f32;
        let new_width = size;
        let new_height = (factor * height as f32).round() as u32;

        let rgba_image = image::imageops::resize(
            &rgba_image,
            new_width,
            new_width,
            image::imageops::FilterType::Gaussian,
        );

        Some(Image::Exists {
            data: ImageData {
                width: new_width as i32,
                height: new_height as i32,
                rowstride: new_width as i32 * 4,
                has_alpha: true,
                bits_per_sample: 8,
                channels: 4,
                data: rgba_image.to_vec(),
            },
            border: None,
        })
    }

    pub(crate) fn or_svg(self, image_path: Option<&str>, image_property: &ImageProperty) -> Self {
        if self.exists() || image_path.is_none() {
            return self;
        }

        let image_path = unsafe { image_path.unwrap_unchecked() };

        let tree = if let Ok(data) = std::fs::read(image_path) {
            resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())
        } else {
            return self;
        };

        let Ok(tree) = tree else {
            return self;
        };

        let tree_size = tree.size();
        let (mut width, mut height) = (
            tree_size.width().round() as i32,
            tree_size.height().round() as i32,
        );

        Self::resize(&mut width, &mut height, image_property.max_size);

        let scale = if width > height {
            width as f32 / tree_size.width()
        } else {
            height as f32 / tree_size.height()
        };

        let mut pixmap = resvg::tiny_skia::Pixmap::new(width as u32, height as u32)
            .expect("The Pixmap must be created. Something happened wrong. Please contact to developer with this information.");
        resvg::render(
            &tree,
            resvg::usvg::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

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

    pub(crate) fn width(&self) -> Option<usize> {
        match self {
            Image::Exists { data, .. } => Some(data.width as usize),
            Image::Unknown => None,
        }
    }

    pub(crate) fn height(&self) -> Option<usize> {
        match self {
            Image::Exists { data, .. } => Some(data.height as usize),
            Image::Unknown => None,
        }
    }

    fn resize(width: &mut i32, height: &mut i32, max_size: u16) {
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
            .color(Bgra::new())
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
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        let Image::Exists { data, border } = self else {
            return;
        };

        let mut chunks = data
            .data
            .chunks_exact(data.channels as usize)
            .map(Self::converter(data.has_alpha));

        for y in 0..data.height as usize {
            for x in 0..data.width as usize {
                let border_coverage = match border
                    .as_ref()
                    .map(|border| border.get_color_at(x, y))
                    .flatten()
                {
                    Some(DrawColor::Transparent(Coverage(factor))) => factor,
                    None => 1.0,
                    _ => unreachable!(),
                };

                let color = unsafe { chunks.next().unwrap_unchecked() }.to_bgra();
                output(
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
