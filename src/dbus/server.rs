use crate::notification::{Action, Notification, Signal, Timeout, Urgency};
use core::panic;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{Sender, UnboundedSender};
use zbus::{
    connection, fdo::Result, interface, object_server::SignalContext, zvariant::Value, Connection,
};

const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";
pub struct Server {
    pub connection: Connection,
}

struct Handler {
    count: u32,
    sender: Sender<Action>,

    // temporary
    sig_sender: UnboundedSender<Signal>,
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
            self.count += 1;
            self.count
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

    async fn trigger_action_invoked_sig(&self) -> Result<()> {
        self.sig_sender
            .send(Signal::ActionInvoked { notification_id: 0 })
            .unwrap();
        Ok(())
    }

    async fn trigger_notification_closed_sig(&self) -> Result<()> {
        self.sig_sender
            .send(Signal::NotificationClosed {
                notification_id: 0,
                reason: 1,
            })
            .unwrap();
        Ok(())
    }

    #[zbus(signal)]
    async fn action_invoked(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
}

impl Server {
    pub async fn init(sender: Sender<Action>, sig_sender: UnboundedSender<Signal>) -> Result<Self> {
        let handler = Handler {
            count: 0,
            sender,
            sig_sender,
        };

        let connection = connection::Builder::session()?
            .name(NOTIFICATIONS_NAME)?
            .serve_at(NOTIFICATIONS_PATH, handler)?
            .build()
            .await?;

        Ok(Self { connection })
    }
}
