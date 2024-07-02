use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub general: General,
    pub display: Display,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct General {
    pub startup_notification: bool,
    pub timeout: u16,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Display {
    pub width: u16,
    pub height: u16,

    pub rounding: u8,
    pub padding: u8,

    pub border: Border,

    pub colors: UrgencyColors,
    pub font: (String, u8),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UrgencyColors {
    low: Colors,
    normal: Colors,
    critical: Colors,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Colors {
    background: String,
    foreground: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Border {
    size: u8,
    color: String,
}

impl Config {
    pub fn parse() -> Self {
        let config_dir = format!(
            "{}/.config/{}/",
            env::var("HOME").unwrap(),
            env!("CARGO_PKG_NAME")
        );

        let config_path = Path::new(&config_dir).join("config.toml");

        if !config_path.parent().unwrap().exists() {
            fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        }

        if !config_path.exists() {
            fs::File::create(&config_path).unwrap();
        }

        let config_content = fs::read_to_string(&config_path).unwrap_or_default();
        let config = toml::from_str(&config_content).unwrap_or_default();
        dbg!(&config);

        config
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config::parse();
}
