use std::sync::Arc;

use config::Config;
use log::debug;
use tokio::sync::mpsc::UnboundedSender;

use crate::{FakeConfig, SystemEvent};

#[derive(Debug)]
pub struct ModuleConfig {
    enabled: bool,
}

pub trait Module: Send + Sync + Sized {
    type M: Module;

    fn name() -> &'static str;
    fn init(
        &self,
        sender: UnboundedSender<SystemEvent>,
        config: &FakeConfig,
    ) -> anyhow::Result<Self::M>;
    fn start(self: Arc<Self>) -> anyhow::Result<()>;
    fn configure(&mut self, config: &FakeConfig) -> anyhow::Result<()>;

    fn init_with_logs<F>(
        &self,
        module_name: &str,
        sender: UnboundedSender<crate::SystemEvent>,
        config: &FakeConfig,
        init_fn: F,
    ) -> anyhow::Result<Self::M>
    where
        F: FnOnce(
            tokio::sync::mpsc::UnboundedSender<crate::SystemEvent>,
            &FakeConfig,
        ) -> anyhow::Result<Self::M>,
    {
        debug!(target: "SystemHub", "{}: Initializing", module_name);
        let mut module = init_fn(sender, config)?;

        debug!(target: "SystemHub", "{}: Initialized", module_name);

        debug!(target: "SystemHub", "{}: Configuring", module_name);
        module.configure(config)?;

        debug!(target: "SystemHub", "{}: Configured", module_name);
        Ok(module)
    }
}
