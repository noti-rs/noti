use dbus::{client::Client, server::Server};
use notification::{Action, Signal};
use std::{error::Error, future::pending};
use tokio::sync::mpsc;

mod dbus;
mod notification;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (action_tx, mut action_rx) = mpsc::channel(5);

    let server = Server::init(action_tx).await?;
    let client = Client::init().await?;

    // startup notification
    client
        .notify(None, None, "Noti is up!", None, Some(1), Some(2000))
        .await?;

    tokio::spawn(async move {
        while let Some(act) = action_rx.recv().await {
            match act {
                Action::Show(notification) => {
                    if let Some(image_data) = &notification.image_data {
                        println!("image_data ok");
                        // utils::save_image(image_data);
                    };

                    if let Some(image_path) = &notification.image_path {
                        dbg!(image_path);
                    }

                    dbg!(&notification);
                }
                Action::Close(Some(id)) => {
                    dbg!(id);

                    server
                        .emit_signal(Signal::NotificationClosed {
                            notification_id: id,
                            reason: 0,
                        })
                        .await
                        .unwrap();
                }
                Action::Close(None) => {
                    dbg!("close last");

                    server
                        .emit_signal(Signal::NotificationClosed {
                            notification_id: 0,
                            reason: 0,
                        })
                        .await
                        .unwrap();
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

    pending::<()>().await;
    unreachable!()
}
