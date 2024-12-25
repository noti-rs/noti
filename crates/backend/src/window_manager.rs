use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};

use log::debug;
use shared::cached_data::CachedData;
use wayland_client::{Connection, EventQueue, QueueHandle};

use crate::cache::CachedLayout;
use crate::dispatcher::Dispatcher;

use config::Config;
use dbus::{actions::Signal, notification::Notification};

use super::window::{ConfigurationState, Window};
use render::font::FontCollection;

pub(crate) struct WindowManager {
    connection: Connection,
    event_queue: Option<EventQueue<Window>>,
    qhandle: Option<QueueHandle<Window>>,
    window: Option<Window>,

    font_collection: Rc<RefCell<FontCollection>>,
    cached_layouts: CachedData<PathBuf, CachedLayout>,

    signals: Vec<Signal>,

    notification_queue: VecDeque<Notification>,
    close_notifications: Vec<u32>,
}

impl Dispatcher for WindowManager {
    type State = Window;

    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)> {
        Some((self.event_queue.as_mut()?, self.window.as_mut()?))
    }
}

impl WindowManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;
        let font_collection =
            Rc::new(FontCollection::load_by_font_name(&config.general().font.name)?.into());
        let cached_layouts = config
            .displays()
            .filter_map(|display| match &display.layout {
                config::display::Layout::Default => None,
                config::display::Layout::FromPath { path_buf } => Some(path_buf),
            })
            .collect();

        let wm = Self {
            connection,
            event_queue: None,
            qhandle: None,
            window: None,

            font_collection,
            cached_layouts,

            signals: vec![],
            notification_queue: VecDeque::new(),
            close_notifications: vec![],
        };

        debug!("Window Manager: Created");

        Ok(wm)
    }

    pub(crate) fn update_cache(&mut self) -> bool {
        self.cached_layouts.update()
    }

    pub(crate) fn update_by_config(&mut self, config: &Config) -> anyhow::Result<()> {
        self.cached_layouts.extend_by_keys(
            config
                .displays()
                .filter_map(|display| match &display.layout {
                    config::display::Layout::Default => None,
                    config::display::Layout::FromPath { path_buf } => Some(path_buf.to_owned()),
                })
                .collect(),
        );

        self.font_collection
            .borrow_mut()
            .update_by_font_name(&config.general().font.name)?;

        if let Some(window) = self.window.as_mut() {
            let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

            window.reconfigure(config);
            window.redraw(qhandle, config, &self.cached_layouts);
            window.frame(qhandle);
            window.commit();
        }

        debug!("Window Manager: Updated the windows by updated config");

        self.roundtrip_event_queue()
    }

    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        self.notification_queue.push_back(*notification);
    }

    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.close_notifications.push(notification_id);
    }

    pub(crate) fn show_window(&mut self, config: &Config) -> anyhow::Result<()> {
        let mut notifications_limit = config.general().limit as usize;
        if notifications_limit == 0 {
            notifications_limit = usize::MAX;
        }

        if self
            .window
            .as_ref()
            .is_none_or(|window| window.total_banners() < notifications_limit)
            && !self.notification_queue.is_empty()
        {
            self.init_window(config)?;
            self.process_notification_queue(config)?;
        }

        Ok(())
    }

    fn process_notification_queue(&mut self, config: &Config) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            let mut notifications_limit = config.general().limit as usize;

            if notifications_limit == 0 {
                notifications_limit = usize::MAX
            }

            window.replace_by_indices(&mut self.notification_queue, config, &self.cached_layouts);

            let available_slots = notifications_limit.saturating_sub(window.total_banners());
            let notifications_to_display: Vec<_> = self
                .notification_queue
                .drain(..available_slots.min(self.notification_queue.len()))
                .collect();

            window.update_banners(notifications_to_display, config, &self.cached_layouts);

            self.update_window(config)?;
            self.roundtrip_event_queue()?;
        }

        Ok(())
    }

    pub(crate) fn handle_close_notifications(&mut self, config: &Config) -> anyhow::Result<()> {
        if self.window.as_ref().is_some() && !self.close_notifications.is_empty() {
            let window = self.window.as_mut().unwrap();

            let notifications = window.remove_banners_by_id(&self.close_notifications);
            self.close_notifications.clear();

            if notifications.is_empty() {
                return Ok(());
            }

            notifications
                .into_iter()
                .map(|notification| notification.id)
                .for_each(|id| {
                    self.signals.push(Signal::NotificationClosed {
                        notification_id: id,
                        reason: dbus::actions::ClosingReason::CallCloseNotification,
                    })
                });

            self.process_notification_queue(config)?;
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
                self.signals.push(Signal::NotificationClosed {
                    notification_id: notification.id,
                    reason: dbus::actions::ClosingReason::Expired,
                })
            });

            self.process_notification_queue(config)?;
        }

        Ok(())
    }

    pub(crate) fn pop_signal(&mut self) -> Option<Signal> {
        self.signals.pop()
    }

    pub(crate) fn handle_actions(&mut self, config: &Config) -> anyhow::Result<()> {
        //TODO: change it to actions which defines in config file

        if let Some(window) = self.window.as_mut() {
            window.handle_hover(config);

            let signals = window.handle_click(config);
            if signals.is_empty() {
                return Ok(());
            }

            self.signals.extend(signals);
            self.process_notification_queue(config)?;
        }

        Ok(())
    }

    pub(crate) fn reset_timeouts(&mut self) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            window.reset_timeouts();
        }

        Ok(())
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

            debug!("Window Manager: Updated the windows");
        }

        Ok(())
    }

    fn roundtrip_event_queue(&mut self) -> anyhow::Result<()> {
        if let Some(event_queue) = self.event_queue.as_mut() {
            event_queue.roundtrip(unsafe { self.window.as_mut().unwrap_unchecked() })?;

            debug!("Window Manager: Roundtrip events for the windows");
        }

        Ok(())
    }

    fn init_window(&mut self, config: &Config) -> anyhow::Result<bool> {
        if self.window.is_none() {
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

            debug!("Window Manager: Created a window");

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

        debug!("Window Manager: Closed window");

        Ok(())
    }
}
