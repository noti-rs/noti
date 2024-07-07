use crate::data::image::ImageData;

use super::color::{Bgra, Rgba};

#[derive(Default)]
pub(crate) struct Image<'a> {
    image_data: Option<&'a ImageData>,
    svg_image: Option<ImageData>,
}

impl<'a> Image<'a> {
    pub(crate) fn add_svg(&mut self, image_path: Option<&'a str>, size: u32) {
        if self.image_data.is_some() || image_path.is_none() {
            return;
        }

        let tree = resvg::usvg::Tree::from_data(
            &std::fs::read(std::path::Path::new(image_path.unwrap())).unwrap(),
            &resvg::usvg::Options::default(),
        )
        .unwrap();

        let sx = size as f32 / tree.size().width();
        let sy = size as f32 / tree.size().height();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
        resvg::render(
            &tree,
            resvg::usvg::Transform::from_scale(sx, sy),
            &mut pixmap.as_mut(),
        );

        self.svg_image = Some(ImageData {
            data: pixmap
                .data()
                .chunks_exact(4)
                .map(|chunk| {
                    Rgba::from(TryInto::<&[u8; 4]>::try_into(chunk).unwrap())
                        .to_bgra()
                        .to_slice()
                })
                .flatten()
                .collect(),
            width: size as i32,
            height: size as i32,
            rowstride: size as i32 * 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
        });
    }

    pub(crate) fn draw<O: FnMut(usize, Bgra)>(
        &self,
        initial_pos: usize,
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

        let mut position = initial_pos;
        for y in 0..image_data.height as usize {
            for _x in 0..image_data.width as usize {
                callback(position, chunks.next().unwrap());
                position += 4;
            }
            position = stride * y;
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
