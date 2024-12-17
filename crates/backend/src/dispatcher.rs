use wayland_client::EventQueue;

pub trait Dispatcher {
    type State;

    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)>;

    fn dispatch(&mut self) -> anyhow::Result<bool> {
        let (event_queue, state) = match self.get_event_queue_and_state() {
            Some(queue) => queue,
            None => return Ok(false),
        };

        let dispatched_count = event_queue.dispatch_pending(state)?;

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
            event_queue.dispatch_pending(state)?;
            true
        } else {
            false
        })
    }
}
