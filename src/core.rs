use tokio::sync::mpsc::unbounded_channel;

use crate::{
    data::{
        aliases::Result,
        dbus::{Action, Signal},
    },
    dbus::server::Server,
};

pub async fn run() -> Result<()> {
    let (sender, mut receiver) = unbounded_channel();
    let server = Server::init(sender).await?;

    std::hint::spin_loop();

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
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
    }
}
