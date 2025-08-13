use crate::{dispatcher::Dispatcher, error::Error, EglState, NoSurface};
use cache::CachedLayout;
use config::Config;
use dbus::{actions::Signal, notification::Notification};
use log::debug;
use shared::{
    cached_data::CachedData,
    data::{Borrowed, Data, Owned},
};
use skia_safe::gpu::DirectContext;
use std::{collections::VecDeque, path::PathBuf};
use wayland_client::{
    protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_shm::WlShm},
    Connection,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use window::Window;

mod banner_stack;
mod cache;
mod window;

pub(crate) struct WindowManager {
    window: Option<Window>,

    font_collection: skia_safe::textlayout::FontCollection,
    cached_layouts: Data<CachedData<PathBuf, CachedLayout>, Owned>,

    signals: Vec<Signal>,

    notification_queue: VecDeque<Notification>,
    close_notifications: Vec<u32>,
}

impl WindowManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let cached_layouts = Data::new(
            config
                .displays()
                .filter_map(|display| match &display.layout {
                    config::display::Layout::Default => None,
                    config::display::Layout::FromPath { path_buf } => Some(path_buf),
                })
                .collect(),
        );

        let wm = Self {
            window: None,

            font_collection: {
                let mut fc = skia_safe::textlayout::FontCollection::new();
                fc.set_default_font_manager_and_family_names(
                    skia_safe::FontMgr::new(),
                    &config
                        .displays()
                        .flat_map(|display| [&display.summary.font.name, &display.body.font.name])
                        .collect::<Vec<_>>(),
                );
                fc
            },
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

    pub(crate) fn update_by_config(&mut self, config: Data<Config, Borrowed>) -> Result<(), Error> {
        self.cached_layouts.extend_by_keys(
            config
                .displays()
                .filter_map(|display| match &display.layout {
                    config::display::Layout::Default => None,
                    config::display::Layout::FromPath { path_buf } => Some(path_buf.to_owned()),
                })
                .collect(),
        );

        self.font_collection.clear_caches();
        self.font_collection
            .set_default_font_manager_and_family_names(
                skia_safe::FontMgr::new(),
                &config
                    .displays()
                    .flat_map(|display| [&display.summary.font.name, &display.body.font.name])
                    .collect::<Vec<_>>(),
            );

        if let Some(window) = self.window.as_mut() {
            window.reconfigure(config);
            window.frame();
            window.commit();
        }

        debug!("Window Manager: Updated the windows by updated config");

        self.sync()?;
        Ok(())
    }

    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        self.notification_queue.push_back(*notification);
    }

    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.close_notifications.push(notification_id);
    }

    pub(crate) fn show_window<P, Gpu>(
        &mut self,
        wayland_connection: &Connection,
        gpu: &mut Gpu,
        protoctols: &P,
        config: Data<Config, Borrowed>,
    ) -> Result<(), Error>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>,
        Gpu: AsRef<EglState> + AsRef<NoSurface> + AsRef<DirectContext>,
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
            self.init_window(wayland_connection, protoctols, gpu, config.clone())?;
            self.process_notification_queue(config, gpu)?;
        }

        Ok(())
    }

    fn process_notification_queue<Gpu>(
        &mut self,
        config: Data<Config, Borrowed>,
        gpu: &mut Gpu,
    ) -> Result<(), Error>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface>,
    {
        if let Some(window) = self.window.as_mut() {
            let mut notifications_limit = config.general().limit as usize;

            if notifications_limit == 0 {
                notifications_limit = usize::MAX
            }

            window.replace_by_indices(&mut self.notification_queue);

            let available_slots = notifications_limit.saturating_sub(window.total_banners());
            let notifications_to_display: Vec<_> = self
                .notification_queue
                .drain(..available_slots.min(self.notification_queue.len()))
                .collect();

            window.add_banners(notifications_to_display);

            self.update_window(gpu)?;
            self.sync()?;
        }

        Ok(())
    }

    pub(crate) fn handle_close_notifications<Gpu>(
        &mut self,
        config: Data<Config, Borrowed>,
        gpu: &mut Gpu,
    ) -> Result<(), Error>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface> + AsMut<DirectContext>,
    {
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

            self.process_notification_queue(config, gpu)?;
        }

        Ok(())
    }

    pub(crate) fn remove_expired<Gpu>(
        &mut self,
        config: Data<Config, Borrowed>,
        gpu: &mut Gpu,
    ) -> Result<(), Error>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface> + AsMut<DirectContext>,
    {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_expired_banners();

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

            self.process_notification_queue(config, gpu)?;
        }

        Ok(())
    }

    pub(crate) fn pop_signal(&mut self) -> Option<Signal> {
        self.signals.pop()
    }

    pub(crate) fn handle_actions<Gpu>(
        &mut self,
        config: Data<Config, Borrowed>,
        gpu: &mut Gpu,
    ) -> Result<(), Error>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface>,
    {
        //TODO: change it to actions which defines in config file

        if let Some(window) = self.window.as_mut() {
            window.handle_hover();

            let Some(signal) = window.handle_click() else {
                return Ok(());
            };

            self.signals.push(signal);
            self.process_notification_queue(config, gpu)?;
        }

        Ok(())
    }

    pub(crate) fn reset_timeouts(&mut self) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            window.reset_timeouts();
        }

        Ok(())
    }

    fn update_window<Gpu>(&mut self, gpu: &mut Gpu) -> Result<(), Error>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface>,
    {
        if let Some(window) = self.window.as_mut() {
            if window.is_empty() {
                return Ok(self.deinit_window(gpu)?);
            }

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

    fn init_window<P, Gpu>(
        &mut self,
        wayland_connection: &Connection,
        protocols: &P,
        gpu: &mut Gpu,
        config: Data<Config, Borrowed>,
    ) -> anyhow::Result<bool>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>,
        Gpu: AsRef<EglState> + AsRef<DirectContext>,
    {
        if self.window.is_none() {
            self.window = Some(Window::init(
                wayland_connection,
                protocols,
                gpu,
                config,
                self.font_collection.clone(),
                self.cached_layouts.borrow(),
            )?);

            debug!("Window Manager: Created a window");

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn deinit_window<Gpu>(&mut self, gpu: &Gpu) -> anyhow::Result<()>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface>,
    {
        if self.window.as_mut().is_some() {
            self.window = None;
            debug!("Window Manager: Closed window");

            let egl_state: &EglState = gpu.as_ref();
            let pbuffer: &NoSurface = gpu.as_ref();
            egl_state.instance.make_current(
                egl_state.display,
                Some(pbuffer.0),
                Some(pbuffer.0),
                Some(egl_state.context),
            )?;
        }

        Ok(())
    }
}
