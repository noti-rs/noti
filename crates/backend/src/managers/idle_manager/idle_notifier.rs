use config::Config;
use log::debug;
use wayland_client::{protocol::wl_seat::WlSeat, Dispatch, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

/// Represents a simple object that tracks the idle state reported by the Wayland compositor,
/// indicating whether the user has been inactive long enough to be considered idle.
///
/// Requires a `threshold` (in milliseconds) after which the compositor reports the object as idle.
///
/// This object also provides a way to handle cases where an idle event has occurred even if
/// the current state is [IdleState::Resumed], allowing important actions to be processed
/// (e.g., refreshing notifications, timeouts, and similar tasks).
pub struct IdleNotifier {
    /// The use of an option here is intentional: if `threshold` is zero, it would be pointless
    /// to create a notification object from the Wayland compositor.
    notification: Option<ExtIdleNotificationV1>,

    /// The `threshold` value, in milliseconds.  
    ///
    /// A value of zero indicates no limit.
    threshold: u32,
    pub idle_state: Option<IdleState>,
    pub was_idled: bool,
}

pub enum IdleState {
    Idled,
    Resumed,
}

impl IdleNotifier {
    /// Initializes the idle notifier object.
    pub(crate) fn init<P>(
        protocotls: &P,
        qhandle: &QueueHandle<Self>,
        config: &Config,
    ) -> anyhow::Result<Self>
    where
        P: AsRef<WlSeat> + AsRef<ExtIdleNotifierV1>,
    {
        let threshold = config.general().idle_threshold.duration;

        let notification = if threshold != 0 {
            Some(
                <P as AsRef<ExtIdleNotifierV1>>::as_ref(protocotls).get_idle_notification(
                    threshold,
                    <P as AsRef<WlSeat>>::as_ref(protocotls),
                    qhandle,
                    (),
                ),
            )
        } else {
            None
        };

        let idle_notifier = Self {
            notification,

            idle_state: None,
            threshold,
            was_idled: false,
        };

        debug!("Idle Notifier: Initialized");
        Ok(idle_notifier)
    }

    /// Updates the `threshold` value from the user configuration and recreates the idle notifier
    /// with the new value. The idle notifier is not created if the `threshold` is zero.
    pub(super) fn recreate<P>(
        &mut self,
        protocotls: &P,
        qhandle: &QueueHandle<Self>,
        config: &Config,
    ) where
        P: AsRef<WlSeat> + AsRef<ExtIdleNotifierV1>,
    {
        self.threshold = config.general().idle_threshold.duration;
        if let Some(notification) = self.notification.take() {
            self.idle_state = None;
            self.was_idled = false;
            notification.destroy();
            debug!("Idle Notifier: Destroyed")
        }

        if self.threshold != 0 {
            self.notification.replace(
                <P as AsRef<ExtIdleNotifierV1>>::as_ref(protocotls).get_idle_notification(
                    self.threshold,
                    <P as AsRef<WlSeat>>::as_ref(protocotls),
                    qhandle,
                    (),
                ),
            );
            debug!("Idle Notifier: Recreated by new idle_threshold value");
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
