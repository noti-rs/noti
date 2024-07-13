use std::time::Duration;

use crate::data::{
    aliases::Result,
    internal_messages::{
        InternalChannel, RendererInternalChannel, ServerInternalChannel, ServerMessage,
    },
};

use self::layer::NotificationStack;

mod border;
mod color;
mod font;
mod image;
mod layer;
mod text;

pub(crate) struct Renderer {
    notification_stack: NotificationStack,
    channel: RendererInternalChannel,
}

impl Renderer {
    pub(crate) fn init() -> Result<(ServerInternalChannel, Self)> {
        let (server_internal_channel, renderer_internal_channel) = InternalChannel::new().split();
        Ok((
            server_internal_channel,
            Self {
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
                    ServerMessage::CloseNotification { id } => {
                        self.notification_stack.close_notification(id)
                    }
                }
            }

            while let Some(message) = self.notification_stack.pop_event() {
                self.channel.send_to_server(message).unwrap();
            }

            self.notification_stack.handle_actions();
            self.notification_stack.dispatch();

            std::thread::sleep(Duration::from_millis(50));
            std::hint::spin_loop();
        }
    }
}
