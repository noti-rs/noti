use config::Config;
use error::Error;
use log::{debug, info, warn};
use scheduler::Scheduler;
use shared::file_watcher::FileState;
use tokio::sync::mpsc::unbounded_channel;

mod backend_manager;
mod banner;
mod cache;
mod dispatcher;
mod error;
mod idle_manager;
mod idle_notifier;
mod scheduler;
mod window;
mod window_manager;

use dbus::actions::{Action, ClosingReason, Signal};
use dbus::server::Server;

use backend_manager::BackendManager;

pub async fn run(mut config: Config) -> anyhow::Result<()> {
    let (sender, mut receiver) = unbounded_channel();

    let server = Server::init(sender).await?;
    info!("Backend: Server initialized");
    let mut backend_manager = BackendManager::init(&config)?;
    info!("Backend: Manager initialized");

    let mut scheduler = Scheduler::new();
    info!("Backend: Scheduler initialized");

    let mut partially_default_config = false;

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    backend_manager.create_notification(notification);
                }
                Action::Close(Some(id)) => {
                    backend_manager.close_notification(id);
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
                backend_manager.create_notification(scheduled.data);
                debug!(
                    "Backend: Notification with id {} due for delivery",
                    &scheduled.id
                );
            });

        backend_manager.poll(&config).handle_error()?;

        match config.check_updates() {
            FileState::Updated => {
                partially_default_config = false;
                config.update();
                backend_manager.update_config(&config).handle_error()?;
                info!("Renderer: Detected changes of config files and updated")
            }
            FileState::NotFound if !partially_default_config => {
                partially_default_config = true;
                config.update();
                backend_manager.update_config(&config).handle_error()?;
                info!("The main or imported configuration file is not found, reverting this part to default values.");
            }
            FileState::NotFound | FileState::NothingChanged => (),
        };

        while let Some(signal) = backend_manager.pop_signal() {
            //INFO: ignore this one because it always emits at server
            if let Signal::NotificationClosed {
                reason: ClosingReason::CallCloseNotification,
                ..
            } = &signal
            {
                continue;
            }
            debug_signal(&signal);
            server.emit_signal(signal).await?;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        std::hint::spin_loop();
    }
}

trait HandleError<T> {
    fn handle_error(self) -> anyhow::Result<T>;
}

impl<T: Default> HandleError<T> for Result<T, Error> {
    fn handle_error(self) -> anyhow::Result<T> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => match err {
                Error::UnrenderedNotifications(_vec) => {
                    //TODO: handle unrenedered banners
                    Ok(Default::default())
                }
                Error::Fatal(error) => Err(error)?,
            },
        }
    }
}

fn debug_signal(signal: &Signal) {
    match signal {
        Signal::ActionInvoked {
            notification_id,
            action_key,
        } => debug!("Action '{action_key}' was invoked for notification id {notification_id}"),
        Signal::NotificationClosed {
            notification_id,
            reason,
        } => debug!("Notification with id {notification_id} closed by {reason} reason"),
    }
}
