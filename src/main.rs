mod config;
mod core;
mod data;
mod dbus;
mod render;

use config::CONFIG;
use data::aliases::Result;

#[tokio::main]
async fn main() -> Result<()> {
    dbg!(&*CONFIG); // &* used for initializing

    core::run().await?;
    Ok(())
}
