use std::thread;

use anyhow::Context;
use config::Config;
use log::{debug, info, warn};
use tokio::sync::mpsc::unbounded_channel;

mod internal_messages;
mod render;
mod window;
mod window_manager;

use dbus::actions::Action;
use dbus::server::Server;

use render::Renderer;

pub async fn run(config: Config) -> anyhow::Result<()> {
    let (sender, mut receiver) = unbounded_channel();
    let server = Server::init(sender).await?;
    info!("Backend: Server initialized");

    let (server_internal_channel, mut renderer) = Renderer::init(config)?;
    info!("Backend: Renderer initialized");

    let backend_thread = thread::spawn(move || renderer.run());

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    server_internal_channel.send_to_renderer(
                        internal_messages::ServerMessage::ShowNotification(notification),
                    )?;
                    debug!("Backend: Sended notification to render to show");
                }
                Action::Close(Some(id)) => {
                    server_internal_channel.send_to_renderer(
                        internal_messages::ServerMessage::CloseNotification { id },
                    )?;
                    debug!("Backend: Sended notification with id {id} to render to close it");
                }
                Action::Close(None) => {
                    warn!("Backend: Unsupported method 'Close'. Ignored");
                }
                Action::ShowLast => {
                    //TODO: make decision about this action. It may be very useless
                    warn!("Backend: Unsupported method 'ShowLast'. Ignored");
                }
                Action::CloseAll => {
                    warn!("Backend: Unsupported method 'CloseAll'. Ignored");
                }
            }
        }

        while let Ok(message) = server_internal_channel.try_recv_from_renderer() {
            match message {
                //TODO: add actions for notifications in render module
                #[allow(unused)]
                internal_messages::RendererMessage::ActionInvoked {
                    notification_id,
                    action_key,
                } => todo!(),
                internal_messages::RendererMessage::ClosedNotification { id, reason } => {
                    match reason {
                        //INFO: ignore the first one because it always emits in server.
                        dbus::actions::ClosingReason::CallCloseNotification => (),
                        other_reason => {
                            debug!("Backend: Closed notification with id {id} and reason {other_reason}");
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
                .with_context(|| "The backend shutdowns due to error");
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        std::hint::spin_loop();
    }
}
