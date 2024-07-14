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
        let mut created = false;
        let mut notifications_to_create = vec![];
        let mut notifications_to_close = vec![];

        loop {
            while let Ok(message) = self.channel.try_recv_from_server() {
                match message {
                    ServerMessage::ShowNotification(notification) => {
                        dbg!(&notification);
                        notifications_to_create.push(notification);
                    }
                    ServerMessage::CloseNotification { id } => {
                        notifications_to_close.push(id);
                    }
                }
            }

            if !notifications_to_create.is_empty() {
                self.notification_stack
                    .create_notification_rects(notifications_to_create);
                notifications_to_create = vec![];
                created = true;
            }

            if !created && !notifications_to_close.is_empty() {
                self.notification_stack
                    .close_notifications(&notifications_to_close);
                notifications_to_close.clear();
            }

            while let Some(message) = self.notification_stack.pop_event() {
                self.channel.send_to_server(message).unwrap();
            }

            self.notification_stack.handle_actions();
            self.notification_stack.dispatch();

            created = false;
            std::thread::sleep(Duration::from_millis(50));
            std::hint::spin_loop();
        }
    }
}
