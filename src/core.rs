use std::thread;

use tokio::sync::mpsc::unbounded_channel;

use crate::{
    data::{aliases::Result, dbus::Action},
    dbus::server::Server,
    render::Renderer,
};

pub async fn run() -> Result<()> {
    let (sender, mut receiver) = unbounded_channel();
    let _server = Server::init(sender).await?;

    let (server_internal_channel, mut renderer) = Renderer::init()?;

    thread::spawn(move || renderer.run());

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    server_internal_channel.send_to_renderer(
                        crate::data::internal_messages::ServerMessage::ShowNotification(
                            notification,
                        ),
                    )?;
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

        std::hint::spin_loop();
    }
}
