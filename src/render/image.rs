use crate::data::image::ImageData;

use super::color::{Bgra, Rgba};

#[derive(Default)]
pub(crate) struct Image<'a> {
    image_data: Option<&'a ImageData>,
    svg_image: Option<ImageData>,
}

impl<'a> Image<'a> {
    pub(crate) fn or_svg(
        mut self,
        image_path: Option<&'a str>,
        min_size: u32,
        max_size: u32,
    ) -> Self {
        fn resize(min_size: f32, max_size: f32, actual_size: f32) -> (f32, f32) {
            if min_size > actual_size {
                (min_size, min_size / actual_size)
            } else if max_size < actual_size {
                (max_size, max_size / actual_size)
            } else {
                (actual_size, 1.0)
            }
        }

        if self.image_data.is_some() || image_path.is_none() {
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

        self.svg_image = Some(ImageData {
            data: pixmap
                .data()
                .chunks_exact(4)
                .flat_map(|chunk| {
                    Rgba::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap())
                        .to_bgra()
                        .to_slice()
                })
                .collect(),
            width: size,
            height: size,
            rowstride: size as i32 * 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
        });
        self
    }

    pub(crate) fn draw<O: FnMut(usize, Bgra)>(
        &self,
        x_offset: usize,
        y_offset: usize,
        stride: usize,
        mut callback: O,
    ) {
        let image_data = self.image_data.or(self.svg_image.as_ref());
        if image_data.is_none() {
            return;
        }

        let image_data = image_data.unwrap();
        let mut chunks = image_data
            .data
            .chunks_exact(image_data.channels as usize)
            .map(|chunk| {
                if image_data.has_alpha {
                    Bgra::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap())
                } else {
                    Bgra::from(TryInto::<&[u8; 3]>::try_into(chunk).unwrap())
                }
            });

        let mut position = stride * y_offset + x_offset * 4;
        for y in 0..image_data.height as usize {
            for _x in 0..image_data.width as usize {
                callback(position, chunks.next().unwrap());
                position += 4;
            }
            position = stride * (y + y_offset) + x_offset * 4;
        }
    }
}

impl<'a> From<Option<&'a ImageData>> for Image<'a> {
    fn from(image_data: Option<&'a ImageData>) -> Self {
        Self {
            image_data,
            ..Default::default()
        }
    }
}
