use std::collections::HashMap;
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
    ) -> anyhow::Result<u32>;

    async fn get_server_information(&self) -> anyhow::Result<(String, String, String, String)>;
}

pub struct Client<'a> {
    proxy: NotificationsProxy<'a>,
}

impl<'a> Client<'a> {
    pub async fn init() -> anyhow::Result<Self> {
        let connection = Connection::session().await?;
        let proxy = NotificationsProxy::new(&connection).await?;

        Ok(Self { proxy })
    }

    pub async fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> anyhow::Result<u32> {
        let reply = self
            .proxy
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

    pub async fn get_server_information(&self) -> anyhow::Result<(String, String, String, String)> {
        let reply = self.proxy.get_server_information().await?;
        Ok(reply)
    }
}
