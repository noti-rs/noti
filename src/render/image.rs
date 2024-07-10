use owned_ttf_parser::{RasterGlyphImage, RasterImageFormat};

use crate::data::image::ImageData;

use super::color::{Bgra, Rgba};

pub(crate) enum Image {
    Exists(ImageData),
    Unknown,
}

impl Image {
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

    pub(crate) fn or_svg(self, image_path: Option<&str>, min_size: u32, max_size: u32) -> Self {
        fn resize(min_size: f32, max_size: f32, actual_size: f32) -> (f32, f32) {
            if min_size > actual_size {
                (min_size, min_size / actual_size)
            } else if max_size < actual_size {
                (max_size, max_size / actual_size)
            } else {
                (actual_size, 1.0)
            }
        }

        if self.exists() || image_path.is_none() {
            return self;
        }

        let tree = resvg::usvg::Tree::from_data(
            &std::fs::read(std::path::Path::new(image_path.unwrap())).unwrap(),
            &resvg::usvg::Options::default(),
        )
        .unwrap();

        let (min_size, max_size) = (min_size as f32, max_size as f32);

        let (size, scale) = resize(min_size, max_size, tree.size().width());
        let size = size.round() as i32;

        let mut pixmap = resvg::tiny_skia::Pixmap::new(size as u32, size as u32).unwrap();
        resvg::render(
            &tree,
            resvg::usvg::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

        Image::Exists(ImageData {
            data: pixmap.data().into_iter().map(|byte| *byte).collect(),
            width: size,
            height: size,
            rowstride: size as i32 * 4,
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
        let image_data = if let Image::Exists(image_data) = self  {
            image_data
        } else {
            return;
        };

        let convert = if image_data.has_alpha {
            |chunk: &[u8]| Rgba::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap())
        } else {
            |chunk: &[u8]| Rgba::from(TryInto::<&[u8; 3]>::try_into(chunk).unwrap())
        };

        let mut chunks = image_data
            .data
            .chunks_exact(image_data.channels as usize)
            .map(convert);

        let mut position = stride * y_offset + x_offset * 4;
        for _y in 0..image_data.height as usize {
            for _x in 0..image_data.width as usize {
                callback(position, chunks.next().unwrap().to_bgra());
                position += 4;
            }
            position += stride - image_data.rowstride as usize;
        }
    }

    pub(crate) fn draw_by_xy<O: FnMut(isize, isize, Bgra)>(
        &self,
        mut callback: O,
    ) {
        let image_data = if let Image::Exists(image_data) = self  {
            image_data
        } else {
            return;
        };

        let convert = if image_data.has_alpha {
            |chunk: &[u8]| Rgba::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap())
        } else {
            |chunk: &[u8]| Rgba::from(TryInto::<&[u8; 3]>::try_into(chunk).unwrap())
        };

        let mut chunks = image_data
            .data
            .chunks_exact(image_data.channels as usize)
            .map(convert);

        for y in 0..image_data.height as isize {
            for x in 0..image_data.width as isize {
                callback(x, y, chunks.next().unwrap().to_bgra());
            }
        }
    }
}

impl From<Option<&ImageData>> for Image {
    fn from(image_data: Option<&ImageData>) -> Self {
        image_data
            .map(|data| Image::Exists(data.clone()))
            .unwrap_or(Image::Unknown)
    }
}
