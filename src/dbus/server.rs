use crate::notification::{Notification, Urgency};
use std::{
    collections::HashMap,
    error::Error,
    future::pending,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{self, Sender};
use zbus::{connection, interface, zvariant::Value};

const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";

enum Method {
    CloseNotification { notification_id: u32 },
    Notify { notification: Notification },
    GetCapabilities,
    GetServerInformation,
}

struct NotificationHandler {
    count: u32,
    sender: Sender<Method>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationHandler {
    #[dbus_interface(name = "Notify")]
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: HashMap<String, Value<'_>>,
        expire_timeout: i32,
    ) {
        self.count += 1;

        dbg!(expire_timeout);
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

        // TODO: 1. parse urgency
        // TODO: 2. parse actions & hints

        let notification = Notification {
            id: replaces_id,
            name: app_name,
            icon: app_icon,
            is_read: false,
            urgency: Urgency::default(),
            summary,
            body,
            expire_timeout,
            created_at,
        };

        self.sender
            .send(Method::Notify { notification })
            .await
            .unwrap();
    }

    #[dbus_interface(name = "CloseNotification")]
    async fn close_notification(&mut self, id: u32) {
        self.sender
            .send(Method::CloseNotification {
                notification_id: id,
            })
            .await
            .unwrap();
    }

    #[dbus_interface(name = "GetServerInformation")]
    async fn get_server_information(
        &mut self,
    ) -> zbus::fdo::Result<(String, String, String, String)> {
        let name = String::from("Notification Daemon");
        let vendor = String::from(env!("CARGO_PKG_NAME"));
        let version = String::from(env!("CARGO_PKG_VERSION"));
        let specification_version = String::from("1.2");

        self.sender
            .send(Method::GetServerInformation)
            .await
            .unwrap();
        Ok((name, vendor, version, specification_version))
    }

    #[dbus_interface(name = "GetCapabilities")]
    async fn get_capabilities(&mut self) -> zbus::fdo::Result<Vec<&str>> {
        // https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#protocol

        let capabilities = vec![
            "action-icons",
            "actions",
            "body",
            "body-hyperlinks",
            "body-images",
            "body-markup",
            "icon-multi",
            "icon-static",
            "persistence",
            "sound",
        ];

        self.sender.send(Method::GetCapabilities).await.unwrap();
        Ok(capabilities)
    }
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    let (method_sender, mut method_receiver) = mpsc::channel(5);

    let handler = NotificationHandler {
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
                }
                Method::CloseNotification { notification_id } => {
                    dbg!(notification_id);
                }
                Method::GetCapabilities => (),
                Method::GetServerInformation => (),
            }
        }
    });

    // Do other things or go to wait forever
    pending::<()>().await;
    Ok(())
}
