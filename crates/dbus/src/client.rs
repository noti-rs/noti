use log::debug;
use std::collections::HashMap;
use zbus::{proxy, zvariant::Value, Connection};

/// Proxy for the D-Bus notification endpoint.
///
/// All functions follow [the Freedesktop notification specification](https://specifications.freedesktop.org/notification-spec/latest/).
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

/// Represents a client connected to a D-Bus notification server via a specified endpoint.
pub struct Client<'a> {
    proxy: NotificationsProxy<'a>,
}

impl Client<'_> {
    /// Connects to a D-Bus notification server.
    ///
    /// Initialization may fail if no D-Bus notification server is available.
    pub async fn init() -> anyhow::Result<Self> {
        debug!("D-Bus Client: Initializing");
        let connection = Connection::session().await?;
        let proxy = NotificationsProxy::new(&connection).await?;

        debug!("D-Bus Client: Initialized");
        Ok(Self { proxy })
    }

    /// Sends a notification to a D-Bus notification server.
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

    /// Requests information about the server from a D-Bus notification server.
    pub async fn get_server_information(&self) -> anyhow::Result<(String, String, String, String)> {
        debug!("D-Bus Client: Trying to get server information");
        let reply = self.proxy.get_server_information().await?;

        debug!("D-Bus Client: Receieved server information");
        Ok(reply)
    }
}
