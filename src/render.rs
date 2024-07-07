use wayland_client::Connection;

use crate::data::{
    aliases::Result,
    internal_messages::{
        InternalChannel, RendererInternalChannel, ServerInternalChannel, ServerMessage,
    },
};

use self::layer::NotificationStack;

mod font;
mod color;
mod image;
mod layer;

pub(crate) struct Renderer {
    connection: Connection,
    notification_stack: NotificationStack,
    channel: RendererInternalChannel,
}

impl Renderer {
    pub(crate) fn init() -> Result<(ServerInternalChannel, Self)> {
        let connection = Connection::connect_to_env()?;

        let (server_internal_channel, renderer_internal_channel) = InternalChannel::new().split();
        Ok((
            server_internal_channel,
            Self {
                connection,
                notification_stack: NotificationStack::init()?,
                channel: renderer_internal_channel,
            },
        ))
    }

    pub(crate) fn run(&mut self) {
        loop {
            if let Ok(message) = self.channel.try_recv_from_server() {
                match message {
                    ServerMessage::ShowNotification(notification) => {
                        dbg!(&notification);
                        self.notification_stack
                            .create_notification_rect(notification);
                    }
                    ServerMessage::CloseNotification { id } => todo!(),
                }
            }

            self.notification_stack.dispatch();
        }
    }
}
