use crate::data::aliases::Result;
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

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct GeneralConfig {
    pub timeout: Option<u16>,
    pub offset: Option<(u8, u8)>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DisplayConfig {
    pub width: Option<u16>,
    pub height: Option<u16>,

    pub rounding: Option<u8>,
    pub padding: Option<u8>,

    pub border: Option<Border>,

    pub colors: Option<UrgencyColors>,
    pub font: Option<(String, u8)>,
}

impl GeneralConfig {
    fn merge(&mut self, other: &GeneralConfig) {
        if self.timeout.is_none() {
            self.timeout = other.timeout;
        }
        if self.offset.is_none() {
            self.offset = other.offset;
        }
    }
}

impl DisplayConfig {
    fn merge(&mut self, other: &DisplayConfig) {
        if self.width.is_none() {
            self.width = other.width;
        }
        if self.height.is_none() {
            self.height = other.height;
        }
        if self.rounding.is_none() {
            self.rounding = other.rounding;
        }
        if self.padding.is_none() {
            self.padding = other.padding;
        }
        if self.border.is_none() {
            self.border = other.border.clone();
        }
        if self.colors.is_none() {
            self.colors = other.colors.clone();
        }
        if self.font.is_none() {
            self.font = other.font.clone();
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct UrgencyColors {
    pub low: Option<Colors>,
    pub normal: Option<Colors>,
    pub critical: Option<Colors>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Colors {
    pub background: Option<String>,
    pub foreground: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
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
        let mut config: Self = toml::from_str(&config_content).unwrap_or_default();

        config.merge().unwrap();
        config
    }

    fn merge(&mut self) -> Result<()> {
        if let Some(apps) = &mut self.apps {
            for app in apps.iter_mut() {
                if let Some(app_general) = &mut app.general {
                    app_general.merge(&self.general);
                } else {
                    app.general = Some(self.general.clone());
                }

                if let Some(app_display) = &mut app.display {
                    app_display.merge(&self.display);
                } else {
                    app.display = Some(self.display.clone());
                }
            }
        }
        Ok(())
    }
}
