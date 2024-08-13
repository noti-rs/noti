use core::panic;
use std::collections::HashMap;

use zbus::zvariant::Value;

pub struct NotificationData<'a> {
    pub id: u32,
    pub app_name: &'a str,
    pub icon: &'a str,
    pub summary: &'a str,
    pub body: &'a str,
    pub hints: &'a str,
    pub timeout: i32,
    pub urgency: &'a str,
    pub category: &'a str,
}

pub struct NotiClient<'a> {
    dbus_client: dbus::client::Client<'a>,
}

impl<'a> NotiClient<'a> {
    pub async fn init() -> Self {
        let client = dbus::client::Client::init().await.unwrap();
        Self {
            dbus_client: client,
        }
    }

    pub async fn send_notification(&self, data: NotificationData<'a>) -> anyhow::Result<()> {
        let hints = Self::build_hints(&data.hints, data.urgency, data.category)?;

        let mut actions = Vec::new();

        self.dbus_client
            .notify(
                &data.app_name,
                data.id,
                &data.icon,
                &data.summary,
                &data.body,
                actions,
                hints,
                data.timeout,
            )
            .await?;

        Ok(())
    }

    pub async fn get_server_info(&self) -> anyhow::Result<()> {
        let server_info = self.dbus_client.get_server_information().await.unwrap();
        println!(
            "Name: {}\nVendor: {}\nVersion: {}\nSpecification version: {}",
            server_info.0, server_info.1, server_info.2, server_info.3
        );

        Ok(())
    }

    fn build_hints(
        hints_string: &'a str,
        urgency: &'a str,
        category: &'a str,
    ) -> anyhow::Result<HashMap<&'a str, Value<'a>>> {
        let mut hints: HashMap<&str, Value> = HashMap::new();

        let entries = hints_string.split(';');

        for entry in entries.to_owned() {
            let parts: Vec<&str> = entry.split(':').collect();

            if parts.len() == 3 {
                let hint_type = parts[0].trim();
                let hint_name = parts[1].trim();
                let hint_value = parts[2].trim();

                hints.insert(
                    hint_name,
                    Value::from(match hint_type {
                        "int" => Value::I32(hint_value.parse::<i32>()?),
                        "bool" => Value::Bool(hint_value.parse::<bool>()?),
                        "string" => Value::from(hint_value),
                        _ => anyhow::bail!(
                            "Invalid hint type \"{}\". Valid types are int, bool and string",
                            hint_type
                        ),
                    }),
                );
            }
        }

        hints.entry("urgency").or_insert(Value::from(urgency));
        hints.entry("category").or_insert(Value::from(category));

        Ok(hints)
    }
}
