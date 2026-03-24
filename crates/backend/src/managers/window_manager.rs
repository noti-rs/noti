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
use wayland_protocols::wp::{
    cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
    presentation_time::client::wp_presentation::WpPresentation,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use window::Window;

mod banner_stack;
mod cache;
mod window;

/// Manages application windows through a convenient API, abstracting away explicit window
/// management and providing high-level access to window-related operations.
///
/// Also this struct stores shared data between windows.
pub(crate) struct WindowManager {
    window: Option<Window>,

    font_collection: skia_safe::textlayout::FontCollection,
    cached_layouts: Data<CachedData<PathBuf, CachedLayout>, Owned>,

    signals: Vec<Signal>,

    notification_queue: VecDeque<Notification>,
    close_notifications: Vec<u32>,
}

impl WindowManager {
    /// Initializes the window manager and loads layouts and fonts into the cache.
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

    /// Dispatches the window events into Wayland compositor.
    pub(crate) fn dispatch(&mut self) -> anyhow::Result<bool> {
        if let Some(window) = self.window.as_mut() {
            window.dispatch()?;
        }

        Ok(false)
    }

    /// Updates the layout cache if any layouts have changed.
    pub(crate) fn update_cache(&mut self) -> bool {
        self.cached_layouts.update()
    }

    /// Updates the shared data and cache with the new user configuration. If any windows are open,
    /// they are updated as well.
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
        }

        debug!("Window Manager: Updated the windows by updated config");
        Ok(())
    }

    /// Shows notification if possible.
    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        self.notification_queue.push_back(*notification);
    }

    /// Closes a notification by ID.
    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.close_notifications.push(notification_id);
    }

    /// Shows the window only if there are notifications to display.
    ///
    /// If the window is shown, this method also triggers a compositor frame
    /// request by calling [`Self::frame_window`], ensuring that the first frame
    /// is rendered immediately.
    pub(crate) fn show_window<P, Gpu>(
        &mut self,
        wayland_connection: &Connection,
        gpu: &mut Gpu,
        protocols: &P,
        config: Data<Config, Borrowed>,
    ) -> Result<(), Error>
    where
        P: AsRef<WlCompositor>
            + AsRef<WlShm>
            + AsRef<WlSeat>
            + AsRef<WpCursorShapeManagerV1>
            + AsRef<ZwlrLayerShellV1>
            + AsRef<WpPresentation>,
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
            self.init_window(wayland_connection, protocols, gpu, config.clone())?;
            self.process_notification_queue(config)?;
            self.frame_window(gpu, protocols)?;
        } else if self.window.is_some() {
            self.frame_window(gpu, protocols)?;
        }

        Ok(())
    }

    /// Returns `true` if the window is currently visible on screen.
    ///
    /// This method is used to decide whether to keep the render loop running
    /// eagerly (for smooth animations and responsiveness) or to slow it down
    /// during idle periods to save CPU.
    pub(crate) fn is_window_visible(&self) -> bool {
        self.window.is_some()
    }

    /// Notification management in the window manager is queue-based. Replacing a notification by ID
    /// bypasses the queue limit; other notifications will be added to the window until the limit is
    /// reached.
    ///
    /// In case the window does not exist, these actions are not performed.
    fn process_notification_queue(&mut self, config: Data<Config, Borrowed>) -> Result<(), Error> {
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
        }

        Ok(())
    }

    /// Handles requests from external applications to close notifications by ID.
    ///
    /// Other kinds of notification closing are not handled here.
    pub(crate) fn handle_close_notifications(&mut self) -> Result<(), Error> {
        self.notification_queue
            .retain(|notification| !self.close_notifications.contains(&notification.id));

        if self.window.as_ref().is_some() && !self.close_notifications.is_empty() {
            let window = self.window.as_mut().unwrap();

            window.close_banners_by_id(&self.close_notifications);
            self.close_notifications.clear();
        }

        Ok(())
    }

    pub(crate) fn remove_closed(&mut self, config: Data<Config, Borrowed>) -> Result<(), Error> {
        if let Some(window) = self.window.as_mut() {
            let closed_notifications = window.remove_closed_banners();

            if closed_notifications.is_empty() {
                return Ok(());
            }

            for (notification, closing_reason) in closed_notifications {
                self.signals.push(Signal::NotificationClosed {
                    notification_id: notification.id,
                    reason: closing_reason,
                })
            }

            self.process_notification_queue(config)?;
        }

        Ok(())
    }

    /// Removes the last stored signal from the state and returns it.
    /// This signal is used as a response in D-Bus communication.
    pub(crate) fn pop_signal(&mut self) -> Option<Signal> {
        self.signals.pop()
    }

    /// Handles user interaction with the window, if any has occurred.
    pub(crate) fn handle_actions(&mut self) -> Result<(), Error> {
        //TODO: change it to actions which defines in config file

        if let Some(window) = self.window.as_mut() {
            window.handle_hover();

            // TODO: also add bypass by config value named 'bypass_click' or something similar
            window.handle_click();
        }

        Ok(())
    }

    /// Resets the timeout for all notifications.
    ///
    /// This is useful when the configuration is updated with new values or other important
    /// changes related to notifications occur.
    pub(crate) fn reset_timeouts(&mut self) -> anyhow::Result<()> {
        if let Some(window) = self.window.as_mut() {
            window.reset_timeouts();
        }

        Ok(())
    }

    /// Requests a new frame from the Wayland compositor if no other frame
    /// request is pending.
    ///
    /// This method respects the compositor's drawing loop and avoids spamming
    /// frame requests by consulting [`Window::has_requested_frame`].
    /// It should be called whenever a new frame needs to be drawn (e.g.,
    /// when the window becomes visible or during animations).
    fn frame_window<Gpu, P>(&mut self, gpu: &mut Gpu, protocols: &P) -> Result<(), Error>
    where
        Gpu: AsRef<EglState> + AsRef<NoSurface>,
        P: AsRef<WpPresentation>,
    {
        if let Some(window) = self.window.as_mut() {
            if window.is_empty() {
                return Ok(self.deinit_window(gpu)?);
            }

            if !window.has_requested_frame() {
                window.frame();
                window.commit(protocols.as_ref());

                debug!("Window Manager: Requested a frame for window");
            }
        }

        Ok(())
    }

    /// Initializes a window to show notifications.
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

    /// Deinitializes the window to free resources on the output.
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
