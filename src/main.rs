mod core;
mod data;
mod dbus;
mod render;

use data::aliases::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // let client = Client::init().await?;
    //
    // // startup notification
    // client
    //     .notify(None, None, "Noti is up!", None, Some(1), Some(2000))
    //     .await?;
    //

    core::run().await?;
    Ok(())
}
