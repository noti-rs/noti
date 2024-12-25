mod module;
mod modules;

use std::sync::Arc;

use log::{debug, error};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    module::Module,
    modules::{network::NetworkModule, power::PowerModule},
};

struct FakeConfig {
    network: NetworkFakeConfig,
}

struct NetworkFakeConfig {
    enabled: bool,
}

pub struct SystemHub<'a> {
    config: FakeConfig, // TODO: config
    modules: Vec<Modules>,
    sender: UnboundedSender<SystemEvent>,
    receiver: UnboundedReceiver<SystemEvent>,
    client: client::NotiClient<'a>,
}

enum Modules {
    Network(Arc<NetworkModule>),
    Power(Arc<PowerModule>),
}

pub enum SystemEvent {
    NetworkConnected { ssid: String },
    PowerLowBattery { level: u8 },
    PowerCharging,
    DeviceAdded { device_name: String },
}

impl<'a> SystemHub<'a> {
    pub fn init(client: client::NotiClient<'a>) -> anyhow::Result<Self> {
        let (sender, receiver) = unbounded_channel();
        let modules = vec![];

        // TODO: config
        let config = FakeConfig {
            network: NetworkFakeConfig { enabled: true },
        };

        let mut hub = Self {
            config,
            modules,
            sender,
            receiver,
            client,
        };

        hub.setup()?;
        debug!(target: "SystemHub", "created");

        Ok(hub)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.start_modules().await?;

        loop {
            tokio::select! {
                Some(event) = self.receiver.recv() => {
                    if let Err(e) = self.handle_event(event).await {
                        error!("Failed to handle event: {}", e);
                    }
                }
            };
        }
    }

    async fn send_notification(
        &self,
        app_name: &str,
        icon_path: &str,
        summary: &str,
        body: &str,
    ) -> anyhow::Result<()> {
        // TODO: ...
        self.client
            .send_notification(
                0,
                app_name.to_string(),
                icon_path.to_string(),
                summary.to_string(),
                body.to_string(),
                0,
                vec![],
                vec![],
                client::HintsData {
                    urgency: None,
                    category: None,
                    desktop_entry: None,
                    image_path: None,
                    resident: None,
                    sound_file: None,
                    sound_name: None,
                    suppress_sound: None,
                    transient: None,
                    action_icons: None,
                    schedule: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn handle_event(&self, event: SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::NetworkConnected { ssid } => {
                self.send_notification(
                    "network_module",
                    "/path/to/icon.svg",
                    "Network",
                    format!("Connected to {}", ssid).as_str(),
                )
                .await
                .expect("wtf is going on");
            }
            _ => unimplemented!(),
        };

        Ok(())
    }

    fn setup(&mut self) -> anyhow::Result<()> {
        if self.config.network.enabled {
            let module = NetworkModule::new(self.sender.clone());
            self.register_module(Modules::Network(Arc::new(module)));
        }

        // TODO: other modules

        self.setup_modules()
    }

    fn register_module(&mut self, module: Modules) {
        self.modules.push(module);
    }

    fn setup_modules(&mut self) -> anyhow::Result<()> {
        for module in &self.modules {
            match module {
                Modules::Network(module) => {
                    module.init(self.sender.clone(), &self.config)?;
                }
                Modules::Power(module) => {
                    module.init(self.sender.clone(), &self.config)?;
                }
            }
        }

        Ok(())
    }

    async fn start_modules(&self) -> anyhow::Result<()> {
        for module in &self.modules {
            match module {
                Modules::Network(module) => {
                    module.clone().start()?;
                }
                Modules::Power(module) => {
                    module.clone().start()?;
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }
}
