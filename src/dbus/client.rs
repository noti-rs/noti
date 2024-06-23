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
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> Result<u32>;
}

#[tokio::main]
async fn main() -> Result<()> {
    // let mut hints = HashMap::new();
    // hints.insert("urgency", Value::from(urgency));

    let connection = Connection::session().await?;

    let mut proxy = NotificationsProxy::new(&connection).await?;
    let reply = proxy
        .notify(
            // TODO: parse input
        )
        .await?;
    dbg!(reply);

    Ok(())
}
