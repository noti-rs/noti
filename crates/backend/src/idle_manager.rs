use crate::{
    dispatcher::Dispatcher,
    idle_notifier::{IdleNotifier, IdleState},
};
use config::Config;
use log::debug;
use wayland_client::{protocol::wl_seat::WlSeat, Connection, EventQueue};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::ExtIdleNotifierV1;

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

    pub(crate) fn update_by_config<P>(&mut self, protocols: &P, config: &Config)
    where
        P: AsRef<WlSeat> + AsRef<ExtIdleNotifierV1>,
    {
        self.idle_notifier
            .recreate(protocols, &self.event_queue.handle(), config);
    }

    pub(crate) fn reset_idle_state(&mut self) {
        self.idle_notifier.was_idled = false;
    }

    pub(crate) fn was_idled(&self) -> bool {
        self.idle_notifier.was_idled
    }

    pub(crate) fn is_idled(&self) -> bool {
        self.idle_notifier
            .idle_state
            .as_ref()
            .is_some_and(|state| matches!(state, IdleState::Idled))
    }
}
