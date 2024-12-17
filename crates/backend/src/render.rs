use std::time::Duration;

use crate::dispatcher::Dispatcher;
use crate::idle_manager::IdleManager;

use super::internal_messages::{RendererInternalChannel, ServerMessage};
use config::Config;
use log::{debug, info};
use shared::file_watcher::FileState;

use super::window_manager::WindowManager;

pub(crate) struct Renderer {
    config: Config,
    window_manager: WindowManager,
    idle_manager: IdleManager,
    channel: RendererInternalChannel,
}

impl Renderer {
    pub(crate) fn init(
        config: Config,
        renderer_internal_channel: RendererInternalChannel,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            window_manager: WindowManager::init(&config)?,
            idle_manager: IdleManager::init(&config)?,
            channel: renderer_internal_channel,
            config,
        })
    }

    pub(crate) fn run(&mut self) -> anyhow::Result<()> {
        let mut notifications_to_create = vec![];
        let mut notifications_to_close = vec![];
        let mut partially_default_config = false;

        debug!("Renderer: Running");
        loop {
            while let Ok(message) = self.channel.try_recv_from_server() {
                match message {
                    ServerMessage::ShowNotification(notification) => {
                        debug!("Renderer: Received notification to render: {notification:?}");
                        notifications_to_create.push(*notification);
                    }
                    ServerMessage::CloseNotification { id } => {
                        debug!("Renderer: Received notification id {id} to close");
                        notifications_to_close.push(id);
                    }
                }
            }

            if !notifications_to_create.is_empty() {
                self.window_manager
                    .create_notifications(notifications_to_create, &self.config)?;
                notifications_to_create = vec![];
                debug!("Renderer: Created notifications");
            }

            if !notifications_to_close.is_empty() {
                self.window_manager
                    .close_notifications(&notifications_to_close, &self.config)?;
                notifications_to_close.clear();
                debug!("Renderer: Closed notifications");
            }
            self.window_manager.remove_expired(&self.config)?;

            while let Some(message) = self.window_manager.pop_event() {
                self.channel.send_to_server(message)?;
            }

            self.window_manager.handle_actions(&self.config)?;
            self.window_manager.dispatch()?;

            self.idle_manager.dispatch()?;

            {
                match self.config.check_updates() {
                    FileState::Updated => {
                        partially_default_config = false;
                        self.update_config()?;
                        info!("Renderer: Detected changes of config files and updated")
                    }
                    FileState::NotFound if !partially_default_config => {
                        partially_default_config = true;
                        self.update_config()?;
                        info!("The main or imported configuration file is not found, reverting this part to default values.");
                    }
                    FileState::NotFound | FileState::NothingChanged => (),
                };

                if self.window_manager.update_cache() {
                    self.window_manager.update_by_config(&self.config)?;
                }
            }

            std::thread::sleep(Duration::from_millis(50));
            std::hint::spin_loop();
        }
    }

    fn update_config(&mut self) -> anyhow::Result<()> {
        self.config.update();
        self.window_manager.update_by_config(&self.config)?;
        Ok(())
    }
}
