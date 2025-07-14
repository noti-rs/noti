use std::time::Duration;

use crate::idle_manager::IdleManager;
use crate::{dispatcher::Dispatcher, error::Error};

use anyhow::bail;
use config::Config;
use dbus::{actions::Signal, notification::Notification};
use derive_builder::Builder;
use log::{debug, error};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::{delegate_noop, Connection, Dispatch};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::ExtIdleNotifierV1;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

use super::window_manager::WindowManager;

const DEFAULT_SETUP_TIME: Duration = Duration::from_secs(10);

pub(crate) struct BackendManager {
    wayland_connection: Connection,
    state: BackendState,
    window_manager: WindowManager,
    idle_manager: IdleManager,
}

#[derive(Builder)]
struct BackendState {
    wl_compositor: WlCompositor,
    wl_shm: WlShm,
    wl_seat: WlSeat,
    ext_idle_notifier: ExtIdleNotifierV1,
    zwlr_layer_shell: ZwlrLayerShellV1,
    wp_cursor_shape_manager: WpCursorShapeManagerV1,
}

impl BackendManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let wayland_connection = Connection::connect_to_env()?;
        let mut event_queue = wayland_connection.new_event_queue();
        let mut backend_state_builder = BackendStateBuilder::create_empty();

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
            state,
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

            window_manager.show_window(&self.wayland_connection, &self.state, config)?;

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
        idle_manager.update_by_config(&self.state, config);
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

impl_as_ref!(BackendState: wl_compositor => WlCompositor);
impl_as_ref!(BackendState: wl_shm => WlShm);
impl_as_ref!(BackendState: wl_seat => WlSeat);
impl_as_ref!(BackendState: ext_idle_notifier => ExtIdleNotifierV1);
impl_as_ref!(BackendState: zwlr_layer_shell => ZwlrLayerShellV1);
impl_as_ref!(BackendState: wp_cursor_shape_manager => WpCursorShapeManagerV1);

impl Dispatch<WlRegistry, ()> for BackendStateBuilder {
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

delegate_noop!(BackendStateBuilder: ignore WlCompositor);
delegate_noop!(BackendStateBuilder: ignore ZwlrLayerShellV1);
delegate_noop!(BackendStateBuilder: ignore WlShm);
delegate_noop!(BackendStateBuilder: ignore WlSeat);
delegate_noop!(BackendStateBuilder: ignore WpCursorShapeManagerV1);
delegate_noop!(BackendStateBuilder: ignore ExtIdleNotifierV1);
