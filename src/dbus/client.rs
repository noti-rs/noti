use clap::{Arg, Command};
use std::collections::HashMap;
use std::u32;
use zbus::fdo::Result;
use zbus::{proxy, zvariant::Value, Connection};

#[proxy(
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> Result<u32>;
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("client")
        .args(&[
            Arg::new("app_name")
                .long("app-name")
                .short('n')
                .default_value("test"),
            Arg::new("replaces_id")
                .long("replaces-id")
                .short('r')
                .default_value("0"),
            Arg::new("app_icon")
                .long("app-icon")
                .short('i')
                .default_value(""),
            Arg::new("summary")
                .long("summary")
                .short('s')
                .default_value("test summary"),
            Arg::new("body")
                .long("body")
                .short('b')
                .default_value("test body"),
            Arg::new("urgency")
                .long("urgency")
                .short('u')
                .default_value("1"),
            Arg::new("expire_timeout")
                .long("expire_timeout")
                .short('t')
                .default_value("2000"),
        ])
        .get_matches();

    let app_name = matches.get_one::<String>("app_name").unwrap().to_owned();
    let app_icon = matches.get_one::<String>("app_icon").unwrap().to_owned();
    let summary = matches.get_one::<String>("summary").unwrap().to_owned();
    let body = matches.get_one::<String>("body").unwrap().to_owned();
    let replaces_id = matches
        .get_one::<String>("replaces_id")
        .unwrap()
        .to_owned()
        .parse::<u32>()
        .unwrap();
    let urgency = matches
        .get_one::<String>("urgency")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let expire_timeout = matches
        .get_one::<String>("expire_timeout")
        .unwrap()
        .to_owned()
        .parse::<i32>()
        .unwrap();

    let mut hints = HashMap::new();
    hints.insert("urgency", Value::from(urgency));

    let actions = Vec::new();

    let connection = Connection::session().await?;

    let mut proxy = NotificationsProxy::new(&connection).await?;
    let reply = proxy
        .notify(
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        )
        .await?;
    dbg!(reply);

    Ok(())
}
