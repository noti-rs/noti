use std::thread;

use anyhow::Context;
use config::Config;
use internal_messages::InternalChannel;
use log::{debug, info, warn};
use scheduler::Scheduler;
use tokio::sync::mpsc::unbounded_channel;

mod backend_manager;
mod banner;
mod cache;
mod dispatcher;
mod idle_manager;
mod idle_notifier;
mod internal_messages;
mod scheduler;
mod window;
mod window_manager;

use dbus::actions::Action;
use dbus::server::Server;

use backend_manager::BackendManager;

pub async fn run(config: Config) -> anyhow::Result<()> {
    let (sender, mut receiver) = unbounded_channel();

    let server = Server::init(sender).await?;
    info!("Backend: Server initialized");

    let (server_internal_channel, renderer_internal_channel) = InternalChannel::new().split();

    let backend_thread: thread::JoinHandle<Result<(), anyhow::Error>> = thread::spawn(move || {
        let mut renderer = BackendManager::init(config, renderer_internal_channel)?;
        info!("Backend: Renderer initialized");
        renderer.run()
    });

    let mut scheduler = Scheduler::new();
    info!("Backend: Scheduler initialized");

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    debug!(
                        "Backend: Sent a request to renderer to show notification with id: {}",
                        &notification.id
                    );
                    server_internal_channel.send_to_renderer(
                        internal_messages::ServerMessage::ShowNotification(notification),
                    )?;
                }
                Action::Close(Some(id)) => {
                    server_internal_channel.send_to_renderer(
                        internal_messages::ServerMessage::CloseNotification { id },
                    )?;
                    debug!(
                        "Backend: Sent a request to renderer to close notification with id: {id}"
                    );
                }
                Action::Schedule(notification) => {
                    debug!(
                        "Backend: Scheduled notification with id {} for time {}",
                        &notification.id, &notification.time
                    );
                    scheduler.add(notification);
                }
                Action::Close(None) => {
                    warn!("Backend: Received 'Close' action without an id. Ignored");
                }
                Action::CloseAll => {
                    warn!("Backend: Received unsupported 'CloseAll' action. Ignored");
                }
            }
        }

        scheduler
            .pop_due_notifications()
            .into_iter()
            .for_each(|scheduled| {
                server_internal_channel
                    .send_to_renderer(internal_messages::ServerMessage::ShowNotification(
                        scheduled.data,
                    ))
                    .unwrap();
                debug!(
                    "Backend: Notification with id {} due for delivery. Sending to renderer",
                    &scheduled.id
                );
            });

        while let Ok(message) = server_internal_channel.try_recv_from_renderer() {
            match message {
                //TODO: add actions for notifications in render module
                #[allow(unused)]
                internal_messages::BackendMessage::ActionInvoked {
                    notification_id,
                    action_key,
                } => todo!(),
                internal_messages::BackendMessage::ClosedNotification { id, reason } => {
                    match reason {
                        //INFO: ignore the first one because it always emits in server.
                        dbus::actions::ClosingReason::CallCloseNotification => (),
                        other_reason => {
                            debug!("Backend: Closed notification with id {id} for reason {other_reason}");
                            server
                                .emit_signal(dbus::actions::Signal::NotificationClosed {
                                    notification_id: id,
                                    reason: other_reason,
                                })
                                .await?;
                        }
                    }
                }
            }
        }

        if backend_thread.is_finished() {
            return backend_thread
                .join()
                .expect("Join the backend thread to finish main thread")
                .with_context(|| "The backend is shutting down due to an error");
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        std::hint::spin_loop();
    }
}
