use ::image::{ImageBuffer, Rgb};
use dbus::{client::Client, server::Server};
use notification::{Action, Signal};
use std::{error::Error, future::pending, path::Path};
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
        .notify("Noti", 1, "", "Noti is up!", "", 1, 2000)
        .await?;

    tokio::spawn(async move {
        while let Some(act) = action_rx.recv().await {
            match act {
                Action::Show(notification) => {
                    if let Some(image_data) = &notification.image_data {
                        let width = image_data.width as u32;
                        let height = image_data.height as u32;

                        dbg!(&width, &height);

                        let buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
                            ImageBuffer::from_vec(width, height, image_data.data.to_vec())
                                .expect("failed to create image buffer");

                        let file_path = Path::new("output.png");
                        buffer.save(file_path).expect("failed to save image");

                        println!("umage saved to {}", file_path.display());
                    };
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

    pending::<()>().await;
    unreachable!()
}
