use crate::{
    dispatcher::Dispatcher,
    idle_notifier::{IdleNotifier, IdleState},
};
use config::Config;
use log::debug;
use wayland_client::{Connection, EventQueue, QueueHandle};

pub struct IdleManager {
    event_queue: EventQueue<IdleNotifier>,
    qhandle: QueueHandle<IdleNotifier>,
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
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;
        let event_queue = connection.new_event_queue();
        let qhandle = event_queue.handle();
        let display = connection.display();

        display.get_registry(&qhandle, ());

        let idle_notifier = IdleNotifier::init(config)?;

        let idle_manager = Self {
            event_queue,
            qhandle,
            idle_notifier,
        };
        debug!("Idle Manager: Initialized");

        Ok(idle_manager)
    }

    pub(crate) fn update_by_config(&mut self, config: &Config) {
        self.idle_notifier.recreate(&self.qhandle, config);
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
