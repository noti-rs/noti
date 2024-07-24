use clap::Parser;

use crate::{backend, config::CONFIG, data::aliases::Result};

/// The notification system which derives a notification to user
/// using wayland client.
#[derive(Parser)]
#[command(version, about, name = env!("CARGO_PKG_NAME"))]
pub enum Args {
    /// Starts the backend. Use it in systemd, openrc or any other services.
    Run,
}

impl Args {
    pub async fn process(&self) -> Result<()> {
        match self {
            Args::Run => {
                let _ = &*CONFIG; // Initializes the configuration.

                backend::run().await?;
            }
        }

        Ok(())
    }
}
