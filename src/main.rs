use std::{error::Error, future::pending, sync::Arc};
use tokio::sync::mpsc;

use dbus::{client::Client, server::Server};
use notification::Action;

mod dbus;
mod notification;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (sender, mut receiver) = mpsc::channel(5);

    let mut _server = Server::init(sender).await?;
    let mut client = Client::init().await?;

    // server.setup_handler(sender).await?;
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

    // TODO: handle signals

    pending::<()>().await;

    Ok(())
}
