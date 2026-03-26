use std::{
    collections::HashMap,
    marker::PhantomData,
    ops::{Add, AddAssign},
};

use macros::GenericBuilder;
use serde::{de::Visitor, Deserialize};
use shared::value::TryFromValue;

#[derive(GenericBuilder, Debug, Default, Clone, Copy)]
#[gbuilder(name(GBuilderSpacing), derive(Clone), constructor)]
pub struct Spacing {
    #[gbuilder(default(0), aliases(vertical, all))]
    top: usize,

    #[gbuilder(default(0), aliases(horizontal, all))]
    right: usize,

    #[gbuilder(default(0), aliases(vertical, all))]
    bottom: usize,

    #[gbuilder(default(0), aliases(horizontal, all))]
    left: usize,
}

impl TryFromValue for Spacing {}

impl Spacing {
    const POSSIBLE_KEYS: [&'static str; 6] =
        ["top", "right", "bottom", "left", "vertical", "horizontal"];

    pub fn all_directional(val: usize) -> Self {
        Self {
            top: val,
            bottom: val,
            right: val,
            left: val,
        }
    }

    pub fn cross(vertical: usize, horizontal: usize) -> Self {
        Self {
            top: vertical,
            bottom: vertical,
            right: horizontal,
            left: horizontal,
        }
    }

    pub fn top(&self) -> usize {
        self.top
    }

    pub fn set_top(&mut self, top: usize) {
        self.top = top;
    }

    pub fn right(&self) -> usize {
        self.right
    }

    pub fn set_right(&mut self, right: usize) {
        self.right = right;
    }

    pub fn bottom(&self) -> usize {
        self.bottom
    }

    pub fn set_bottom(&mut self, bottom: usize) {
        self.bottom = bottom;
    }

    pub fn left(&self) -> usize {
        self.left
    }

    pub fn set_left(&mut self, left: usize) {
        self.left = left;
    }

    pub fn horizontal(&self) -> u16 {
        self.left as u16 + self.right as u16
    }

    pub fn vertical(&self) -> u16 {
        self.top as u16 + self.bottom as u16
    }

    pub fn shrink(&self, width: &mut usize, height: &mut usize) {
        *width = width.saturating_sub(self.left + self.right);
        *height = height.saturating_sub(self.top + self.bottom);
    }
}

impl Add<Spacing> for Spacing {
    type Output = Spacing;

    fn add(self, rhs: Spacing) -> Self::Output {
        Spacing {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl Add<Spacing> for &Spacing {
    type Output = Spacing;

    fn add(self, rhs: Spacing) -> Self::Output {
        Spacing {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl AddAssign<Spacing> for Spacing {
    fn add_assign(&mut self, rhs: Spacing) {
        self.top += rhs.top;
        self.right += rhs.right;
        self.bottom += rhs.bottom;
        self.left += rhs.left;
    }
}
impl From<i64> for Spacing {
    fn from(value: i64) -> Self {
        Spacing::all_directional(value.clamp(0, usize::MAX as i64) as usize)
    }
}

impl From<Vec<usize>> for Spacing {
    fn from(value: Vec<usize>) -> Self {
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

impl From<HashMap<String, usize>> for Spacing {
    fn from(map: HashMap<String, usize>) -> Self {
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
    T: Deserialize<'de> + From<HashMap<String, usize>> + From<Vec<usize>> + From<i64>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            r#"either usize, [usize, usize], [usize, usize, usize], [usize, usize, usize, usize] or Table.

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
        while let Some(value) = seq.next_element::<usize>()? {
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

        while let Some((key, value)) = map.next_entry::<String, usize>()? {
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
