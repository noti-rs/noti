use std::{collections::HashMap, marker::PhantomData};

use serde::{Deserialize, de::Visitor};

#[derive(Debug, Clone, Copy)]
pub enum Size {
    Fixed(u16),
    Dynamic { min: u16, max: u16 },
}

impl Size {
    const POSSIBLE_KEYS: [&'static str; 2] = ["min", "max"];
}

impl From<i64> for Size {
    fn from(value: i64) -> Self {
        Self::Fixed(value.clamp(0, u16::MAX as i64) as u16)
    }
}

impl From<Vec<u16>> for Size {
    fn from(value: Vec<u16>) -> Self {
        match value.len() {
            1 => Self::Fixed(value[0]),
            2 => Self::Dynamic {
                min: value[0],
                max: value[1],
            },
            _ => unreachable!(),
        }
    }
}

impl From<HashMap<String, u16>> for Size {
    fn from(map: HashMap<String, u16>) -> Self {
        let min = map.get("min");
        let max = map.get("max");

        Self::Dynamic {
            min: *min.unwrap_or(&0),
            max: *max.unwrap_or(&0),
        }
    }
}

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(SizeVisitor(PhantomData))
    }
}

struct SizeVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for SizeVisitor<T>
where
    T: Deserialize<'de> + From<HashMap<String, u16>> + From<Vec<u16>> + From<i64>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            r#"either u16, [u16, u16] or Table.

Example:

# A fixed size.
width = 300

# A dynamic size where is 150 is minimal and 300 is maximal
width = [150, 300]

# When you want to declare in explicit way:
height = {{ min = 100, max = 150 }}

# It's applied only if minimal and maximal values are set.
"#
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
        while let Some(value) = seq.next_element::<u16>()? {
            fields.push(value);
        }

        match fields.len() {
            1..=2 => Ok(fields.into()),
            other => Err(serde::de::Error::invalid_length(other, &self)),
        }
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut table = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, u16>()? {
            if !Size::POSSIBLE_KEYS.contains(&key.as_str()) {
                return Err(serde::de::Error::invalid_value(
                    serde::de::Unexpected::Str(key.as_str()),
                    &self,
                ));
            }

            table.insert(key, value);
        }

        for possible_key in Size::POSSIBLE_KEYS {
            if !table.contains_key(possible_key) {
                return Err(serde::de::Error::missing_field(possible_key))
            }
        }

        if !table.is_empty() {
            Ok(table.into())
        } else {
            Err(serde::de::Error::invalid_length(0, &self))
        }
    }
}
