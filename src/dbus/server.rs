use crate::data::{
    dbus::{Action, ClosingReason, Signal},
    image::ImageData,
    notification::{Category, Notification, Timeout, Urgency},
};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::UnboundedSender;
use zbus::{
    connection, fdo::Result, interface, object_server::SignalContext, zvariant::Value, Connection,
};

static UNIQUE_ID: AtomicU32 = AtomicU32::new(1);

pub struct Server {
    connection: Connection,
}

impl Server {
    const NOTIFICATIONS_PATH: &'static str = "/org/freedesktop/Notifications";
    const NOTIFICATIONS_NAME: &'static str = "org.freedesktop.Notifications";

    pub async fn init(sender: UnboundedSender<Action>) -> Result<Self> {
        let handler = Handler { sender };

        let connection = connection::Builder::session()?
            .name(Self::NOTIFICATIONS_NAME)?
            .serve_at(Self::NOTIFICATIONS_PATH, handler)?
            .build()
            .await?;

        Ok(Self { connection })
    }

    pub async fn emit_signal(&self, signal: Signal) -> zbus::Result<()> {
        let ctxt = SignalContext::new(&self.connection, Self::NOTIFICATIONS_PATH)?;
        match signal {
            Signal::NotificationClosed {
                notification_id,
                reason,
            } => {
                let id = match notification_id {
                    0 => UNIQUE_ID.load(Ordering::Relaxed),
                    _ => notification_id,
                };

                Handler::notification_closed(&ctxt, id, u32::from(reason)).await
            }
            Signal::ActionInvoked {
                notification_id,
                action_key,
            } => Handler::action_invoked(&ctxt, notification_id, &action_key).await,
        }
    }
}

struct Handler {
    sender: UnboundedSender<Action>,
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

        let category = if let Some(Value::Str(val)) = hints.get("category") {
            Category::from(val.to_owned())
        } else {
            Default::default()
        };

        let image_data = ["image-data", "image_data", "icon-data", "icon_data"]
            .iter()
            .find_map(|&name| hints.get(name))
            .and_then(ImageData::from_hint);

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
            summary,
            body,
            urgency,
            category,
            expire_timeout,
            created_at,
            image_data,
            image_path,
            is_read: false,
        };

        self.sender.send(Action::Show(notification)).unwrap();
        Ok(id)
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> Result<()> {
        Self::notification_closed(&ctxt, id, ClosingReason::CallCloseNotification.into()).await?;
        self.sender.send(Action::Close(Some(id))).unwrap();

        Ok(())
    }

    // NOTE: temporary
    async fn close_last_notification(
        &self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> Result<()> {
        //WARNING: temporary id value
        Self::notification_closed(&ctxt, 0, ClosingReason::CallCloseNotification.into()).await?;
        self.sender.send(Action::Close(None)).unwrap();

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

    #[zbus(signal)]
    async fn action_invoked(
        ctxt: &SignalContext<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(
        ctxt: &SignalContext<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}
