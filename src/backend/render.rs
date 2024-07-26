use std::time::Duration;

use crate::data::{
    aliases::Result,
    internal_messages::{
        InternalChannel, RendererInternalChannel, ServerInternalChannel, ServerMessage,
    },
};

use self::window::WindowManager;

mod types;
mod banner;
mod border;
mod color;
mod font;
mod image;
mod text;
mod window;

pub(crate) struct Renderer {
    window_manager: WindowManager,
    channel: RendererInternalChannel,
}

impl Renderer {
    pub(crate) fn init() -> Result<(ServerInternalChannel, Self)> {
        let (server_internal_channel, renderer_internal_channel) = InternalChannel::new().split();
        Ok((
            server_internal_channel,
            Self {
                window_manager: WindowManager::init()?,
                channel: renderer_internal_channel,
            },
        ))
    }

    pub(crate) fn run(&mut self) {
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
                self.window_manager
                    .create_notifications(notifications_to_create);
                notifications_to_create = vec![];
            }

            if !notifications_to_close.is_empty() {
                self.window_manager
                    .close_notifications(&notifications_to_close);
                notifications_to_close.clear();
            }
            self.window_manager.remove_expired();

            while let Some(message) = self.window_manager.pop_event() {
                self.channel.send_to_server(message).unwrap();
            }

            self.window_manager.handle_actions();
            self.window_manager.dispatch();

            std::thread::sleep(Duration::from_millis(50));
            std::hint::spin_loop();
        }
    }
}
