mod config;
mod core;
mod data;
mod dbus;
mod render;

use data::aliases::Result;

#[tokio::main]
async fn main() -> Result<()> {
    core::run().await?;
    Ok(())
}
