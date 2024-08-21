use anyhow::bail;
use std::collections::HashMap;
use zbus::zvariant::Value;

pub struct HintsData {
    pub urgency: Option<String>,
    pub category: Option<String>,
    pub desktop_entry: Option<String>,
    pub image_path: Option<String>,
    pub resident: Option<bool>,
    pub sound_file: Option<String>,
    pub sound_name: Option<String>,
    pub suppress_sound: Option<bool>,
    pub transient: Option<bool>,
    pub action_icons: Option<bool>,
}

pub struct NotiClient<'a> {
    dbus_client: dbus::client::Client<'a>,
}

impl<'a> NotiClient<'a> {
    pub async fn init() -> anyhow::Result<Self> {
        let client = dbus::client::Client::init().await?;
        Ok(Self {
            dbus_client: client,
        })
    }

    pub async fn send_notification(
        &self,
        id: u32,
        app_name: &'a str,
        icon: &'a str,
        summary: &'a str,
        body: &'a str,
        timeout: i32,
        actions: &'a Vec<String>,
        hints: &'a Vec<String>,
        hints_data: &'a HintsData,
    ) -> anyhow::Result<()> {
        let new_hints = Self::build_hints(&hints, &hints_data)?;
        let actions = Self::build_actions(actions)?;

        self.dbus_client
            .notify(
                &app_name, id, &icon, &summary, &body, actions, new_hints, timeout,
            )
            .await?;

        Ok(())
    }

    pub async fn get_server_info(&self) -> anyhow::Result<()> {
        let server_info = self.dbus_client.get_server_information().await?;
        println!(
            "Name: {}\nVendor: {}\nVersion: {}\nSpecification version: {}",
            server_info.0, server_info.1, server_info.2, server_info.3
        );

        Ok(())
    }

    fn build_actions(actions: &'a [String]) -> anyhow::Result<Vec<&'a str>> {
        let mut new_actions = Vec::with_capacity(actions.len() * 2);

        for entry in actions {
            if let Some((action_name, action_desc)) = entry.split_once(':') {
                new_actions.push(action_name.trim());
                new_actions.push(action_desc.trim());
            } else {
                bail!(
                    "Invalid action format for entry '{}'. Expected format: 'name:desc'",
                    entry
                );
            }
        }

        Ok(new_actions)
    }

    fn build_hints(
        hints: &'a [String],
        hints_data: &'a HintsData,
    ) -> anyhow::Result<HashMap<&'a str, Value<'a>>> {
        let mut hints_map: HashMap<&str, Value> = HashMap::with_capacity(hints.len());

        for entry in hints {
            let parts: Vec<&'a str> = entry.split(':').collect();

            if parts.len() == 3 {
                let hint_type = parts[0].trim();
                let hint_name = parts[1].trim();
                let hint_value = parts[2].trim();

                let value = Self::parse_hint_value(hint_type, hint_value)?;
                hints_map.insert(hint_name, value);
            } else {
                bail!(
                    "Invalid hint format for entry '{}'. Expected format: 'type:name:value'",
                    entry
                );
            }
        }

        Self::or_insert_hints(&mut hints_map, &hints_data)?;

        Ok(hints_map)
    }

    fn parse_hint_value(hint_type: &'a str, hint_value: &'a str) -> anyhow::Result<Value<'a>> {
        match hint_type {
            "int" => Ok(Value::I32(hint_value.parse()?)),
            "uint" => Ok(Value::U32(hint_value.parse()?)),
            "bool" => Ok(Value::Bool(hint_value.parse()?)),
            "string" => Ok(Value::from(hint_value)),
            _ => anyhow::bail!(
                "Invalid hint type \"{}\". Valid types are int, uint, bool, and string.",
                hint_type
            ),
        }
    }

    fn or_insert_hints(
        hints: &mut HashMap<&'a str, Value<'a>>,
        hints_data: &'a HintsData,
    ) -> anyhow::Result<()> {
        if let Some(urgency) = &hints_data.urgency {
            hints
                .entry("urgency")
                .or_insert(Value::U32(match urgency.to_lowercase().as_str() {
                    "low" => 0,
                    "normal" => 1,
                    "critical" => 2,
                    _ => anyhow::bail!(
                        "Invalid urgency value: {}. Valid values are low, normal and critical.",
                        { &urgency }
                    ),
                }));
        }

        if let Some(category) = &hints_data.category {
            hints
                .entry("category")
                .or_insert(Value::from(category.as_str()));
        }

        if let Some(desktop_entry) = &hints_data.desktop_entry {
            hints
                .entry("desktop-entry")
                .or_insert(Value::from(desktop_entry.as_str()));
        }

        if let Some(image_path) = &hints_data.image_path {
            hints
                .entry("image-path")
                .or_insert(Value::from(image_path.as_str()));
        }

        if let Some(resident) = hints_data.resident {
            hints.entry("resident").or_insert(Value::Bool(resident));
        }

        if let Some(sound_file) = &hints_data.sound_file {
            hints
                .entry("sound-file")
                .or_insert(Value::from(sound_file.as_str()));
        }

        if let Some(sound_name) = &hints_data.sound_name {
            hints
                .entry("sound-name")
                .or_insert(Value::from(sound_name.as_str()));
        }

        if let Some(suppress_sound) = hints_data.suppress_sound {
            hints
                .entry("suppress-sound")
                .or_insert(Value::Bool(suppress_sound));
        }

        if let Some(transient) = hints_data.transient {
            hints.entry("transient").or_insert(Value::Bool(transient));
        }

        if let Some(action_icons) = hints_data.action_icons {
            hints
                .entry("action-icons")
                .or_insert(Value::Bool(action_icons));
        }

        Ok(())
    }
}
