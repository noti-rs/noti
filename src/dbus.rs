pub mod server {
    use crate::notification::{Notification, Urgency};
    use std::{
        collections::HashMap,
        error::Error,
        future::pending,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tokio::sync::mpsc::{self, Sender};
    use zbus::{connection, interface, zvariant::Value};

    #[derive(Debug)]
    struct NotificationHandler {
        count: u32,
        sender: Sender<Method>,
    }

    enum Method {
        CloseNotification { notification_id: u32 },
        Notify { notification: Notification },
        GetCapabilities,
        GetServerInformation,
    }

    #[interface(name = "org.freedesktop.Notifications")]
    impl NotificationHandler {
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

            dbg!(notification);
        }

        #[dbus_interface(name = "GetServerInformation")]
        async fn get_server_information(
            &mut self,
        ) -> zbus::fdo::Result<(String, String, String, String)> {
            let name = String::from("Notification Daemon");
            let vendor = String::from(env!("CARGO_PKG_NAME"));
            let version = String::from(env!("CARGO_PKG_VERSION"));
            let specification_version = String::from("1.2");

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

            Ok(capabilities)
        }
    }

    pub async fn run() -> Result<(), Box<dyn Error>> {
        let (sender, mut receiver) = mpsc::channel(5);

        let handler = NotificationHandler { count: 0, sender };

        let _conn = connection::Builder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at("/org/freedesktop/Notifications", handler)?
            .build()
            .await?;

        tokio::spawn(async move {
            while let Some(method) = receiver.recv().await {
                match method {
                    Method::Notify { notification } => {
                        todo!()
                    }
                    Method::CloseNotification { notification_id } => {
                        todo!()
                    }
                    Method::GetCapabilities => todo!(),
                    Method::GetServerInformation => todo!(),
                }
            }
        });

        // Do other things or go to wait forever
        pending::<()>().await;
        Ok(())
    }
}

pub mod client {}
