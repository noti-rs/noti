use dbus::{client::Client, server::Server};
use notification::{Action, Signal};
use std::{error::Error, future::pending};
use tokio::sync::mpsc;

mod dbus;
mod notification;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (action_tx, mut action_rx) = mpsc::channel(5);
    let (signal_tx, mut signal_rx) = mpsc::unbounded_channel();

    // NOTE: sig_sender here - is a temporary solution
    let server = Server::init(action_tx, signal_tx).await?;
    let mut client = Client::init().await?;

    client
        .notify(
            "Noti".into(),
            1,
            "".into(),
            "Noti is up!".into(),
            "".into(),
            1,
            2000,
        )
        .await?;

    tokio::spawn(async move {
        while let Some(act) = action_rx.recv().await {
            match act {
                Action::Show(notification) => {
                    dbg!(notification);
                }
                Action::Close(Some(id)) => {
                    dbg!(id);
                }
                Action::Close(None) => {
                    todo!("close last");
                }
                Action::ShowLast => {
                    todo!("show last");
                }
                Action::CloseAll => {
                    todo!("show all");
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(sig) = signal_rx.recv().await {
            match sig {
                Signal::ActionInvoked { notification_id } => {
                    server
                        .connection
                        .emit_signal(
                            None::<()>,
                            "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications",
                            "ActionInvoked",
                            &(notification_id),
                        )
                        .await
                        .unwrap();

                    dbg!(notification_id);
                }
                Signal::NotificationClosed {
                    notification_id,
                    reason,
                } => {
                    server
                        .connection
                        .emit_signal(
                            None::<()>,
                            "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications",
                            "NotificationClosed",
                            &(notification_id, reason),
                        )
                        .await
                        .unwrap();

                    dbg!(notification_id, reason);
                }
            }
        }
    });

    // TODO: handle signals

    pending::<()>().await;
    unreachable!()
}
