use std::{error::Error, sync::Arc};
use tokio::sync::mpsc;

use dbus::{client::Client, server::Server};
use notification::Action;

mod dbus;
mod notification;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut server = Server::init().await?;
    let mut client = Client::init().await?;
    let (sender, receiver) = mpsc::channel(5);
    let receiver_clone = Arc::new(tokio::sync::Mutex::new(receiver));

    server.setup_handler(sender).await?;

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

    loop {
        let receiver = Arc::clone(&receiver_clone);

        tokio::spawn(async move {
            let mut receiver = receiver.lock().await;

            while let Some(act) = receiver.recv().await {
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
    }

    // TODO: signals handling
}
