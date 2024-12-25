use std::sync::Arc;

use config::Config;
use derive_more::derive::Display;
use futures_util::{stream::StreamExt, Future};
use log::{debug, error};
use tokio::sync::mpsc::UnboundedSender;

use crate::{module::Module, FakeConfig, SystemEvent};

pub struct NetworkModule {
    sender: UnboundedSender<SystemEvent>,
}

impl Module for NetworkModule {
    type M = NetworkModule;

    fn init(
        &self,
        sender: UnboundedSender<crate::SystemEvent>,
        config: &FakeConfig,
    ) -> anyhow::Result<Self::M> {
        self.init_with_logs(Self::name(), sender, config, |sender, config| {
            let mut m = NetworkModule { sender };
            m.configure(config)?;

            Ok(m)
        })
    }

    fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let self_clone = self.clone();

        tokio::spawn(async move {
            debug!(target: "SystemHub", "starting {} module", Self::name());
            self_clone.listen().await;
            if let Err(e) = self_clone.listen().await {
                error!("Network module failed: {}", e);
            }
        });

        Ok(())
    }

    fn configure(&mut self, config: &FakeConfig) -> anyhow::Result<()> {
        debug!(target: "SystemHub", "configuring {} module", Self::name());

        Ok(())
    }

    fn name() -> &'static str {
        "network"
    }
}

#[derive(Display)]
enum NetworkStateMap {
    Unknown,
    Unmanaged,
    Unavailable,
    Disconnected,
    Prepare,
    Config,
    NeedAuth,
    IPConfig,
    IPCheck,
    Secondaries,
    Activated,
    Deactivating,
    Failed,
}

impl From<u32> for NetworkStateMap {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Unknown,
            10 => Self::Unmanaged,
            20 => Self::Unavailable,
            30 => Self::Disconnected,
            40 => Self::Prepare,
            50 => Self::Config,
            60 => Self::NeedAuth,
            70 => Self::IPConfig,
            80 => Self::IPCheck,
            90 => Self::Secondaries,
            100 => Self::Activated,
            110 => Self::Deactivating,
            120 => Self::Failed,
            _ => unreachable!(),
        }
    }
}

impl NetworkModule {
    pub fn new(sender: UnboundedSender<SystemEvent>) -> Self {
        Self { sender }
    }

    async fn get_wireless_device_path(&self, conn: &zbus::Connection) -> anyhow::Result<String> {
        let proxy = NetworkManagerProxy::new(conn).await?;
        let devices: Vec<zbus::zvariant::OwnedObjectPath> = proxy.get_all_devices().await?;

        for device_path in devices {
            let device_proxy = zbus::Proxy::new(
                conn,
                "org.freedesktop.NetworkManager",
                device_path.as_str(),
                "org.freedesktop.NetworkManager.Device",
            )
            .await?;

            let device_type: u32 = device_proxy.get_property("DeviceType").await?;

            // NOTE: 2 - device type for WiFi devices
            if device_type == 2 {
                return Ok(device_path.to_string());
            }
        }

        anyhow::bail!("No wireless device found")
    }

    pub async fn listen(&self) -> anyhow::Result<()> {
        let conn = zbus::Connection::system().await?;

        let device_path = self.get_wireless_device_path(&conn).await?;

        let device_proxy = DeviceProxy::builder(&conn)
            .path(device_path)?
            .build()
            .await?;

        let state = device_proxy.state().await?;
        debug!("Initial WiFi state: {}", NetworkStateMap::from(state));

        let mut stream = device_proxy.receive_state_changed().await;
        debug!("Listening for WiFi device state changes...");

        while let Some(signal) = stream.next().await {
            let state = signal.get().await?;
            debug!("WiFi state changed to {}", state);

            match NetworkStateMap::from(state) {
                NetworkStateMap::Activated => {
                    self.sender.send(SystemEvent::NetworkConnected {
                        ssid: "todo".to_string(),
                    })?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    fn get_all_devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Device {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}
