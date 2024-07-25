use owned_ttf_parser::{RasterGlyphImage, RasterImageFormat};

use crate::data::image::ImageData;

use super::color::{Bgra, Rgba};

#[derive(Clone)]
pub(crate) enum Image {
    Exists(ImageData),
    Unknown,
}

impl Image {
    pub(crate) fn from_image_data(image_data: Option<&ImageData>, size: u16) -> Self {
        image_data
            .map(|image_data| {
                let mut width = image_data.width;
                let mut height = image_data.height;
                let has_alpha = image_data.has_alpha;

                let image = if has_alpha {
                    image::DynamicImage::from(
                        image::RgbaImage::from_vec(
                            width as u32,
                            height as u32,
                            image_data.data.clone(),
                        )
                        .unwrap(),
                    )
                } else {
                    image::DynamicImage::from(
                        image::RgbImage::from_vec(
                            width as u32,
                            height as u32,
                            image_data.data.clone(),
                        )
                        .unwrap(),
                    )
                };

                Self::resize(&mut width, &mut height, size);
                let rowstride = width * 4;

                let image = image::imageops::resize(
                    &image,
                    width as u32,
                    height as u32,
                    image::imageops::FilterType::Gaussian,
                );

                Image::Exists(ImageData {
                    width,
                    height,
                    rowstride,
                    has_alpha: true,
                    bits_per_sample: image_data.bits_per_sample,
                    channels: 4,
                    data: image.to_vec(),
                })
            })
            .unwrap_or(Image::Unknown)
    }

    pub(crate) fn exists(&self) -> bool {
        if let Image::Exists(_) = self {
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
                        Bgra::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap())
                            .to_rgba()
                            .to_slice()
                    })
                    .collect::<Vec<u8>>(),
            )
            .unwrap(),
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

        Some(Image::Exists(ImageData {
            width: new_width as i32,
            height: new_height as i32,
            rowstride: new_width as i32 * 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: rgba_image.to_vec(),
        }))
    }

    pub(crate) fn or_svg(self, image_path: Option<&str>, size: u16) -> Self {
        if self.exists() || image_path.is_none() {
            return self;
        }

        let tree = resvg::usvg::Tree::from_data(
            &std::fs::read(std::path::Path::new(image_path.unwrap())).unwrap(),
            &resvg::usvg::Options::default(),
        )
        .unwrap();

        let tree_size = tree.size();
        let (mut width, mut height) = (
            tree_size.width().round() as i32,
            tree_size.height().round() as i32,
        );

        Self::resize(&mut width, &mut height, size);

        let scale = if width > height {
            width as f32 / tree_size.width()
        } else {
            height as f32 / tree_size.height()
        };

        let mut pixmap = resvg::tiny_skia::Pixmap::new(width as u32, height as u32).unwrap();
        resvg::render(
            &tree,
            resvg::usvg::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

        Image::Exists(ImageData {
            data: pixmap.data().to_vec(),
            width,
            height,
            rowstride: width * 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
        })
    }

    pub(crate) fn width(&self) -> Option<usize> {
        match self {
            Image::Exists(img) => Some(img.width as usize),
            Image::Unknown => None,
        }
    }

    pub(crate) fn height(&self) -> Option<usize> {
        match self {
            Image::Exists(img) => Some(img.height as usize),
            Image::Unknown => None,
        }
    }

    pub(crate) fn draw<O: FnMut(usize, Bgra)>(
        &self,
        x_offset: usize,
        y_offset: usize,
        stride: usize,
        mut callback: O,
    ) {
        let image_data = if let Image::Exists(image_data) = self {
            image_data
        } else {
            return;
        };

        let mut chunks = image_data
            .data
            .chunks_exact(image_data.channels as usize)
            .map(Self::converter(image_data.has_alpha));

        let mut position = stride * y_offset + x_offset * 4;
        for _y in 0..image_data.height as usize {
            for _x in 0..image_data.width as usize {
                callback(position, chunks.next().unwrap().to_bgra());
                position += 4;
            }
            position += stride - image_data.rowstride as usize;
        }
    }

    pub(crate) fn draw_by_xy<O: FnMut(isize, isize, Bgra)>(&self, mut callback: O) {
        let image_data = if let Image::Exists(image_data) = self {
            image_data
        } else {
            return;
        };

        let mut chunks = image_data
            .data
            .chunks_exact(image_data.channels as usize)
            .map(Self::converter(image_data.has_alpha));

        for y in 0..image_data.height as isize {
            for x in 0..image_data.width as isize {
                callback(x, y, chunks.next().unwrap().to_bgra());
            }
        }
    }

    fn resize(width: &mut i32, height: &mut i32, new_size: u16) {
        if width > height {
            let factor = new_size as f32 / *width as f32;
            *width = new_size as i32;
            *height = (factor * *height as f32).round() as i32;
        } else {
            let factor = new_size as f32 / *height as f32;
            *height = new_size as i32;
            *width = (factor * *width as f32).round() as i32;
        }
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
