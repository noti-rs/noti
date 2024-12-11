use clap::Parser;
use config::Config;

/// The notification system which derives a notification to user
/// using wayland client.
#[derive(Parser)]
#[command(version, about, name = env!("APP_NAME"))]
pub enum Args {
    /// Start the backend. Use it in systemd, openrc or any other service.
    Run(Box<RunCommand>),

    /// Send the notification
    Send(Box<SendCommand>),

    /// Print server information
    ServerInfo,
}

#[derive(Parser)]
pub struct RunCommand {
    #[arg(
        short,
        long,
        help = "Path to config file",
        long_help = "Path to config file which will be used primarily"
    )]
    config: Option<String>,
}

#[derive(Parser)]
pub struct SendCommand {
    #[arg(help = "Summary", long_help = "Summary of the notification")]
    summary: String,

    #[arg(
        default_value_t = String::from(""),
        hide_default_value = true,
        help = "Body",
        long_help = "Body text of the notification"
    )]
    body: String,

    #[arg(
        short,
        long,
        default_value_t = String::from("Noti"),
        hide_default_value = true,
        help = "The name of the application"
    )]
    app_name: String,

    #[arg(
        short,
        long,
        default_value_t = 0,
        hide_default_value = true,
        help = "ID",
        long_help = "ID of the notification to replace"
    )]
    replaces_id: u32,

    #[arg(
        short,
        long,
        default_value_t = String::from(""),
        hide_default_value = true,
        help = "Icon",
        long_help = "Path to the icon file"
    )]
    icon: String,

    #[arg(short, long, default_value_t = -1, hide_default_value = true, help = "Timeout in milliseconds")]
    timeout: i32,

    #[arg(
        short = 'A',
        long,
        help = "Actions",
        long_help = "Extra actions that define interactive options for the notification"
    )]
    actions: Vec<String>,

    #[arg(
        short = 'H',
        long,
        help = "Hints",
        long_help = "Extra hints that modify notification behavior"
    )]
    hints: Vec<String>,

    #[arg(short, long, help = "Urgency level (low, normal, critical)")]
    urgency: Option<String>,

    #[arg(
        short,
        long,
        help = "Notification category",
        long_help = "The type of notification this is"
    )]
    category: Option<String>,

    #[arg(
        short,
        long,
        help = "Desktop entry path",
        long_help = "Desktop entry filename representing the calling program"
    )]
    desktop_entry: Option<String>,

    #[arg(
        short = 'I',
        long,
        help = "Image file",
        long_help = "Path to image file"
    )]
    image_path: Option<String>,

    #[arg(
        short = 'R',
        long,
        help = "Resident",
        long_help = "Prevents automatic removal of notifications after an action"
    )]
    resident: Option<bool>,

    #[arg(
        long,
        help = "Sound file",
        long_help = "Path to a sound file to play when the notification pops up"
    )]
    sound_file: Option<String>,

    #[arg(
        short = 'N',
        long,
        help = "Sound name",
        long_help = "A themeable sound name to play when the notification pops up"
    )]
    sound_name: Option<String>,

    #[arg(
        short = 'S',
        long,
        help = "Suppress sound",
        long_help = "Causes the server to suppress playing any sounds"
    )]
    suppress_sound: Option<bool>,

    #[arg(
        short = 'T',
        long,
        help = "Transient",
        long_help = "Marks the notification as transient, bypassing the server's persistence capability if available"
    )]
    transient: Option<bool>,

    #[arg(
        short = 'C',
        long,
        help = "Action icons",
        long_help = "Interprets action IDs as icons, annotated by display names"
    )]
    action_icons: Option<bool>,

    #[arg(
        short = 's',
        long,
        help = "Schedule",
        long_help = "Specifies the time to schedule the notification to be shown."
    )]
    schedule: String,
}

impl Args {
    pub async fn process(self) -> anyhow::Result<()> {
        if let Args::Run(ref args) = self {
            run(args).await?
        }

        let noti = client::NotiClient::init().await?;

        match self {
            Args::Run { .. } => unreachable!(),
            Args::Send(args) => send(noti, *args).await?,
            Args::ServerInfo => server_info(noti).await?,
        }

        Ok(())
    }
}

async fn run(args: &RunCommand) -> anyhow::Result<()> {
    let config = Config::init(args.config.as_deref());
    backend::run(config).await
}

async fn send(noti: client::NotiClient<'_>, args: SendCommand) -> anyhow::Result<()> {
    let hints_data = client::HintsData {
        urgency: args.urgency,
        category: args.category,
        desktop_entry: args.desktop_entry,
        image_path: args.image_path,
        sound_file: args.sound_file,
        sound_name: args.sound_name,
        resident: args.resident,
        suppress_sound: args.suppress_sound,
        transient: args.transient,
        action_icons: args.action_icons,
    };

    noti.send_notification(
        args.replaces_id,
        args.app_name,
        args.icon,
        args.summary,
        args.body,
        args.timeout,
        args.actions,
        args.hints,
        args.schedule,
        hints_data,
    )
    .await
}

async fn server_info(noti: client::NotiClient<'_>) -> anyhow::Result<()> {
    noti.get_server_info().await
}
