use config::Config;
use log::debug;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_registry,
        wl_seat::{self, WlSeat},
    },
    Dispatch, QueueHandle,
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

pub struct IdleNotifier {
    wl_seat: Option<WlSeat>,
    notifier: Option<ExtIdleNotifierV1>,
    notification: Option<ExtIdleNotificationV1>,

    threshold: u32,
    pub idle_state: Option<IdleState>,
    pub was_idled: bool,
}

pub enum IdleState {
    Idled,
    Resumed,
}

impl IdleNotifier {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let threshold = config.general().idle_threshold.duration;
        let idle_notifier = Self {
            wl_seat: None,
            notifier: None,
            notification: None,

            idle_state: None,
            threshold,
            was_idled: false,
        };
        debug!("Idle Notifier: Initialized");
        Ok(idle_notifier)
    }

    pub(super) fn recreate(&mut self, qh: &QueueHandle<Self>, config: &Config) {
        self.threshold = config.general().idle_threshold.duration;

        if let Some(notifier) = self.notifier.as_ref() {
            if let Some(notification) = self.notification.take() {
                self.idle_state = None;
                self.was_idled = false;
                notification.destroy();
                debug!("Idle Notifier: Destroyed")
            }

            if self.wl_seat.is_some() && self.threshold != 0 {
                self.notification.replace(notifier.get_idle_notification(
                    self.threshold,
                    self.wl_seat.as_ref().unwrap(),
                    qh,
                    (),
                ));
                debug!("Idle Notifier: Recreated by new idle_threshold value");
            }
        }
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
                    state.wl_seat =
                        Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version, qhandle, ()));
                    debug!("Idle Notifier: Bound the wl_seat");
                }
                "ext_idle_notifier_v1" => {
                    let idle_notifier =
                        registry.bind::<ExtIdleNotifierV1, _, _>(name, version, qhandle, ());
                    debug!("Idle Notifier: Bound the ext_idle_notifier_v1");

                    if state.wl_seat.is_some() && state.threshold != 0 {
                        state
                            .notification
                            .replace(idle_notifier.get_idle_notification(
                                state.threshold,
                                state.wl_seat.as_ref().unwrap(),
                                qhandle,
                                (),
                            ));
                    }

                    state.notifier.replace(idle_notifier);
                }
                _ => (),
            }
        }
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for IdleNotifier {
    fn event(
        state: &mut Self,
        _idle_notification: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: <ext_idle_notification_v1::ExtIdleNotificationV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                state.idle_state = Some(IdleState::Idled);
                state.was_idled = true;
                debug!("Idle Notifier: Idled");
            }
            ext_idle_notification_v1::Event::Resumed => {
                state.idle_state = Some(IdleState::Resumed);
                debug!("Idle Notifier: Resumed");
            }
            _ => (),
        }
    }
}

delegate_noop!(IdleNotifier: ignore WlSeat);
delegate_noop!(IdleNotifier: ignore ExtIdleNotifierV1);
