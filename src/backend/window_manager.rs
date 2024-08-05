use std::sync::Arc;

use wayland_client::{Connection, EventQueue, QueueHandle};

use crate::{
    config::CONFIG,
    data::{aliases::Result, internal_messages::RendererMessage, notification::Notification},
};

use super::{render::FontCollection, window::Window};

pub(crate) struct WindowManager {
    connection: Connection,
    event_queue: Option<EventQueue<Window>>,
    qhandle: Option<QueueHandle<Window>>,
    window: Option<Window>,

    font_collection: Arc<FontCollection>,

    events: Vec<RendererMessage>,
}

impl WindowManager {
    pub(crate) fn init() -> Result<Self> {
        let connection = Connection::connect_to_env()?;
        let font_collection = Arc::new(FontCollection::load_by_font_name(
            CONFIG.general().font().name(),
        )?);

        Ok(Self {
            connection,
            event_queue: None,
            qhandle: None,
            window: None,

            font_collection,

            events: vec![],
        })
    }

    pub(crate) fn create_notifications(&mut self, notifications: Vec<Notification>) {
        let _ = self.init_window();

        let window = unsafe { self.window.as_mut().unwrap_unchecked() };
        window.update_banners(notifications);

        self.update_window();
        self.roundtrip_event_queue();
    }

    pub(crate) fn close_notifications(&mut self, indices: &[u32]) {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_banners_by_id(indices);

            if notifications.is_empty() {
                return;
            }

            notifications
                .into_iter()
                .map(|notification| notification.id)
                .for_each(|id| {
                    self.events.push(RendererMessage::ClosedNotification {
                        id,
                        reason: crate::data::dbus::ClosingReason::CallCloseNotification,
                    })
                });

            self.update_window();
            self.roundtrip_event_queue();
        }
    }

    pub(crate) fn remove_expired(&mut self) {
        if let Some(window) = self.window.as_mut() {
            let notifications = window.remove_expired_banners();

            if notifications.is_empty() {
                return;
            }

            notifications.into_iter().for_each(|notification| {
                self.events.push(RendererMessage::ClosedNotification {
                    id: notification.id,
                    reason: crate::data::dbus::ClosingReason::Expired,
                })
            });

            self.update_window();
            self.roundtrip_event_queue();
        }
    }

    pub(crate) fn pop_event(&mut self) -> Option<RendererMessage> {
        self.events.pop()
    }

    pub(crate) fn handle_actions(&mut self) {
        //TODO: change it to actions which defines in config file

        if let Some(window) = self.window.as_mut() {
            let messages = window.handle_click();
            if messages.is_empty() {
                return;
            }

            self.events.extend(messages);

            self.update_window();
            self.roundtrip_event_queue();
        }
    }

    pub(crate) fn dispatch(&mut self) -> bool {
        if self.event_queue.is_none() {
            return false;
        }

        let event_queue = unsafe { self.event_queue.as_mut().unwrap_unchecked() };
        let window = unsafe { self.window.as_mut().unwrap_unchecked() };

        let dispatched_count = event_queue
            .dispatch_pending(window)
            .expect("Successful dispatch");

        if dispatched_count > 0 {
            return true;
        }

        event_queue.flush().expect("Successful event queue flush");
        let guard = event_queue.prepare_read().expect("Get read events guard");
        let Ok(count) = guard.read() else {
            return false;
        };

        if count > 0 {
            event_queue
                .dispatch_pending(window)
                .expect("Successful dispatch");
            true
        } else {
            false
        }
    }

    fn update_window(&mut self) {
        if let Some(window) = self.window.as_mut() {
            if window.is_empty() {
                self.deinit_window();
                return;
            }

            let qhandle = unsafe { self.qhandle.as_ref().unwrap_unchecked() };

            window.draw(qhandle);
            window.frame(qhandle);
            window.commit();
        }
    }

    fn roundtrip_event_queue(&mut self) {
        if let Some(event_queue) = self.event_queue.as_mut() {
            event_queue
                .roundtrip(unsafe { self.window.as_mut().unwrap_unchecked() })
                .unwrap();
        }
    }

    fn init_window(&mut self) -> bool {
        if let None = self.window {
            let mut event_queue = self.connection.new_event_queue();
            let qhandle = event_queue.handle();
            let display = self.connection.display();
            display.get_registry(&qhandle, ());

            let mut window = Window::init(self.font_collection.clone());
            while !window.is_configured() {
                let _ = event_queue.blocking_dispatch(&mut window);
            }

            self.event_queue = Some(event_queue);
            self.qhandle = Some(qhandle);
            self.window = Some(window);
            true
        } else {
            false
        }
    }

    fn deinit_window(&mut self) {
        unsafe {
            let window = self.window.as_mut().unwrap_unchecked();
            window.deinit();
            self.event_queue
                .as_mut()
                .unwrap_unchecked()
                .roundtrip(window)
                .unwrap();
        }
        self.window = None;
        self.event_queue = None;
        self.qhandle = None;
    }
}
