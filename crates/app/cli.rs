use clap::Parser;
use config::CONFIG;

/// The notification system which derives a notification to user
/// using wayland client.
#[derive(Parser)]
#[command(version, about, name = env!("CARGO_PKG_NAME"))]
pub enum Args {
    /// Starts the backend. Use it in systemd, openrc or any other services.
    Run,

    /// Sends the notification
    Send(SendCommand),

    /// Prints server information
    ServerInfo,
}

#[derive(Parser)]
pub struct SendCommand {
    #[arg(help = "Summary of the notification")]
    summary: String,

    #[arg(default_value_t = String::from(""), help = "Body text of the notification")]
    body: String,

    #[arg(short, long, default_value_t = String::from("Noti"), hide_default_value = true, help = "The name of the application")]
    app_name: String,

    #[arg(
        short,
        long,
        default_value_t = 0,
        hide_default_value = true,
        help = "ID of the notification to replace"
    )]
    replaces_id: u32,

    #[arg(short, long, default_value_t = String::from(""), hide_default_value = true, help = "Path to the icon file")]
    icon: String,

    #[arg(short, long, default_value_t = -1, hide_default_value = true, help = "Timeout in milliseconds")]
    timeout: i32,

    #[arg(short = 'H', long, default_value_t = String::from(""), hide_default_value = true, help = "Hints")]
    hints: String,

    #[arg(
        short,
        long,
        default_value_t = String::from("normal"),
        hide_default_value = true,
        help = "Urgency level (low, normal, critical)"
    )]
    urgency: String,

    #[arg(short, long, default_value_t = String::from(""), hide_default_value = true, help = "Notification category")]
    category: String,
}

impl Args {
    pub async fn process(&self) -> anyhow::Result<()> {
        let noti = client::NotiClient::init().await;

        match self {
            Args::Run => {
                let _ = &*CONFIG; // Initializes the configuration.

                backend::run().await?;
            }
            Args::Send(args) => {
                noti.send_notification(client::NotificationData {
                    id: args.replaces_id,
                    app_name: &args.app_name,
                    body: &args.body,
                    summary: &args.summary,
                    icon: &args.icon,
                    timeout: args.timeout,
                    hints: &args.hints,
                    urgency: &args.urgency,
                    category: &args.category,
                })
                .await?;
            }
            Args::ServerInfo => noti.get_server_info().await?,
        }

        Ok(())
    }
}
