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
}

#[derive(Parser)]
pub struct SendCommand {
    #[arg(help = "Summary of the notification")]
    summary: String,

    #[arg(help = "Body text of the notification")]
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
        default_value_t = 1,
        hide_default_value = true,
        help = "Urgency level (1: low, 2: normal, 3: critical)"
    )]
    urgency: u8,

    #[arg(short, long, default_value_t = String::from(""), hide_default_value = true, help = "Notification category")]
    category: String,
}

impl Args {
    pub async fn process(&self) -> anyhow::Result<()> {
        match self {
            Args::Run => {
                let _ = &*CONFIG; // Initializes the configuration.

                backend::run().await?;
            }
            Args::Send(args) => {
                client::send_notification(client::NotificationData {
                    id: args.replaces_id,
                    app_name: args.app_name.to_string(),
                    body: args.body.to_string(),
                    summary: args.summary.to_string(),
                    icon: args.icon.to_string(),
                    timeout: args.timeout.into(),
                    hints: args.hints.to_string(),
                    urgency: args.urgency.into(),
                    category: args.category.to_string(),
                })
                .await?;
            }
        }

        Ok(())
    }
}
