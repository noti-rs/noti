use anyhow::bail;
use config::Config;
use dbus::{
    actions::{Action, ClosingReason, Signal},
    notification::Notification,
    server::Server,
};
use derive_builder::Builder;
use dispatcher::Dispatcher;
use error::Error;
use log::{debug, error, info, warn};
use managers::{idle_manager::IdleManager, window_manager::WindowManager};
use scheduler::Scheduler;
use shared::file_watcher::FileState;
use tokio::sync::mpsc::unbounded_channel;
use std::time::Duration;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_compositor::WlCompositor, wl_registry::WlRegistry, wl_seat::WlSeat, wl_shm::WlShm,
    },
    Connection, Dispatch,
};
use wayland_protocols::{
    ext::idle_notify::v1::client::ext_idle_notifier_v1::ExtIdleNotifierV1,
    wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

mod dispatcher;
mod error;
mod managers;
mod scheduler;

pub async fn run(mut config: Config) -> anyhow::Result<()> {
    let (sender, mut receiver) = unbounded_channel();

    let server = Server::init(sender).await?;
    info!("Server: initialized");
    let mut backend = Backend::init(&config)?;
    info!("Backend: initialized");

    let mut scheduler = Scheduler::new();
    info!("Scheduler: initialized");

    let mut partially_default_config = false;

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    backend.create_notification(notification);
                }
                Action::Close(Some(id)) => {
                    backend.close_notification(id);
                }
                Action::Schedule(notification) => {
                    debug!(
                        "Backend: Scheduled notification with id {} for time {}",
                        &notification.id, &notification.time
                    );
                    scheduler.add(notification);
                }
                Action::Close(None) => {
                    warn!("Backend: Received 'Close' action without an id. Ignored");
                }
                Action::CloseAll => {
                    warn!("Backend: Received unsupported 'CloseAll' action. Ignored");
                }
            }
        }

        scheduler
            .pop_due_notifications()
            .into_iter()
            .for_each(|scheduled| {
                backend.create_notification(scheduled.data);
                debug!(
                    "Backend: Notification with id {} due for delivery",
                    &scheduled.id
                );
            });

        backend.poll(&config).handle_error()?;

        match config.check_updates() {
            FileState::Updated => {
                partially_default_config = false;
                config.update();
                backend.update_config(&config).handle_error()?;
                info!("Renderer: Detected changes of config files and updated")
            }
            FileState::NotFound if !partially_default_config => {
                partially_default_config = true;
                config.update();
                backend.update_config(&config).handle_error()?;
                info!("The main or imported configuration file is not found, reverting this part to default values.");
            }
            FileState::NotFound | FileState::NothingChanged => (),
        };

        while let Some(signal) = backend.pop_signal() {
            //INFO: ignore this one because it always emits at server
            if let Signal::NotificationClosed {
                reason: ClosingReason::CallCloseNotification,
                ..
            } = &signal
            {
                continue;
            }
            debug_signal(&signal);
            server.emit_signal(signal).await?;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        std::hint::spin_loop();
    }
}

trait HandleError<T> {
    fn handle_error(self) -> anyhow::Result<T>;
}

impl<T: Default> HandleError<T> for Result<T, Error> {
    fn handle_error(self) -> anyhow::Result<T> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => match err {
                Error::UnrenderedNotifications(_vec) => {
                    //TODO: handle unrenedered banners
                    Ok(Default::default())
                }
                Error::Fatal(error) => Err(error)?,
            },
        }
    }
}

fn debug_signal(signal: &Signal) {
    match signal {
        Signal::ActionInvoked {
            notification_id,
            action_key,
        } => debug!("Action '{action_key}' was invoked for notification id {notification_id}"),
        Signal::NotificationClosed {
            notification_id,
            reason,
        } => debug!("Notification with id {notification_id} closed by {reason} reason"),
    }
}

pub(crate) struct Backend {
    wayland_connection: Connection,
    protocols: Protocols,
    window_manager: WindowManager,
    idle_manager: IdleManager,
}

#[derive(Builder)]
struct Protocols {
    wl_compositor: WlCompositor,
    wl_shm: WlShm,
    wl_seat: WlSeat,
    ext_idle_notifier: ExtIdleNotifierV1,
    zwlr_layer_shell: ZwlrLayerShellV1,
    wp_cursor_shape_manager: WpCursorShapeManagerV1,
}

impl Backend {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        const DEFAULT_SETUP_TIME: Duration = Duration::from_secs(10);

        let wayland_connection = Connection::connect_to_env()?;
        let mut event_queue = wayland_connection.new_event_queue();
        let mut backend_state_builder = ProtocolsBuilder::create_empty();

        wayland_connection
            .display()
            .get_registry(&event_queue.handle(), ());
        let timer = std::time::Instant::now();

        while backend_state_builder.build().is_err() && timer.elapsed() < DEFAULT_SETUP_TIME {
            event_queue.blocking_dispatch(&mut backend_state_builder)?;
        }

        let state = match backend_state_builder.build() {
            Ok(state) => state,
            Err(_) => {
                error!("BackendManager: Failed to init due missing protocols of compositor.");
                bail!("Failed to init noti backend due missing protocosls of compositor.")
            }
        };

        Ok(Self {
            window_manager: WindowManager::init(config)?,
            idle_manager: IdleManager::init(&wayland_connection, &state, config)?,
            wayland_connection,
            protocols: state,
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
            ..
        } = self;

        if !idle_manager.is_idled() {
            if idle_manager.was_idled() {
                idle_manager.reset_idle_state();

                window_manager.reset_timeouts()?;
            }

            window_manager.show_window(&self.wayland_connection, &self.protocols, config)?;

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
            ..
        } = self;

        window_manager.update_by_config(config)?;
        idle_manager.update_by_config(&self.protocols, config);
        window_manager.reset_timeouts()?;
        Ok(())
    }
}

macro_rules! impl_as_ref {
    ($source_type:ty: $field:ident => $derived_type:ty) => {
        impl AsRef<$derived_type> for $source_type {
            fn as_ref(&self) -> &$derived_type {
                &self.$field
            }
        }
    };
}

impl_as_ref!(Protocols: wl_compositor => WlCompositor);
impl_as_ref!(Protocols: wl_shm => WlShm);
impl_as_ref!(Protocols: wl_seat => WlSeat);
impl_as_ref!(Protocols: ext_idle_notifier => ExtIdleNotifierV1);
impl_as_ref!(Protocols: zwlr_layer_shell => ZwlrLayerShellV1);
impl_as_ref!(Protocols: wp_cursor_shape_manager => WpCursorShapeManagerV1);

impl Dispatch<WlRegistry, ()> for ProtocolsBuilder {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_ref() {
                "wl_compositor" => {
                    state.wl_compositor(registry.bind::<WlCompositor, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                    debug!("Backend: Bound the wl_compositor");
                }
                "wl_shm" => {
                    state.wl_shm(registry.bind::<WlShm, _, _>(name, version, qhandle, ()));
                    debug!("Backend: Bound the wl_shm");
                }
                "wl_seat" => {
                    state.wl_seat(registry.bind::<WlSeat, _, _>(name, version, qhandle, ()));
                    debug!("Backend: Bound the wl_seat");
                }
                "zwlr_layer_shell_v1" => {
                    state.zwlr_layer_shell(registry.bind::<ZwlrLayerShellV1, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                    debug!("Backend: Bound the zwlr_layer_shell_v1");
                }
                "ext_idle_notifier_v1" => {
                    state.ext_idle_notifier(registry.bind::<ExtIdleNotifierV1, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                    debug!("Backend: Bound the ext_idle_notifier_v1");
                }
                "wp_cursor_shape_manager_v1" => {
                    state.wp_cursor_shape_manager = Some(
                        registry.bind::<WpCursorShapeManagerV1, _, _>(name, version, qhandle, ()),
                    );

                    debug!("Backend: Bound the wp_cursor_shape_manager_v1");
                }
                _ => (),
            }
        }
    }
}

delegate_noop!(ProtocolsBuilder: ignore WlCompositor);
delegate_noop!(ProtocolsBuilder: ignore ZwlrLayerShellV1);
delegate_noop!(ProtocolsBuilder: ignore WlShm);
delegate_noop!(ProtocolsBuilder: ignore WlSeat);
delegate_noop!(ProtocolsBuilder: ignore WpCursorShapeManagerV1);
delegate_noop!(ProtocolsBuilder: ignore ExtIdleNotifierV1);
