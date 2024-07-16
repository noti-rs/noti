use std::{collections::HashMap, marker::PhantomData};

use serde::{de::Visitor, Deserialize};


#[derive(Debug, Default, Clone)]
pub struct Offset {
    top: u8,
    right: u8,
    bottom: u8,
    left: u8,
}

impl Offset {
    fn all_directional(val: u8) -> Self {
        Self {
            top: val,
            bottom: val,
            right: val,
            left: val,
        }
    }

    fn cross(vertical: u8, horizontal: u8) -> Self {
        Self {
            top: vertical,
            bottom: vertical,
            right: horizontal,
            left: horizontal,
        }
    }

    pub fn top(&self) -> u8 {
        self.top
    }

    pub fn right(&self) -> u8 {
        self.right
    }

    pub fn bottom(&self) -> u8 {
        self.bottom
    }

    pub fn left(&self) -> u8 {
        self.left
    }
}

impl From<i64> for Offset {
    fn from(value: i64) -> Self {
        Offset::all_directional(value.clamp(0, u8::MAX as i64) as u8)
    }
}

impl From<Vec<u8>> for Offset {
    fn from(value: Vec<u8>) -> Self {
        match value.len() {
            1 => Offset::all_directional(value[0]),
            2 => Offset::cross(value[0], value[1]),
            3 => Offset {
                top: value[0],
                right: value[1],
                left: value[1],
                bottom: value[2],
            },
            4 => Offset {
                top: value[0],
                right: value[1],
                bottom: value[2],
                left: value[3],
            },
            _ => unreachable!(),
        }
    }
}

impl From<HashMap<String, u8>> for Offset {
    fn from(map: HashMap<String, u8>) -> Self {
        let vertical = map.get("vertical");
        let horizontal = map.get("horizontal");
        let top = map.get("top");
        let bottom = map.get("bottom");
        let right = map.get("right");
        let left = map.get("left");

        Self {
            top: *top.or(vertical).unwrap_or(&0),
            bottom: *bottom.or(vertical).unwrap_or(&0),
            right: *right.or(horizontal).unwrap_or(&0),
            left: *left.or(horizontal).unwrap_or(&0),
        }
    }
}

impl<'de> Deserialize<'de> for Offset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(PaddingVisitor(PhantomData))
    }
}

struct PaddingVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for PaddingVisitor<T>
where
    T: Deserialize<'de> + From<HashMap<String, u8>> + From<Vec<u8>> + From<i64>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "Expected the padding value as table or CSS-like value - [top, right, left, bottom]"
        )
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.into())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut fields = vec![];
        while let Some(value) = seq.next_element::<u8>()? {
            fields.push(value);
        }

        match fields.len() {
            1..=4 => Ok(fields.into()),
            other => Err(serde::de::Error::invalid_length(other, &self)),
        }
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        const POSSIBLE_KEYS: [&str; 6] =
            ["top", "right", "bottom", "left", "horizontal", "vertical"];
        let mut custom_padding = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, u8>()? {
            if !POSSIBLE_KEYS.contains(&key.as_str()) {
                return Err(serde::de::Error::invalid_value(
                    serde::de::Unexpected::Str(key.as_str()),
                    &self,
                ));
            }

            match key.as_str() {
                "top" | "bottom" if custom_padding.contains_key("vertical") => {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(key.as_str()),
                        &self,
                    ))
                }
                "vertical"
                    if custom_padding.contains_key("top")
                        || custom_padding.contains_key("bottom") =>
                {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(key.as_str()),
                        &self,
                    ))
                }
                "right" | "left" if custom_padding.contains_key("horizontal") => {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(key.as_str()),
                        &self,
                    ))
                }
                "horizontal"
                    if custom_padding.contains_key("right")
                        || custom_padding.contains_key("left") =>
                {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(key.as_str()),
                        &self,
                    ))
                }
                _ => (),
            }

            custom_padding.insert(key, value);
        }

        if !custom_padding.is_empty() {
            Ok(custom_padding.into())
        } else {
            Err(serde::de::Error::invalid_length(0, &self))
        }
    }
}
