use tokio::sync::mpsc::unbounded_channel;

use crate::{
    data::{aliases::Result, dbus::Action},
    dbus::server::Server,
};

pub async fn run() -> Result<()> {
    let (sender, mut receiver) = unbounded_channel();
    let _server = Server::init(sender).await?;

    std::hint::spin_loop();

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    if let Some(image_data) = &notification.hints.image_data {
                        println!("image_data ok");
                        // utils::save_image(image_data);
                    };

                    if let Some(image_path) = &notification.hints.image_path {
                        dbg!(image_path);
                    }

                    dbg!(&notification);
                }
                Action::Close(Some(id)) => {
                    dbg!(id);
                }
                Action::Close(None) => {
                    dbg!("close last");
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
