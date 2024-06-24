use crate::notification::{Notification, Urgency};
use std::{
    collections::HashMap,
    future::pending,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{self, Sender};
use zbus::{connection, fdo::Result, interface, zvariant::Value};

const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";

enum Method {
    CloseNotification { id: u32 },
    Notify { notification: Notification },
    GetCapabilities,
    GetServerInformation,
}

struct Handler {
    count: u32,
    sender: Sender<Method>,
}

// generated via zbus-xmlgen
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

// NOTE: https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html
#[interface(name = "org.freedesktop.Notifications")]
impl Notifications for Handler {
    async fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
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
        // TODO: parse hints
        // TODO: handle image data
        // TODO: handle desktop entry

        // TODO: handle actions

        let notification = Notification {
            id,
            app_name: app_name.to_string(),
            app_icon: app_icon.to_string(),
            is_read: false,
            urgency,
            summary: summary.to_string(),
            body: body.to_string(),
            expire_timeout,
            created_at,
        };

        self.sender
            .send(Method::Notify { notification })
            .await
            .unwrap();

        Ok(id)
    }

    async fn close_notification(&self, id: u32) -> Result<()> {
        self.sender
            .send(Method::CloseNotification { id })
            .await
            .unwrap();

        Ok(())
    }

    async fn get_server_information(&self) -> Result<(String, String, String, String)> {
        let name = String::from(env!("CARGO_PKG_NAME"));
        let vendor = String::from(env!("CARGO_PKG_NAME"));
        let version = String::from(env!("CARGO_PKG_VERSION"));
        let specification_version = String::from("1.2");

        self.sender
            .send(Method::GetServerInformation)
            .await
            .unwrap();
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

        self.sender.send(Method::GetCapabilities).await.unwrap();
        Ok(capabilities)
    }
}

pub async fn run() -> Result<()> {
    let (method_sender, mut method_receiver) = mpsc::channel(5);

    let handler = Handler {
        count: 0,
        sender: method_sender,
    };

    let _conn = connection::Builder::session()?
        .name(NOTIFICATIONS_NAME)?
        .serve_at(NOTIFICATIONS_PATH, handler)?
        .build()
        .await?;

    tokio::spawn(async move {
        while let Some(method) = method_receiver.recv().await {
            match method {
                Method::Notify { notification } => {
                    dbg!(notification);
                    println!("");
                }
                Method::CloseNotification { id } => {
                    dbg!(id);
                }
                Method::GetCapabilities => (),
                Method::GetServerInformation => (),
            }
        }
    });

    // TODO: signals handling

    // Do other things or go to wait forever
    pending::<()>().await;
    Ok(())
}
