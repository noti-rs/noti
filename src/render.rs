use std::sync::Arc;

use smithay_client_toolkit::reexports::client::Connection;

use crate::data::{aliases::Result, internal_messages::InternalChannel};
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
}
