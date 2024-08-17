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

    #[arg(default_value_t = String::from(""), hide_default_value = true, help = "Body text of the notification")]
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

    #[arg(short = 'A', long, help = "Actions")]
    actions: Vec<String>,

    #[arg(short = 'H', long, help = "Hints")]
    hints: Vec<String>,

    #[arg(
        short,
        long,
        default_value_t = String::from("normal"),
        hide_default_value = true,
        help = "Urgency level (low, normal, critical)"
    )]
    urgency: String,

    #[arg(short, long, help = "Notification category")]
    category: Option<String>,

    #[arg(short, long, help = "Desktop entry path")]
    desktop_entry: Option<String>,

    #[arg(short = 'I', long, help = "Path to image")]
    image_path: Option<String>,

    #[arg(short = 'R', long, help = "Resident")]
    resident: Option<bool>,

    #[arg(short, long, help = "Path to sound file")]
    sound_file: Option<String>,

    #[arg(short = 'N', long, help = "Sound name")]
    sound_name: Option<String>,

    #[arg(short = 'S', long, help = "Suppress sound")]
    suppress_sound: Option<bool>,

    #[arg(short = 'T', long, help = "Transient")]
    transient: Option<bool>,

    #[arg(short = 'C', long, help = "Action icons")]
    action_icons: Option<bool>,

    #[arg(
        short,
        long,
        help = "X location on the screen that the notification should point to"
    )]
    x: Option<i32>,

    #[arg(
        short,
        long,
        help = "Y location on the screen that the notification should point to"
    )]
    y: Option<i32>,
}

impl Args {
    pub async fn process(&self) -> anyhow::Result<()> {
        if let Args::Run = self {
            self.run().await?
        }

        let noti = client::NotiClient::init().await?;

        match self {
            Args::Run => unreachable!(),
            Args::Send(args) => self.send(noti, args).await?,
            Args::ServerInfo => self.server_info(noti).await?,
        }

        Ok(())
    }

    async fn run(&self) -> anyhow::Result<()> {
        let _ = &*CONFIG; // Initializes the configuration.

        backend::run().await
    }

    async fn send(&self, noti: client::NotiClient<'_>, args: &SendCommand) -> anyhow::Result<()> {
        let hints_data = client::HintsData {
            urgency: args.urgency.clone(),
            category: args.category.clone(),
            desktop_entry: args.desktop_entry.clone(),
            image_path: args.image_path.clone(),
            sound_file: args.sound_file.clone(),
            sound_name: args.sound_name.clone(),
            resident: args.resident,
            suppress_sound: args.suppress_sound,
            transient: args.transient,
            action_icons: args.action_icons,
            x: args.x,
            y: args.y,
        };

        noti.send_notification(
            args.replaces_id,
            &args.app_name,
            &args.icon,
            &args.summary,
            &args.body,
            args.timeout,
            &args.actions,
            &args.hints,
            &hints_data,
        )
        .await
    }

    async fn server_info(&self, noti: client::NotiClient<'_>) -> anyhow::Result<()> {
        noti.get_server_info().await
    }
}
