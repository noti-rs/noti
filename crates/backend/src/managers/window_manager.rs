use crate::{dispatcher::Dispatcher, error::Error};
use cache::CachedLayout;
use config::Config;
use dbus::{actions::Signal, notification::Notification};
use log::debug;
use render::PangoContext;
use shared::cached_data::CachedData;
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};
use wayland_client::{
    protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_shm::WlShm},
    Connection,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use window::Window;

mod banner;
mod cache;
mod window;

pub(crate) struct WindowManager {
    window: Option<Window>,

    pango_context: Option<Rc<RefCell<PangoContext>>>,
    cached_layouts: CachedData<PathBuf, CachedLayout>,

    signals: Vec<Signal>,

    notification_queue: VecDeque<Notification>,
    close_notifications: Vec<u32>,
}

impl WindowManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let cached_layouts = config
            .displays()
            .filter_map(|display| match &display.layout {
                config::display::Layout::Default => None,
                config::display::Layout::FromPath { path_buf } => Some(path_buf),
            })
            .collect();

        let wm = Self {
            window: None,

            pango_context: None,
            cached_layouts,

            signals: vec![],
            notification_queue: VecDeque::new(),
            close_notifications: vec![],
        };

        debug!("Window Manager: Created");

        Ok(wm)
    }

    pub(crate) fn dispatch(&mut self) -> anyhow::Result<bool> {
        if let Some(window) = self.window.as_mut() {
            window.dispatch()?;
        }

        Ok(false)
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

        if let Some(pango_context) = self.pango_context.as_ref() {
            pango_context
                .borrow_mut()
                .update_font_family(&config.general().font.name);
        }

        let mut unrendered_notifcations = Ok(());
        if let Some(window) = self.window.as_mut() {
            window.reconfigure(config);
            unrendered_notifcations = window.redraw(config, &self.cached_layouts);
            window.frame();
            window.commit();
        }

        debug!("Window Manager: Updated the windows by updated config");

        self.sync()?;
        Ok(unrendered_notifcations?)
    }

    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        self.notification_queue.push_back(*notification);
    }

    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.close_notifications.push(notification_id);
    }

    pub(crate) fn show_window<P>(
        &mut self,
        wayland_connection: &Connection,
        protoctols: &P,
        config: &Config,
    ) -> Result<(), Error>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>,
    {
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
            self.init_window(wayland_connection, protoctols, config)?;
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
            self.sync()?;
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

            window.draw(config);
            window.frame();
            window.commit();

            debug!("Window Manager: Updated the windows");
        }

        Ok(())
    }

    fn sync(&mut self) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            window.sync()?;
            debug!("Window Manager: Roundtrip events for the windows");
        }

        Ok(())
    }

    fn init_window<P>(
        &mut self,
        wayland_connection: &Connection,
        protocols: &P,
        config: &Config,
    ) -> anyhow::Result<bool>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>,
    {
        if self.window.is_none() {
            let pango_context = Rc::new(RefCell::new(PangoContext::from_font_family(
                &config.general().font.name,
            )));
            self.pango_context = Some(pango_context.clone());
            self.window = Some(Window::init(
                wayland_connection,
                protocols,
                pango_context,
                config,
            )?);

            debug!("Window Manager: Created a window");

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn deinit_window(&mut self) -> anyhow::Result<()> {
        if self.window.as_mut().is_some() {
            self.pango_context = None;
            self.window = None;
            debug!("Window Manager: Closed window");
        }

        Ok(())
    }
}
