use std::sync::Arc;

use log::debug;
use tokio::sync::mpsc::UnboundedSender;

use crate::{module::Module, FakeConfig, SystemEvent};

pub struct PowerModule {
    sender: UnboundedSender<SystemEvent>,
}

impl PowerModule {}

// TODO: ...
impl Module for PowerModule {
    type M = PowerModule;

    fn init(
        &self,
        sender: UnboundedSender<crate::SystemEvent>,
        config: &FakeConfig,
    ) -> anyhow::Result<Self::M> {
        self.init_with_logs(Self::name(), sender, config, |sender, config| {
            let mut m = PowerModule { sender };
            m.configure(config)?;

            Ok(m)
        })
    }

    fn configure(&mut self, config: &FakeConfig) -> anyhow::Result<()> {
        debug!(target: "SystemHub", "configuring {} module", Self::name());

        Ok(())
    }

    fn start(self: Arc<Self>) -> anyhow::Result<()> {
        let self_clone = self.clone();

        std::thread::spawn(|| {
            debug!(target: "SystemHub", "starting {} module", Self::name());
            // self_clone.listen();
        });

        Ok(())
    }

    fn name() -> &'static str {
        "power"
    }
}
