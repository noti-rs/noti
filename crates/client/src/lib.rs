use anyhow::bail;
use log::debug;
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

    #[allow(clippy::too_many_arguments)]
    pub async fn send_notification(
        &self,
        id: u32,
        app_name: String,
        icon: String,
        summary: String,
        body: String,
        timeout: i32,
        actions: Vec<String>,
        hints: Vec<String>,
        hints_data: HintsData,
    ) -> anyhow::Result<()> {
        debug!("Client: Building hints and actions from user prompt");
        let new_hints = build_hints(&hints, hints_data)?;
        let actions = build_actions(&actions)?;

        debug!(
            "Client: Send notification with metadata:\n\
            \treplaces_id - {id},\n\
            \tapp_name - {app_name},\n\
            \tapp_icon - {icon},\n\
            \tsummary - {summary},\n\
            \tbody - {body},\n\
            \tactions - {actions:?},\n\
            \thints - {new_hints:?},\n\
            \ttimeout - {timeout}"
        );

        self.dbus_client
            .notify(
                &app_name, id, &icon, &summary, &body, actions, new_hints, timeout,
            )
            .await?;

        debug!("Client: Successful send");

        Ok(())
    }

    pub async fn get_server_info(&self) -> anyhow::Result<()> {
        debug!("Client: Trying to request server information");
        let server_info = self.dbus_client.get_server_information().await?;
        debug!("Client: Received server information");

        println!(
            "Name: {}\nVendor: {}\nVersion: {}\nSpecification version: {}",
            server_info.0, server_info.1, server_info.2, server_info.3
        );

        Ok(())
    }
}

fn build_actions(actions: &[String]) -> anyhow::Result<Vec<&str>> {
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

fn build_hints<'a>(
    hints: &'a [String],
    hints_data: HintsData,
) -> anyhow::Result<HashMap<&'a str, Value<'a>>> {
    let mut hints_map: HashMap<&'a str, Value<'a>> = HashMap::with_capacity(hints.len());

    for entry in hints {
        let parts: Vec<&'a str> = entry.split(':').collect();

        if parts.len() == 3 {
            let hint_type = parts[0].trim();
            let hint_name = parts[1].trim();
            let hint_value = parts[2].trim();

            let value = parse_hint_value(hint_type, hint_value)?;
            hints_map.insert(hint_name, value);
        } else {
            bail!(
                "Invalid hint format for entry '{}'. Expected format: 'type:name:value'",
                entry
            );
        }
    }

    hints_map.insert_if_empty("urgency", hints_data.urgency, Value::from);
    hints_map.insert_if_empty("category", hints_data.category, Value::from);
    hints_map.insert_if_empty("desktop-entry", hints_data.desktop_entry, Value::from);
    hints_map.insert_if_empty("image-path", hints_data.image_path, Value::from);
    hints_map.insert_if_empty("sound-file", hints_data.sound_file, Value::from);
    hints_map.insert_if_empty("sound-name", hints_data.sound_name, Value::from);
    hints_map.insert_if_empty("resident", hints_data.resident, Value::Bool);
    hints_map.insert_if_empty("suppress-sound", hints_data.suppress_sound, Value::Bool);
    hints_map.insert_if_empty("transient", hints_data.transient, Value::Bool);
    hints_map.insert_if_empty("action-icons", hints_data.action_icons, Value::Bool);

    Ok(hints_map)
}

fn parse_hint_value<'a>(hint_type: &'_ str, hint_value: &'a str) -> anyhow::Result<Value<'a>> {
    Ok(match hint_type {
        "int" => Value::I32(hint_value.parse()?),
        "byte" => Value::U8(hint_value.parse()?),
        "bool" => Value::Bool(hint_value.parse()?),
        "string" => Value::from(hint_value),
        _ => anyhow::bail!(
            "Invalid hint type \"{}\". Valid types are int, byte, bool, and string.",
            hint_type
        ),
    })
}

trait InsertIfEmpty<K, V>
where
    K: std::cmp::Eq + std::hash::Hash,
{
    fn insert_if_empty<T, F>(&mut self, key: K, value: Option<T>, conversion: F)
    where
        F: FnOnce(T) -> V;
}

impl<K, V> InsertIfEmpty<K, V> for HashMap<K, V>
where
    K: std::cmp::Eq + std::hash::Hash,
{
    fn insert_if_empty<'b, T, F>(&mut self, key: K, value: Option<T>, conversion: F)
    where
        F: FnOnce(T) -> V,
    {
        let Some(value) = value else {
            return;
        };

        self.entry(key).or_insert_with(|| conversion(value));
    }
}
