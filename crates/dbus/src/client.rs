use log::debug;
use std::collections::HashMap;
use zbus::{proxy, zvariant::Value, Connection};

#[proxy(
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait Notifications {
    #[allow(clippy::too_many_arguments)]
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

impl Client<'_> {
    pub async fn init() -> anyhow::Result<Self> {
        debug!("D-Bus Client: Initializing");
        let connection = Connection::session().await?;
        let proxy = NotificationsProxy::new(&connection).await?;

        debug!("D-Bus Client: Initialized");
        Ok(Self { proxy })
    }

    #[allow(clippy::too_many_arguments)]
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
        debug!("D-Bus Client: Trying to notify");
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

        debug!("D-Bus Client: Notified");
        Ok(reply)
    }

    pub async fn get_server_information(&self) -> anyhow::Result<(String, String, String, String)> {
        debug!("D-Bus Client: Trying to get server information");
        let reply = self.proxy.get_server_information().await?;

        debug!("D-Bus Client: Receieved server information");
        Ok(reply)
    }
}
