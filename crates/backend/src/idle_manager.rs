use config::Config;
use log::debug;
use wayland_client::{Connection, EventQueue};

use crate::idle_notifier::IdleNotifier;

pub struct IdleManager {
    event_queue: Option<EventQueue<IdleNotifier>>,
    idle_notifier: Option<IdleNotifier>,
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

    pub(crate) fn dispatch(&mut self) -> anyhow::Result<bool> {
        if self.event_queue.is_none() {
            return Ok(false);
        }

        let event_queue = unsafe { self.event_queue.as_mut().unwrap_unchecked() };
        let idle_notifier = unsafe { self.idle_notifier.as_mut().unwrap_unchecked() };

        let dispatched_count = event_queue.dispatch_pending(idle_notifier)?;

        if dispatched_count > 0 {
            return Ok(true);
        }

        event_queue.flush()?;
        let Some(guard) = event_queue.prepare_read() else {
            return Ok(false);
        };
        let Ok(count) = guard.read() else {
            return Ok(false);
        };

        Ok(if count > 0 {
            event_queue.dispatch_pending(idle_notifier)?;
            true
        } else {
            false
        })
    }
}
