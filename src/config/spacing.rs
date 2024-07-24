use std::{collections::HashMap, marker::PhantomData};

use serde::{de::Visitor, Deserialize};

#[derive(Debug, Default, Clone)]
pub struct Spacing {
    top: u8,
    right: u8,
    bottom: u8,
    left: u8,
}

impl Spacing {
    const POSSIBLE_KEYS: [&'static str; 6] =
        ["top", "right", "bottom", "left", "vertical", "horizontal"];

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

    pub fn shrink(&self, width: &mut usize, height: &mut usize) {
        *width -= self.left as usize + self.right as usize;
        *height -= self.top as usize + self.bottom as usize;
    }
}

impl From<i64> for Spacing {
    fn from(value: i64) -> Self {
        Spacing::all_directional(value.clamp(0, u8::MAX as i64) as u8)
    }
}

impl From<Vec<u8>> for Spacing {
    fn from(value: Vec<u8>) -> Self {
        match value.len() {
            1 => Spacing::all_directional(value[0]),
            2 => Spacing::cross(value[0], value[1]),
            3 => Spacing {
                top: value[0],
                right: value[1],
                left: value[1],
                bottom: value[2],
            },
            4 => Spacing {
                top: value[0],
                right: value[1],
                bottom: value[2],
                left: value[3],
            },
            _ => unreachable!(),
        }
    }
}

impl From<HashMap<String, u8>> for Spacing {
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

impl<'de> Deserialize<'de> for Spacing {
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
            r#"either u8, [u8, u8], [u8, u8, u8], [u8, u8, u8, u8] or Table.

Example:

# All-directional margin
margin = 3

# The application can also apply the CSS-like values:
# Applies vertical and horizontal paddings respectively
padding = [0, 5]

# Applies top, horizontal and bottom paddings respectively
margin = [3, 2, 5]

# Applies top, right, bottom, left paddings respectively
padding = [1, 2, 3, 4]

# When you want to declare in explicit way:

# Sets only top padding
padding = {{ top = 3 }}

# Sets only top and right padding
padding = {{ top = 5, right = 6 }}

# Insead of
# padding = {{ top = 5, right = 6, bottom = 5 }}
# Write
padding = {{ vertical = 5, right = 6 }}

# If gots collision of values the error will throws because of ambuguity
# padding = {{ top = 5, vertical = 6 }}

# You can apply the same way for margin
margin = {{ top = 5, horizontal = 10 }}"#
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
        let mut custom_padding = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, u8>()? {
            if !Spacing::POSSIBLE_KEYS.contains(&key.as_str()) {
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
