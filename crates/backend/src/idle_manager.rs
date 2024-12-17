use crate::{
    dispatcher::Dispatcher,
    idle_notifier::{IdleNotifier, IdleState},
};
use config::Config;
use log::debug;
use wayland_client::{Connection, EventQueue};

pub struct IdleManager {
    pub event_queue: Option<EventQueue<IdleNotifier>>,
    pub idle_notifier: Option<IdleNotifier>,
}

impl Dispatcher for IdleManager {
    type State = IdleNotifier;

    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)> {
        Some((self.event_queue.as_mut()?, self.idle_notifier.as_mut()?))
    }
}

impl IdleManager {
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;
        let event_queue = connection.new_event_queue();
        let qhandle = event_queue.handle();
        let display = connection.display();

        display.get_registry(&qhandle, ());

        let idle_notifier = IdleNotifier::init(&config)?;

        let idle_manager = Self {
            event_queue: Some(event_queue),
            idle_notifier: Some(idle_notifier),
        };
        debug!("Idle Manager: Initialized");

        Ok(idle_manager)
    }

    pub(crate) fn get_idle_state(&self) -> Option<&IdleState> {
        self.idle_notifier
            .as_ref()
            .and_then(IdleNotifier::get_idle_state)
    }

    pub(crate) fn blocking_dispatch(&mut self) -> anyhow::Result<()> {
        if let Some(event_queue) = self.event_queue.as_mut() {
            event_queue.blocking_dispatch(self.idle_notifier.as_mut().unwrap())?;
        }

        Ok(())
    }
}
