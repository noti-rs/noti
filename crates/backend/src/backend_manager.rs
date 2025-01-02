use crate::{dispatcher::Dispatcher, error::Error};
use crate::idle_manager::IdleManager;

use config::Config;
use dbus::{actions::Signal, notification::Notification};
use log::debug;

use super::window_manager::WindowManager;

pub(crate) struct BackendManager {
    window_manager: WindowManager,
    idle_manager: IdleManager,
}

impl BackendManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            window_manager: WindowManager::init(config)?,
            idle_manager: IdleManager::init(config)?,
        })
    }

    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        let id = notification.id;
        self.window_manager.create_notification(notification);
        debug!("Backend Manager: Received notification with id {id} to append queue");
    }

    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.window_manager.close_notification(notification_id);
        debug!("Backend Manager: Received notification id {notification_id} to close");
    }

    pub(crate) fn poll(&mut self, config: &Config) -> Result<(), Error> {
        let Self {
            idle_manager,
            window_manager,
        } = self;

        if !idle_manager.is_idled() {
            if idle_manager.was_idled() {
                idle_manager.reset_idle_state();

                window_manager.reset_timeouts()?;
            }

            window_manager.show_window(config)?;

            window_manager.handle_close_notifications(config)?;
            window_manager.remove_expired(config)?;

            window_manager.handle_actions(config)?;
        }

        window_manager.dispatch()?;
        idle_manager.dispatch()?;

        if window_manager.update_cache() {
            window_manager.update_by_config(config)?;
        }

        Ok(())
    }

    pub(crate) fn pop_signal(&mut self) -> Option<Signal> {
        self.window_manager.pop_signal()
    }

    pub(crate) fn update_config(&mut self, config: &Config) -> Result<(), Error> {
        let Self {
            window_manager,
            idle_manager,
        } = self;

        window_manager.update_by_config(config)?;
        idle_manager.update_by_config(config);
        window_manager.reset_timeouts()?;
        Ok(())
    }
}
