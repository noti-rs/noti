use crate::dispatcher::Dispatcher;
use config::Config;
use idle_notifier::{IdleNotifier, IdleState};
use log::debug;
use wayland_client::{protocol::wl_seat::WlSeat, Connection, EventQueue};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::ExtIdleNotifierV1;

mod idle_notifier;

/// Handles idle state events from the Wayland compositor to pause or resume the application.
///
/// This is useful to avoid wasting resources when the user is inactive.
pub struct IdleManager {
    event_queue: EventQueue<IdleNotifier>,
    pub idle_notifier: IdleNotifier,
}

impl Dispatcher for IdleManager {
    type State = IdleNotifier;

    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)> {
        Some((&mut self.event_queue, &mut self.idle_notifier))
    }
}

impl IdleManager {
    /// Initializes the idle manager and makes the idle notifier.
    pub(crate) fn init<P>(
        wayland_connection: &Connection,
        protocols: &P,
        config: &Config,
    ) -> anyhow::Result<Self>
    where
        P: AsRef<WlSeat> + AsRef<ExtIdleNotifierV1>,
    {
        let event_queue = wayland_connection.new_event_queue();
        let idle_notifier = IdleNotifier::init(protocols, &event_queue.handle(), config)?;

        let idle_manager = Self {
            event_queue,
            idle_notifier,
        };
        debug!("Idle Manager: Initialized");

        Ok(idle_manager)
    }

    /// Updates the idle configuration with the new user configuration.
    pub(crate) fn update_by_config<P>(&mut self, protocols: &P, config: &Config)
    where
        P: AsRef<WlSeat> + AsRef<ExtIdleNotifierV1>,
    {
        self.idle_notifier
            .recreate(protocols, &self.event_queue.handle(), config);
    }

    /// Refreshes the idle state, allowing it to wait for the next idle event.
    ///
    /// The idle manager does not refresh the state automatically; this explicit call ensures that
    /// the caller waits for the state to change only after the previous idle state has been handled.
    pub(crate) fn reset_idle_state(&mut self) {
        self.idle_notifier.was_idled = false;
    }

    /// Checks whether an idle event has occurred.
    pub(crate) fn was_idled(&self) -> bool {
        self.idle_notifier.was_idled
    }

    /// Checks whether the state is idle.
    pub(crate) fn is_idled(&self) -> bool {
        self.idle_notifier
            .idle_state
            .as_ref()
            .is_some_and(|state| matches!(state, IdleState::Idled))
    }
}
