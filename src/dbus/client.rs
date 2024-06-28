use std::collections::HashMap;
use std::u32;
use zbus::fdo::Result;
use zbus::{proxy, zvariant::Value, Connection};

#[proxy(
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait Notifications {
    async fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
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
        &self,
        replaces_id: Option<u32>,
        app_icon: Option<&str>,
        summary: &str,
        body: Option<&str>,
        urgency: Option<u32>,
        expire_timeout: Option<i32>,
    ) -> Result<u32> {
        let proxy = NotificationsProxy::new(&self.connection).await?;

        let app_name = "Noti";

        let id = match replaces_id {
            Some(id) => id,
            None => 0,
        };

        let app_icon = match app_icon {
            Some(s) => s,
            None => "",
        };

        let body = match body {
            Some(s) => s,
            None => "",
        };

        let expire_timeout = match expire_timeout {
            Some(i) => i,
            None => 2000, // TODO: set value from user config
        };

        let mut hints = HashMap::new();
        if let Some(urgency) = urgency {
            hints.insert("urgency", Value::from(urgency));
        }

        let actions = Vec::new();

        let reply = proxy
            .notify(
                app_name,
                id,
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
