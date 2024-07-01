use std::sync::{Arc, Mutex};

use wayland_client::Connection;

use crate::data::{
    aliases::Result,
    internal_messages::{InternalChannel, ServerMessage},
};

use self::layer::NotificationStack;

mod font;
mod layer;

pub(crate) struct Renderer {
    connection: Connection,
    notification_stack: NotificationStack,
    channel: Arc<Mutex<InternalChannel>>,
}

impl Renderer {
    pub(crate) fn init() -> Result<Self> {
        let connection = Connection::connect_to_env()?;

        Ok(Self {
            connection,
            notification_stack: NotificationStack::init()?,
            channel: Arc::new(Mutex::new(InternalChannel::new())),
        })
    }

    pub(crate) fn clone_channel(&self) -> Arc<Mutex<InternalChannel>> {
        self.channel.clone()
    }

    pub(crate) fn run(&mut self) {
        loop {
            if let Ok(channel) = self.channel.try_lock() {
                if let Ok(message) = channel.try_recv_from_server() {
                    match message {
                        ServerMessage::ShowNotification(notification) => {
                            dbg!(&notification);
                            self.notification_stack
                                .create_notification_rect(notification);
                        }
                        ServerMessage::CloseNotification { id } => todo!(),
                    }
                }
            }

            self.notification_stack.dispatch();
        }
    }
}
