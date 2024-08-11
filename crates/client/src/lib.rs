use std::collections::HashMap;

use zbus::zvariant::Value;

pub struct NotificationData {
    pub id: u32,
    pub app_name: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub hints: String,
    pub timeout: i32,
    pub urgency: u8,
    pub category: String,
}

pub async fn send_notification(data: NotificationData) -> anyhow::Result<()> {
    let client = dbus::client::Client::init().await?;

    let mut hints = HashMap::new();
    hints.entry("urgency").or_insert(Value::from(data.urgency));
    hints
        .entry("urgency")
        .or_insert(Value::from(&data.category));

    let mut actions = Vec::new();

    client
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
