mod core;
mod dbus;
mod notification;

#[tokio::main]
async fn main() -> core::Result<()> {
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
