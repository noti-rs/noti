use config::Config;
use log::debug;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_registry,
        wl_seat::{self, WlSeat},
    },
    Dispatch,
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

#[derive(Debug)]
pub struct IdleNotifier {
    seat: Option<WlSeat>,
    idle_notification: Option<ExtIdleNotificationV1>,
    threshold: u16,
}

impl IdleNotifier {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let threshold = config.general().idle_threshold;
        let idle_notifier = Self {
            seat: None,
            idle_notification: None,
            threshold,
        };
        debug!("Idle Notifier: Initialized");
        Ok(idle_notifier)
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for IdleNotifier {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_ref() {
                "wl_seat" => {
                    state.seat =
                        Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version, qhandle, ()));
                    debug!("Idle Notifier: Bound the wl_seat");
                }
                "ext_idle_notifier_v1" => {
                    let idle_notifier =
                        registry.bind::<ExtIdleNotifierV1, _, _>(name, version, qhandle, ());
                    debug!("Idle Notifier: Bound the ext_idle_notifier_v1");

                    if let Some(seat) = state.seat.as_ref() {
                        state.idle_notification = Some(idle_notifier.get_idle_notification(
                            state.threshold as u32,
                            seat,
                            qhandle,
                            (),
                        ));
                    }
                }
                _ => (),
            }
        }
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for IdleNotifier {
    fn event(
        _state: &mut Self,
        _idle_notification: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: <ext_idle_notification_v1::ExtIdleNotificationV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                debug!("Idle");
            }
            ext_idle_notification_v1::Event::Resumed => {
                debug!("Huy");
            }
            _ => (),
        }
    }
}

delegate_noop!(IdleNotifier: ignore WlSeat);
delegate_noop!(IdleNotifier: ignore ExtIdleNotifierV1);
