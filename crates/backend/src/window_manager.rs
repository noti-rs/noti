use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};

use log::debug;
use render::PangoContext;
use shared::cached_data::CachedData;
use wayland_client::{Connection, EventQueue, QueueHandle};

use crate::dispatcher::Dispatcher;
use crate::{cache::CachedLayout, error::Error};

use config::Config;
use dbus::{actions::Signal, notification::Notification};

use super::window::{ConfigurationState, Window};

pub(crate) struct WindowManager {
    connection: Connection,
    event_queue: EventQueue<Window>,
    qhandle: QueueHandle<Window>,
    window: Option<Window>,

    pango_context: Rc<RefCell<PangoContext>>,
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
        Some((&mut self.event_queue, self.window.as_mut()?))
    }
}

impl WindowManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;
        let pango_context =
            Rc::new(PangoContext::from_font_family(&config.general().font.name).into());
        let cached_layouts = config
            .displays()
            .filter_map(|display| match &display.layout {
                config::display::Layout::Default => None,
                config::display::Layout::FromPath { path_buf } => Some(path_buf),
            })
            .collect();

        let event_queue = connection.new_event_queue();
        let qhandle = event_queue.handle();
        let wm = Self {
            connection,
            event_queue,
            qhandle,
            window: None,

            pango_context,
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

    pub(crate) fn update_by_config(&mut self, config: &Config) -> Result<(), Error> {
        self.cached_layouts.extend_by_keys(
            config
                .displays()
                .filter_map(|display| match &display.layout {
                    config::display::Layout::Default => None,
                    config::display::Layout::FromPath { path_buf } => Some(path_buf.to_owned()),
                })
                .collect(),
        );

        self.pango_context
            .borrow_mut()
            .update_font_family(&config.general().font.name);

        let mut unrendered_notifcations = Ok(());
        if let Some(window) = self.window.as_mut() {
            window.reconfigure(config);
            unrendered_notifcations = window.redraw(&self.qhandle, config, &self.cached_layouts);
            window.frame(&self.qhandle);
            window.commit();
        }

        debug!("Window Manager: Updated the windows by updated config");

        self.roundtrip_event_queue()?;
        Ok(unrendered_notifcations?)
    }

    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        self.notification_queue.push_back(*notification);
    }

    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.close_notifications.push(notification_id);
    }

    pub(crate) fn show_window(&mut self, config: &Config) -> Result<(), Error> {
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

    fn process_notification_queue(&mut self, config: &Config) -> Result<(), Error> {
        let mut unrendered_notifications = Ok(());
        if let Some(window) = self.window.as_mut() {
            let mut notifications_limit = config.general().limit as usize;

            if notifications_limit == 0 {
                notifications_limit = usize::MAX
            }

            window.replace_by_indices(
                &mut self.notification_queue,
                config,
                &self.cached_layouts,
            )?;

            let available_slots = notifications_limit.saturating_sub(window.total_banners());
            let notifications_to_display: Vec<_> = self
                .notification_queue
                .drain(..available_slots.min(self.notification_queue.len()))
                .collect();

            unrendered_notifications =
                window.update_banners(notifications_to_display, config, &self.cached_layouts);

            self.update_window(config)?;
            self.roundtrip_event_queue()?;
        }

        Ok(unrendered_notifications?)
    }

    pub(crate) fn handle_close_notifications(&mut self, config: &Config) -> Result<(), Error> {
        if self.window.as_ref().is_some() && !self.close_notifications.is_empty() {
            let window = self.window.as_mut().unwrap();

            let notifications = window.remove_banners_by_id(&self.close_notifications);
            self.close_notifications.clear();

            if notifications.is_empty() {
                return Ok(());
            }

            notifications.into_iter().for_each(|notification| {
                let notification_id = notification.id;
                self.signals.push(Signal::NotificationClosed {
                    notification_id,
                    reason: dbus::actions::ClosingReason::CallCloseNotification,
                })
            });

            self.process_notification_queue(config)?;
        }

        Ok(())
    }

    pub(crate) fn remove_expired(&mut self, config: &Config) -> Result<(), Error> {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_expired_banners(config);

            if notifications.is_empty() {
                return Ok(());
            }

            notifications.into_iter().for_each(|notification| {
                let notification_id = notification.id;
                self.signals.push(Signal::NotificationClosed {
                    notification_id,
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

    pub(crate) fn handle_actions(&mut self, config: &Config) -> Result<(), Error> {
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

            window.draw(&self.qhandle, config);
            window.frame(&self.qhandle);
            window.commit();

            debug!("Window Manager: Updated the windows");
        }

        Ok(())
    }

    fn roundtrip_event_queue(&mut self) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            self.event_queue.roundtrip(window)?;

            debug!("Window Manager: Roundtrip events for the windows");
        }

        Ok(())
    }

    fn init_window(&mut self, config: &Config) -> anyhow::Result<bool> {
        if self.window.is_none() {
            let display = self.connection.display();
            display.get_registry(&self.qhandle, ());

            let mut window = Window::init(self.pango_context.clone(), config);

            while let ConfigurationState::NotConfiured = window.configuration_state() {
                self.event_queue.blocking_dispatch(&mut window)?;
            }

            window.configure(&self.qhandle, config);

            while let ConfigurationState::Ready = window.configuration_state() {
                self.event_queue.blocking_dispatch(&mut window)?;
            }

            self.window = Some(window);
            debug!("Window Manager: Created a window");

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn deinit_window(&mut self) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            window.deinit();
            self.event_queue.roundtrip(window)?;

            self.window = None;
            debug!("Window Manager: Closed window");
        }

        Ok(())
    }
}
