use std::{collections::HashMap, io::Write};

use shared::file_descriptor::FileDescriptor;
use zbus::zvariant::{Array, Structure, Value};

/// Represents information about an image sent through D-Bus.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Width of image in pixels.
    pub width: i32,

    /// Height of image in pixels.
    pub height: i32,

    /// Number of bytes between the start of one row and the next.
    pub rowstride: i32,

    /// Indicates whether the image has an alpha channel.
    pub has_alpha: bool,

    /// Number of bits per sample. Must always be 8 according to the specification.
    pub bits_per_sample: i32,

    /// Number of channels. Must be 4 if `has_alpha` is true, otherwise 3.
    pub channels: i32,

    /// File descriptor containing the image data in RGB byte order.
    ///
    /// To avoid storing large data in RAM, the image is stored in a temporary file.
    /// The file is automatically removed when no handles reference it.
    pub image_file_descriptor: FileDescriptor,
}

impl ImageData {
    const WIDTH_INDEX: usize = 0;
    const HEIGHT_INDEX: usize = 1;
    const ROWSTRIDE_INDEX: usize = 2;
    const HAS_ALPHA_INDEX: usize = 3;
    const BITS_PER_SAMPLE_INDEX: usize = 4;
    const CHANNELS_INDEX: usize = 5;
    const FILE_INDEX: usize = 6;

    /// Attempts to parse image data from a hint.
    pub fn from_hint(hint: Value<'_>) -> Option<Self> {
        Structure::try_from(hint)
            .ok()
            .and_then(Self::from_structure)
    }

    /// Attempts to parse image data from a structure.
    ///
    /// If the data is corrupted, an [ImageData] instance will not be created.
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

        let width = i32::try_from(get_field(&mut fields, &Self::WIDTH_INDEX)).ok()?;
        let height = i32::try_from(get_field(&mut fields, &Self::HEIGHT_INDEX)).ok()?;
        let rowstride = i32::try_from(get_field(&mut fields, &Self::ROWSTRIDE_INDEX)).ok()?;
        let has_alpha = bool::try_from(get_field(&mut fields, &Self::HAS_ALPHA_INDEX)).ok()?;
        let bits_per_sample =
            i32::try_from(get_field(&mut fields, &Self::BITS_PER_SAMPLE_INDEX)).ok()?;
        let channels = i32::try_from(get_field(&mut fields, &Self::CHANNELS_INDEX)).ok()?;

        let file = match Array::try_from(get_field(&mut fields, &Self::FILE_INDEX)) {
            Ok(array) => {
                const BUF_SIZE_4KB: usize = 4096;
                let mut file = tempfile::tempfile().expect("The temp file must be created!");
                array
                    .chunks(BUF_SIZE_4KB)
                    .map(|bytes| {
                        bytes
                            .iter()
                            .map(|byte| u8::try_from(byte).expect("Expected u8 byte of image data"))
                            .collect::<Vec<u8>>()
                    })
                    .for_each(|buffer| {
                        file.write_all(&buffer)
                            .expect("The temp file must be able to write image.")
                    });

                file
            }
            Err(_) => return None,
        };

        Some(ImageData {
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            image_file_descriptor: file.into(),
        })
    }
}
