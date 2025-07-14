use std::{collections::HashMap, io::Write};

use shared::file_descriptor::FileDescriptor;
use zbus::zvariant::{Array, Structure, Value};

#[derive(Debug, Clone)]
pub struct ImageData {
    /// Width of image in pixels
    pub width: i32,

    /// Height of image in pixels
    pub height: i32,

    /// Distance in bytes between row starts
    pub rowstride: i32,

    /// Whether the image has an alpha channel
    pub has_alpha: bool,

    /// Must always be 8
    ///
    /// It's because of specification.
    pub bits_per_sample: i32,

    /// If has_alpha is **true**, must be 4, otherwise 3
    pub channels: i32,

    /// The image data, in RGB byte order
    ///
    /// To avoid the stroing data in RAM, the image stores in temporary file which will be
    /// destroyed if there is no handle to this file.
    pub image_file_descriptor: FileDescriptor,
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

        let width = i32::try_from(get_field(&mut fields, &0)).ok()?;
        let height = i32::try_from(get_field(&mut fields, &1)).ok()?;
        let rowstride = i32::try_from(get_field(&mut fields, &2)).ok()?;
        let has_alpha = bool::try_from(get_field(&mut fields, &3)).ok()?;
        let bits_per_sample = i32::try_from(get_field(&mut fields, &4)).ok()?;
        let channels = i32::try_from(get_field(&mut fields, &5)).ok()?;

        let file = match Array::try_from(get_field(&mut fields, &6)) {
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
