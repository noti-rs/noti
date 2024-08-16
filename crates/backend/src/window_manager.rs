use std::sync::Arc;

use wayland_client::{Connection, EventQueue, QueueHandle};

use super::internal_messages::RendererMessage;
use config::Config;
use dbus::notification::Notification;

use super::{
    render::FontCollection,
    window::{ConfigurationState, Window},
};

pub(crate) struct WindowManager {
    connection: Connection,
    event_queue: Option<EventQueue<Window>>,
    qhandle: Option<QueueHandle<Window>>,
    window: Option<Window>,

    font_collection: Arc<FontCollection>,

    events: Vec<RendererMessage>,
}

impl WindowManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;
        let font_collection = Arc::new(FontCollection::load_by_font_name(
            &config.general().font.name,
        )?);

        Ok(Self {
            connection,
            event_queue: None,
            qhandle: None,
            window: None,

            font_collection,

            events: vec![],
        })
    }

    pub(crate) fn update_by_config(&mut self, config: &Config) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

            window.reconfigure(config);
            window.redraw(qhandle, config);
            window.frame(qhandle);
            window.commit();
        }

        self.roundtrip_event_queue()
    }

    pub(crate) fn create_notifications(
        &mut self,
        notifications: Vec<Notification>,
        config: &Config,
    ) -> anyhow::Result<()> {
        self.init_window(config)?;

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        window.update_banners(notifications, config);

        self.update_window(config)?;
        self.roundtrip_event_queue()
    }

    pub(crate) fn close_notifications(
        &mut self,
        indices: &[u32],
        config: &Config,
    ) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_banners_by_id(indices);

            if notifications.is_empty() {
                return Ok(());
            }

            notifications
                .into_iter()
                .map(|notification| notification.id)
                .for_each(|id| {
                    self.events.push(RendererMessage::ClosedNotification {
                        id,
                        reason: dbus::actions::ClosingReason::CallCloseNotification,
                    })
                });

            self.update_window(config)?;
            self.roundtrip_event_queue()?;
        }

        Ok(())
    }

    pub(crate) fn remove_expired(&mut self, config: &Config) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_expired_banners(config);

            if notifications.is_empty() {
                return Ok(());
            }

            notifications.into_iter().for_each(|notification| {
                self.events.push(RendererMessage::ClosedNotification {
                    id: notification.id,
                    reason: dbus::actions::ClosingReason::Expired,
                })
            });

            self.update_window(config)?;
            self.roundtrip_event_queue()?;
        }

        Ok(())
    }

    pub(crate) fn pop_event(&mut self) -> Option<RendererMessage> {
        self.events.pop()
    }

    pub(crate) fn handle_actions(&mut self, config: &Config) -> anyhow::Result<()> {
        //TODO: change it to actions which defines in config file

        if let Some(window) = self.window.as_mut() {
            window.handle_hover(config);

            let messages = window.handle_click(config);
            if messages.is_empty() {
                return Ok(());
            }

            self.events.extend(messages);

            self.update_window(config)?;
            self.roundtrip_event_queue()?;
        }

        Ok(())
    }

    pub(crate) fn dispatch(&mut self) -> anyhow::Result<bool> {
        if self.event_queue.is_none() {
            return Ok(false);
        }

        let event_queue = unsafe { self.event_queue.as_mut().unwrap_unchecked() };
        let window = unsafe { self.window.as_mut().unwrap_unchecked() };

        let dispatched_count = event_queue.dispatch_pending(window)?;

        if dispatched_count > 0 {
            return Ok(true);
        }

        event_queue.flush()?;
        let Some(guard) = event_queue.prepare_read() else {
            return Ok(false);
        };
        let Ok(count) = guard.read() else {
            return Ok(false);
        };

        Ok(if count > 0 {
            event_queue.dispatch_pending(window)?;
            true
        } else {
            false
        })
    }

    fn update_window(&mut self, config: &Config) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            if window.is_empty() {
                return self.deinit_window();
            }

            let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

            window.draw(qhandle, config);
            window.frame(qhandle);
            window.commit();
        }

        Ok(())
    }

    fn roundtrip_event_queue(&mut self) -> anyhow::Result<()> {
        if let Some(event_queue) = self.event_queue.as_mut() {
            event_queue.roundtrip(unsafe { self.window.as_mut().unwrap_unchecked() })?;
        }

        Ok(())
    }

    fn init_window(&mut self, config: &Config) -> anyhow::Result<bool> {
        if let None = self.window {
            let mut event_queue = self.connection.new_event_queue();
            let qhandle = event_queue.handle();
            let display = self.connection.display();
            display.get_registry(&qhandle, ());

            let mut window = Window::init(self.font_collection.clone(), config);

            while let ConfigurationState::NotConfiured = window.configuration_state() {
                event_queue.blocking_dispatch(&mut window)?;
            }

            window.configure(&qhandle, config);

            while let ConfigurationState::Ready = window.configuration_state() {
                event_queue.blocking_dispatch(&mut window)?;
            }

            self.event_queue = Some(event_queue);
            self.qhandle = Some(qhandle);
            self.window = Some(window);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn deinit_window(&mut self) -> anyhow::Result<()> {
        unsafe {
            let window = self.window.as_mut().unwrap_unchecked();
            window.deinit();
            self.event_queue
                .as_mut()
                .unwrap_unchecked()
                .roundtrip(window)?;
        }
        self.window = None;
        self.event_queue = None;
        self.qhandle = None;

        Ok(())
    }
}
