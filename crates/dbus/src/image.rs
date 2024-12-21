use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zbus::zvariant::{Array, Structure, Value};

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageData {
    // Width of image in pixels
    pub width: i32,

    // Height of image in pixels
    pub height: i32,

    // Distance in bytes between row starts
    pub rowstride: i32,

    // Whether the image has an alpha channel
    pub has_alpha: bool,

    // Must always be 8
    pub bits_per_sample: i32,

    // If has_alpha is TRUE, must be 4, otherwise 3
    pub channels: i32,

    // The image data, in RGB byte order
    pub data: Vec<u8>,
}

impl ImageData {
    pub fn from_hint(hint: Value<'_>) -> Option<Self> {
        Structure::try_from(hint)
            .ok()
            .and_then(Self::from_structure)
    }

    fn from_structure(image_structure: Structure) -> Option<Self> {
        fn get_field<'a, 'b>(
            fields: &'a mut HashMap<usize, Value<'b>>,
            index: &'a usize,
        ) -> Value<'b> {
            unsafe { fields.remove(index).unwrap_unchecked() }
        }

        let mut fields = image_structure.into_fields().into_iter().enumerate().fold(
            HashMap::new(),
            |mut acc, (index, value)| {
                acc.insert(index, value);
                acc
            },
        );

        if fields.len() < 7 {
            return None;
        }

        let image_raw = match Array::try_from(get_field(&mut fields, &6)) {
            Ok(array) => array,
            Err(_) => return None,
        };

        let width = i32::try_from(get_field(&mut fields, &0)).ok()?;
        let height = i32::try_from(get_field(&mut fields, &1)).ok()?;
        let rowstride = i32::try_from(get_field(&mut fields, &2)).ok()?;
        let has_alpha = bool::try_from(get_field(&mut fields, &3)).ok()?;
        let bits_per_sample = i32::try_from(get_field(&mut fields, &4)).ok()?;
        let channels = i32::try_from(get_field(&mut fields, &5)).ok()?;

        let data = image_raw
            .iter()
            .map(|value| u8::try_from(value).expect("expected u8"))
            .collect::<Vec<_>>();

        Some(ImageData {
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        })
    }
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rowstride", &self.rowstride)
            .field("has_alpha", &self.has_alpha)
            .field("bits_per_sample", &self.bits_per_sample)
            .field("channels", &self.channels)
            .field("data", &"Vec<u8> [...]")
            .finish()
    }
}
