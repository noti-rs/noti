use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub general: GeneralConfig,
    pub display: DisplayConfig,

    #[serde(rename(deserialize = "app"))]
    pub apps: Option<Vec<AppConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GeneralConfig {
    pub timeout: Option<u16>,
    pub offset: Option<(u8, u8)>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DisplayConfig {
    pub width: Option<u16>,
    pub height: Option<u16>,

    pub rounding: Option<u8>,
    pub padding: Option<u8>,

    pub border: Option<Border>,

    pub colors: Option<UrgencyColors>,
    pub font: Option<(String, u8)>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UrgencyColors {
    pub low: Option<Colors>,
    pub normal: Option<Colors>,
    pub critical: Option<Colors>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Colors {
    pub background: Option<String>,
    pub foreground: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Border {
    pub enabled: Option<bool>,
    pub size: Option<u8>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub name: String,
    pub general: Option<GeneralConfig>,
    pub display: Option<DisplayConfig>,
}

pub static CONFIG: Lazy<Config> = Lazy::new(Config::parse);

impl Config {
    fn parse() -> Self {
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

        let config_content = fs::read_to_string(&config_path).unwrap();
        let config = toml::from_str(&config_content).unwrap_or_default();

        config
    }
}
