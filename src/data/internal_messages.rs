use std::sync::mpsc::{channel, Receiver, Sender};

use super::{aliases::Result, dbus::ClosingReason, notification::Notification};

pub(crate) struct InternalChannel {
    server_sender: Sender<ServerMessage>,
    server_receiver: Receiver<RendererMessage>,
    renderer_sender: Sender<RendererMessage>,
    renderer_receiver: Receiver<ServerMessage>,
}

impl InternalChannel {
    pub(crate) fn new() -> Self {
        let (server_sender, renderer_receiver) = channel();
        let (renderer_sender, server_receiver) = channel();
        Self {
            server_sender,
            server_receiver,
            renderer_sender,
            renderer_receiver,
        }
    }

    pub(crate) fn split(self) -> (ServerInternalChannel, RendererInternalChannel) {
        (
            ServerInternalChannel {
                server_sender: self.server_sender,
                server_receiver: self.server_receiver,
            },
            RendererInternalChannel {
                renderer_sender: self.renderer_sender,
                renderer_receiver: self.renderer_receiver,
            },
        )
    }
}

pub(crate) struct ServerInternalChannel {
    server_sender: Sender<ServerMessage>,
    server_receiver: Receiver<RendererMessage>,
}

impl ServerInternalChannel {
    pub(crate) fn send_to_renderer(&self, server_message: ServerMessage) -> Result<()> {
        self.server_sender.send(server_message)?;
        Ok(())
    }

    pub(crate) fn try_recv_from_renderer(&self) -> Result<RendererMessage> {
        Ok(self.server_receiver.try_recv()?)
    }
}

pub(crate) struct RendererInternalChannel {
    renderer_sender: Sender<RendererMessage>,
    renderer_receiver: Receiver<ServerMessage>,
}

impl RendererInternalChannel {
    pub(crate) fn send_to_server(&self, renderer_message: RendererMessage) -> Result<()> {
        self.renderer_sender.send(renderer_message)?;
        Ok(())
    }

    pub(crate) fn try_recv_from_server(&self) -> Result<ServerMessage> {
        Ok(self.renderer_receiver.try_recv()?)
    }
}

pub(crate) enum ServerMessage {
    ShowNotification(Notification),
    CloseNotification { id: u32 },
}

pub(crate) enum RendererMessage {
    ActionInvoked {
        notification_id: u32,
        action_key: String,
    },
    ClosedNotification {
        id: u32,
        reason: ClosingReason,
    },
}
