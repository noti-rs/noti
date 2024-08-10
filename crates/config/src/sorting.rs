use std::{cmp::Ordering, collections::HashMap, marker::PhantomData};

use serde::{de::Visitor, Deserialize};

use dbus::notification::Notification;

#[derive(Debug, Default, Clone)]
pub struct Sorting {
    by: SortBy,
    ordering: CmpOrdering,
}

impl Sorting {
    pub fn get_cmp<T>(&self) -> for<'a, 'b> fn(&'a T, &'b T) -> Ordering
    where
        for<'a> &'a T: Into<&'a Notification>,
    {
        match &self.by {
            SortBy::Id => match &self.ordering {
                CmpOrdering::Ascending => Self::cmp_by_id,
                CmpOrdering::Descending => |lhs, rhs| Self::cmp_by_id(rhs, lhs),
            },
            SortBy::Urgency => match &self.ordering {
                CmpOrdering::Ascending => Self::cmp_by_urgency,
                CmpOrdering::Descending => |lhs, rhs| Self::cmp_by_urgency(rhs, lhs),
            },
            SortBy::Time => match &self.ordering {
                CmpOrdering::Ascending => Self::cmp_by_time,
                CmpOrdering::Descending => |lhs, rhs| Self::cmp_by_time(rhs, lhs),
            },
        }
    }

    fn cmp_by_id<T>(lhs: &T, rhs: &T) -> Ordering
    where
        for<'a> &'a T: Into<&'a Notification>,
    {
        Into::<&Notification>::into(lhs)
            .id
            .cmp(&Into::<&Notification>::into(rhs).id)
    }

    fn cmp_by_urgency<T>(lhs: &T, rhs: &T) -> Ordering
    where
        for<'a> &'a T: Into<&'a Notification>,
    {
        Into::<&Notification>::into(lhs)
            .hints
            .urgency
            .cmp(&Into::<&Notification>::into(rhs).hints.urgency)
    }

    fn cmp_by_time<T>(lhs: &T, rhs: &T) -> Ordering
    where
        for<'a> &'a T: Into<&'a Notification>,
    {
        Into::<&Notification>::into(lhs)
            .created_at
            .cmp(&Into::<&Notification>::into(rhs).created_at)
    }
}

#[derive(Debug, Default, Clone)]
pub enum SortBy {
    Id,
    Urgency,
    #[default]
    Time,
}

impl SortBy {
    const POSSIBLE_VALUES: [&'static str; 4] = ["default", "id", "urgency", "time"];
}

impl From<&String> for SortBy {
    fn from(value: &String) -> Self {
        match value.as_str() {
            "default" => Default::default(),
            "id" => SortBy::Id,
            "urgency" => SortBy::Urgency,
            "time" => SortBy::Time,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum CmpOrdering {
    #[default]
    Ascending,
    Descending,
}

impl CmpOrdering {
    const POSSIBLE_VALUES: [&'static str; 4] = ["ascending", "asc", "descending", "desc"];
}

impl From<&String> for CmpOrdering {
    fn from(value: &String) -> Self {
        match value.as_str() {
            "ascending" | "asc" => CmpOrdering::Ascending,
            "descending" | "desc" => CmpOrdering::Descending,
            _ => unreachable!(),
        }
    }
}

impl From<String> for Sorting {
    fn from(value: String) -> Self {
        match value.as_str() {
            "id" => Sorting {
                by: SortBy::Id,
                ..Default::default()
            },
            "urgency" => Sorting {
                by: SortBy::Urgency,
                ..Default::default()
            },
            _ => Default::default(),
        }
    }
}

impl From<HashMap<String, String>> for Sorting {
    fn from(map: HashMap<String, String>) -> Self {
        Self {
            by: map
                .get("by")
                .map(|value| value.into())
                .expect("Must be by key from deserializer!"),
            ordering: map
                .get("ordering")
                .map(|value| value.into())
                .unwrap_or_default(),
        }
    }
}

struct SortingVisitor<T>(PhantomData<fn() -> T>);

impl<'de> Deserialize<'de> for Sorting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(SortingVisitor(PhantomData))
    }
}

impl<'de, T> Visitor<'de> for SortingVisitor<T>
where
    T: Deserialize<'de> + From<String> + From<HashMap<String, String>>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let possible_values = SortBy::POSSIBLE_VALUES
            .iter()
            .fold(String::new(), |acc, val| {
                if acc.len() == 0 {
                    acc + "\"" + val + "\""
                } else {
                    acc + " | \"" + val + "\""
                }
            });
        write!(
            formatter,
            r#"either String or Table value.
Example:

sorting = "id"
# or
sorting = {{ by = "id" }}
# or
sorting = {{ by = "id", ordering = "descending" }}

Possible values:
sorting = {possible_values}

sorting = {{
    by: String = {possible_values}
    ordering: String? = "ascending" | "asc" | "descending" | "desc"
}}"#
        )
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if SortBy::POSSIBLE_VALUES.contains(&v.as_str()) {
            Ok(v.into())
        } else {
            Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&v),
                &self,
            ))
        }
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut local_map = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, String>()? {
            match key.as_str() {
                "by" => {
                    if SortBy::POSSIBLE_VALUES.contains(&value.as_str()) {
                        local_map.insert(key, value);
                    } else {
                        return Err(serde::de::Error::invalid_value(
                            serde::de::Unexpected::Str(&value),
                            &self,
                        ));
                    }
                }
                "ordering" => {
                    if CmpOrdering::POSSIBLE_VALUES.contains(&value.as_str()) {
                        local_map.insert(key, value);
                    } else {
                        return Err(serde::de::Error::invalid_value(
                            serde::de::Unexpected::Str(&value),
                            &self,
                        ));
                    }
                }
                _ => return Err(serde::de::Error::unknown_variant(&key, &["by", "ordering"])),
            }
        }

        if !local_map.contains_key("by") {
            return Err(serde::de::Error::missing_field("by"));
        }

        Ok(local_map.into())
    }
}
