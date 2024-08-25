use std::time::Duration;

use super::internal_messages::{
    InternalChannel, RendererInternalChannel, ServerInternalChannel, ServerMessage,
};
use config::{watcher::ConfigState, Config};

pub(super) use self::{banner::BannerRect, font::FontCollection, types::RectSize};
use super::window_manager::WindowManager;

mod banner;
mod border;
mod color;
mod font;
mod image;
mod text;
mod types;
mod widget;

pub(crate) struct Renderer {
    config: Config,
    window_manager: WindowManager,
    channel: RendererInternalChannel,
}

impl Renderer {
    pub(crate) fn init(config: Config) -> anyhow::Result<(ServerInternalChannel, Self)> {
        let (server_internal_channel, renderer_internal_channel) = InternalChannel::new().split();

        Ok((
            server_internal_channel,
            Self {
                window_manager: WindowManager::init(&config)?,
                channel: renderer_internal_channel,
                config,
            },
        ))
    }

    pub(crate) fn run(&mut self) -> anyhow::Result<()> {
        let mut notifications_to_create = vec![];
        let mut notifications_to_close = vec![];

        loop {
            while let Ok(message) = self.channel.try_recv_from_server() {
                match message {
                    ServerMessage::ShowNotification(notification) => {
                        dbg!(&notification);
                        notifications_to_create.push(*notification);
                    }
                    ServerMessage::CloseNotification { id } => {
                        notifications_to_close.push(id);
                    }
                }
            }

            if !notifications_to_create.is_empty() {
                self.window_manager
                    .create_notifications(notifications_to_create, &self.config)?;
                notifications_to_create = vec![];
            }

            if !notifications_to_close.is_empty() {
                self.window_manager
                    .close_notifications(&notifications_to_close, &self.config)?;
                notifications_to_close.clear();
            }
            self.window_manager.remove_expired(&self.config)?;

            while let Some(message) = self.window_manager.pop_event() {
                self.channel.send_to_server(message)?;
            }

            self.window_manager.handle_actions(&self.config)?;
            self.window_manager.dispatch()?;

            {
                match self.config.check_updates() {
                    ConfigState::NotFound | ConfigState::Updated => {
                        self.config.update();
                        self.window_manager.update_by_config(&self.config)?;
                    }
                    ConfigState::NothingChanged => (),
                };
            }

            std::thread::sleep(Duration::from_millis(50));
            std::hint::spin_loop();
        }
    }
}
