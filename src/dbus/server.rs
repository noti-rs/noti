use crate::notification::{Action, ImageData, Notification, Signal, Timeout, Urgency};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::Sender;
use zbus::{
    connection,
    fdo::Result,
    interface,
    zvariant::{Array, Value},
    Connection,
};

pub const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
pub const NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";

static UNIQUE_ID: AtomicU32 = AtomicU32::new(1);

pub struct Server {
    connection: Connection,
}

impl Server {
    pub async fn init(sender: Sender<Action>) -> Result<Self> {
        let handler = Handler { sender };

        let connection = connection::Builder::session()?
            .name(NOTIFICATIONS_NAME)?
            .serve_at(NOTIFICATIONS_PATH, handler)?
            .build()
            .await?;

        Ok(Self { connection })
    }

    pub async fn emit_signal(&self, signal: Signal) -> zbus::Result<()> {
        match signal {
            Signal::NotificationClosed {
                notification_id,
                reason,
            } => {
                let id = match notification_id {
                    0 => UNIQUE_ID.load(Ordering::Relaxed),
                    _ => notification_id,
                };

                self.connection
                    .emit_signal(
                        None::<()>,
                        NOTIFICATIONS_PATH,
                        NOTIFICATIONS_NAME,
                        "NotificationClosed",
                        &(id, reason),
                    )
                    .await
            }
            Signal::ActionInvoked { notification_id } => {
                self.connection
                    .emit_signal(
                        None::<()>,
                        NOTIFICATIONS_PATH,
                        NOTIFICATIONS_NAME,
                        "ActionInvoked",
                        &(notification_id),
                    )
                    .await
            }
        }
    }
}

struct Handler {
    sender: Sender<Action>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Handler {
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
            UNIQUE_ID.fetch_add(1, Ordering::Relaxed)
        } else {
            replaces_id
        };

        let expire_timeout = match expire_timeout {
            t if t < -1 => todo!(),
            -1 => Timeout::Never,
            0 => Timeout::Configurable,
            t => Timeout::Millis(t as u32),
        };

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let urgency = if let Some(Value::U32(val)) = hints.get("urgency") {
            Urgency::from(val.to_owned())
        } else {
            Default::default()
        };

        let image_data = ["image-data", "image_data", "icon-data", "icon_data"]
            .iter()
            .find_map(|&name| hints.get(name))
            .and_then(|hint| {
                zbus::zvariant::Structure::try_from(hint)
                    .ok()
                    .and_then(|image_structure| {
                        let fields = image_structure.fields();
                        if fields.len() < 7 {
                            return None;
                        }

                        let image_raw = match Array::try_from(&fields[6]) {
                            Ok(array) => array,
                            Err(_) => return None,
                        };

                        let width = i32::try_from(&fields[0]).ok()?;
                        let height = i32::try_from(&fields[1]).ok()?;
                        let rowstride = i32::try_from(&fields[2]).ok()?;
                        let has_alpha = bool::try_from(&fields[3]).ok()?;
                        let bits_per_sample = i32::try_from(&fields[4]).ok()?;
                        let channels = i32::try_from(&fields[5]).ok()?;

                        let data = image_raw
                            .iter()
                            .map(|value| u8::try_from(value).expect("expected u8"))
                            .collect::<Vec<_>>();

                        Some(ImageData {
                            width,
                            height,
                            rowstride,
                            has_alpha,
                            bits_per_sample,
                            channels,
                            data,
                        })
                    })
            });

        let image_path = match hints.get("image-path") {
            Some(path) => Some(zbus::zvariant::Str::try_from(path).unwrap().to_string()),
            None => None,
        };

        // TODO: parse other hints
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
            image_data,
            image_path,
            is_read: false,
        };

        self.sender.send(Action::Show(notification)).await.unwrap();
        Ok(id)
    }

    async fn close_notification(&self, id: u32) -> Result<()> {
        self.sender.send(Action::Close(Some(id))).await.unwrap();

        Ok(())
    }

    // NOTE: temporary
    async fn close_last_notification(&self) -> Result<()> {
        self.sender.send(Action::Close(None)).await.unwrap();

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
