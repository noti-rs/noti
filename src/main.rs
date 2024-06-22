use std::error::Error;

mod dbus;
mod notification;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dbus::server::run().await?;

    Ok(())
}
