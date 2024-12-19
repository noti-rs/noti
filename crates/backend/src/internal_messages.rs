use std::sync::mpsc::{channel, Receiver, Sender};

use dbus::{actions::ClosingReason, notification::Notification};

pub(crate) struct InternalChannel {
    server_channel: ServerChannel,
    backend_channel: BackendChannel,
}

impl InternalChannel {
    pub(crate) fn new() -> Self {
        let (server_sender, backend_receiver) = channel();
        let (backend_sender, server_receiver) = channel();
        Self {
            server_channel: ServerChannel {
                sender: server_sender,
                receiver: server_receiver,
            },
            backend_channel: BackendChannel {
                sender: backend_sender,
                receiver: backend_receiver,
            },
        }
    }

    pub(crate) fn split(self) -> (ServerChannel, BackendChannel) {
        (self.server_channel, self.backend_channel)
    }
}

pub(crate) struct ServerChannel {
    sender: Sender<ServerMessage>,
    receiver: Receiver<BackendMessage>,
}

impl ServerChannel {
    pub(crate) fn send_to_renderer(&self, server_message: ServerMessage) -> anyhow::Result<()> {
        self.sender.send(server_message)?;
        Ok(())
    }

    pub(crate) fn try_recv_from_renderer(&self) -> anyhow::Result<BackendMessage> {
        Ok(self.receiver.try_recv()?)
    }
}

pub(crate) struct BackendChannel {
    sender: Sender<BackendMessage>,
    receiver: Receiver<ServerMessage>,
}

impl BackendChannel {
    pub(crate) fn send_to_server(&self, renderer_message: BackendMessage) -> anyhow::Result<()> {
        self.sender.send(renderer_message)?;
        Ok(())
    }

    pub(crate) fn try_recv_from_server(&self) -> anyhow::Result<ServerMessage> {
        Ok(self.receiver.try_recv()?)
    }
}

pub(crate) enum ServerMessage {
    ShowNotification(Box<Notification>),
    CloseNotification { id: u32 },
}

pub(crate) enum BackendMessage {
    #[allow(unused)]
    ActionInvoked {
        notification_id: u32,
        action_key: String,
    },
    ClosedNotification {
        id: u32,
        reason: ClosingReason,
    },
}
