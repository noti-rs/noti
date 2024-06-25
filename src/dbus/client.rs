use std::collections::HashMap;
use std::u32;
use zbus::fdo::Result;
use zbus::{proxy, zvariant::Value, Connection};

#[proxy(
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> Result<u32>;
}

pub struct Client {
    connection: Connection,
}

impl Client {
    pub async fn init() -> Result<Self> {
        let connection = Connection::session().await?;
        Ok(Self { connection })
    }

    pub async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        urgency: u32,
        expire_timeout: i32,
    ) -> Result<u32> {
        let mut proxy = NotificationsProxy::new(&self.connection).await?;

        let mut hints = HashMap::new();
        hints.insert("urgency", Value::from(urgency));

        let actions = Vec::new();

        let reply = proxy
            .notify(
                app_name,
                replaces_id,
                app_icon,
                summary,
                body,
                actions,
                hints,
                expire_timeout,
            )
            .await?;

        Ok(reply)
    }
}
