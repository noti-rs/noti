use std::sync::Arc;

use smithay_client_toolkit::reexports::client::Connection;

use crate::data::{
    aliases::Result,
    internal_messages::{InternalChannel, ServerMessage},
};
use crate::render::layer::NotificationLayer;

mod font;
mod layer;

struct Renderer {
    connection: Connection,
    layers: Vec<NotificationLayer>,
    channel: Arc<InternalChannel>,
}

impl Renderer {
    fn init() -> Result<Self> {
        let connection = Connection::connect_to_env()?;

        Ok(Self {
            connection,
            layers: vec![],
            channel: Arc::new(InternalChannel::new()),
        })
    }

    fn clone_channel(&self) -> Arc<InternalChannel> {
        self.channel.clone()
    }

    fn run(&mut self) {
        loop {
            if let Ok(message) = self.channel.try_recv_from_server() {
                match message {
                    ServerMessage::ShowNotification(notification) => todo!(),
                    ServerMessage::CloseNotification { id } => todo!(),
                }
            }
        }
    }
}
