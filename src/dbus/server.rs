use crate::notification::{Action, Notification, Urgency};
use std::{
    collections::HashMap,
    future::pending,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{self, Sender};
use zbus::{connection, fdo::Result, interface, zvariant::Value, Connection};

const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";

pub struct Server {
    connection: Connection,
}

struct Handler {
    count: u32,
    sender: Sender<Action>,
}

impl Handler {
    fn init(sender: Sender<Action>) -> Self {
        Self { count: 0, sender }
    }
}

/// Represents org.freedesktop.Notifications DBus interface
trait Notifications {
    /// CloseNotification method
    async fn close_notification(&self, id: u32) -> Result<()>;

    /// GetCapabilities method
    async fn get_capabilities(&self) -> Result<Vec<String>>;

    /// GetServerInformation method
    async fn get_server_information(&self) -> Result<(String, String, String, String)>;

    /// Notify method
    #[allow(clippy::too_many_arguments)]
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

#[interface(name = "org.freedesktop.Notifications")]
impl Notifications for Handler {
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> Result<u32> {
        let id = if replaces_id == 0 {
            self.count += 1;
            self.count
        } else {
            replaces_id
        };

        let expire_timeout = if expire_timeout != -1 {
            match expire_timeout.try_into() {
                Ok(v) => Some(Duration::from_millis(v)),
                Err(_) => None,
            }
        } else {
            None
        };

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut urgency = Urgency::default();
        if let Some(value) = hints.get("urgency") {
            if let Value::U32(val) = value {
                urgency = Urgency::from(val.to_owned());
            };
        };

        // TODO: parse other hints
        // TODO: handle image data
        // TODO: handle desktop entry

        // TODO: handle actions

        let notification = Notification {
            id,
            app_name,
            app_icon,
            urgency,
            summary,
            body,
            expire_timeout,
            created_at,
            is_read: false,
        };

        self.sender.send(Action::Show(notification)).await.unwrap();
        Ok(id)
    }

    async fn close_notification(&self, id: u32) -> Result<()> {
        self.sender.send(Action::Close(Some(id))).await.unwrap();

        Ok(())
    }

    async fn get_server_information(&self) -> Result<(String, String, String, String)> {
        let name = String::from(env!("CARGO_PKG_NAME"));
        let vendor = String::from(env!("CARGO_PKG_AUTHORS"));
        let version = String::from(env!("CARGO_PKG_VERSION"));
        let specification_version = String::from("1.2");

        Ok((name, vendor, version, specification_version))
    }

    async fn get_capabilities(&self) -> Result<Vec<String>> {
        let capabilities = vec![
            // String::from("action-icons"),
            // String::from("actions"),
            String::from("body"),
            // String::from("body-hyperlinks"),
            // String::from("body-images"),
            // String::from("body-markup"),
            // String::from("icon-multi"),
            // String::from("icon-static"),
            // String::from("persistence"),
            // String::from("sound"),
        ];

        Ok(capabilities)
    }
}

impl Server {
    pub async fn init(sender: Sender<Action>) -> Result<Self> {
        let handler = Handler::init(sender);
        let connection = connection::Builder::session()?
            .name(NOTIFICATIONS_NAME)?
            .serve_at(NOTIFICATIONS_PATH, handler)?
            .build()
            .await?;

        Ok(Self { connection })
    }
}
