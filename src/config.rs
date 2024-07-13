use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path, str::Chars};

pub static CONFIG: Lazy<Config> = Lazy::new(Config::init);

#[derive(Debug)]
pub struct Config {
    general: GeneralConfig,
    display: DisplayConfig,

    app_configs: HashMap<String, DisplayConfig>,
}

impl Config {
    pub fn init() -> Self {
        let TomlConfig {
            general,
            mut display,
            apps,
        } = TomlConfig::parse();

        display.fill_empty_by_default();
        let mut app_configs =
            HashMap::with_capacity(apps.as_ref().map(|data| data.len()).unwrap_or(0));

        if let Some(apps) = apps {
            for mut app in apps {
                app.merge(&display);
                app_configs.insert(app.name, app.display.unwrap());
            }
        }

        Self {
            general,
            display,
            app_configs,
        }
    }

    pub fn general(&self) -> &GeneralConfig {
        &self.general
    }

    pub fn default_display(&self) -> &DisplayConfig {
        &self.display
    }

    pub fn display_by_app(&self, name: &str) -> &DisplayConfig {
        self.app_configs.get(name).unwrap_or(&self.display)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct TomlConfig {
    general: GeneralConfig,
    display: DisplayConfig,

    #[serde(rename(deserialize = "app"))]
    apps: Option<Vec<AppConfig>>,
}

impl TomlConfig {
    fn parse() -> Self {
        let config_dirs = [
            format!("{}/{}/", env!("XDG_CONFIG_HOME"), env!("CARGO_PKG_NAME")),
            format!("{}/.config/{}/", env!("HOME"), env!("CARGO_PKG_NAME")),
        ];

        config_dirs
            .iter()
            .map(|str| Path::new(&str).join("config.toml"))
            .find(|path| path.exists())
            .map(|config_path| fs::read_to_string(&config_path).unwrap())
            .map(|content| toml::from_str(&content).unwrap())
            .unwrap_or(Default::default())
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct GeneralConfig {
    font: Font,

    #[serde(default = "GeneralConfig::default_width")]
    width: u16,
    #[serde(default = "GeneralConfig::default_height")]
    height: u16,

    anchor: Anchor,
    offset: (u8, u8),
}

impl GeneralConfig {
    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    pub fn offset(&self) -> (u8, u8) {
        self.offset
    }

    fn default_width() -> u16 {
        300
    }

    fn default_height() -> u16 {
        150
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(from = "(String, u8)")]
pub struct Font {
    name: String,
    size: u8,
}

impl Font {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> u8 {
        self.size
    }
}

impl From<(String, u8)> for Font {
    fn from((name, size): (String, u8)) -> Self {
        Font { name, size }
    }
}

impl Default for Font {
    fn default() -> Self {
        Font {
            name: "Noto Sans".to_string(),
            size: 12,
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(from = "String")]
pub enum Anchor {
    Top,
    TopLeft,
    #[default]
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
    Left,
    Right,
}

impl Anchor {
    pub fn is_top(&self) -> bool {
        match self {
            Anchor::Top | Anchor::TopLeft | Anchor::TopRight => true,
            _ => false,
        }
    }

    pub fn is_right(&self) -> bool {
        match self {
            Anchor::TopRight | Anchor::BottomRight | Anchor::Right => true,
            _ => false,
        }
    }

    pub fn is_bottom(&self) -> bool {
        match self {
            Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight => true,
            _ => false,
        }
    }

    pub fn is_left(&self) -> bool {
        match self {
            Anchor::TopLeft | Anchor::BottomLeft | Anchor::Left => true,
            _ => false,
        }
    }
}

impl From<String> for Anchor {
    fn from(value: String) -> Self {
        match value.as_str() {
            "top" => Anchor::Top,
            "top-left" => Anchor::TopLeft,
            "top-right" => Anchor::TopRight,
            "bottom" => Anchor::Bottom,
            "bottom-left" => Anchor::BottomLeft,
            "bottom-right" => Anchor::BottomRight,
            "left" => Anchor::Left,
            "right" => Anchor::Right,
            other => panic!(
                "Invalid anchor option! There are possible values:\n\
                - top\n\
                - top-right\n\
                - top-left\n\
                - bottom\n\
                - bottom-right\n\
                - bottom-left\n\
                - left\n\
                - right\n\
                Used: {other}"
            ),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct DisplayConfig {
    image_size: Option<u16>,

    rounding: Option<u8>,
    padding: Option<u8>,

    border: Option<Border>,

    colors: Option<UrgencyColors>,
    timeout: Option<u16>,
}

impl DisplayConfig {
    pub fn image_size(&self) -> u16 {
        self.image_size.unwrap()
    }

    pub fn rounding(&self) -> u8 {
        self.rounding.unwrap()
    }

    pub fn padding(&self) -> u8 {
        self.padding.unwrap()
    }

    pub fn border(&self) -> &Border {
        self.border.as_ref().unwrap()
    }

    pub fn colors(&self) -> &UrgencyColors {
        self.colors.as_ref().unwrap()
    }

    pub fn timeout(&self) -> u16 {
        self.timeout.unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.image_size.is_none() {
            self.image_size = Some(64);
        }

        if self.rounding.is_none() {
            self.rounding = Some(0);
        }

        if self.padding.is_none() {
            self.padding = Some(0);
        }

        if self.border.is_none() {
            self.border = Some(Default::default());
        }
        self.border.as_mut().unwrap().fill_empty_by_default();

        if self.colors.is_none() {
            self.colors = Some(Default::default());
        }
        self.colors.as_mut().unwrap().fill_empty_by_default();

        if self.timeout.is_none() {
            self.timeout = Some(0);
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct UrgencyColors {
    low: Option<Colors>,
    normal: Option<Colors>,
    critical: Option<Colors>,
}

impl UrgencyColors {
    pub fn low(&self) -> &Colors {
        self.low.as_ref().unwrap()
    }

    pub fn normal(&self) -> &Colors {
        self.normal.as_ref().unwrap()
    }

    pub fn critical(&self) -> &Colors {
        self.critical.as_ref().unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.low.is_none() {
            self.low = Some(Default::default());
        }
        self.low.as_mut().unwrap().fill_empty_by_default();

        if self.normal.is_none() {
            self.normal = Some(Default::default());
        }
        self.normal.as_mut().unwrap().fill_empty_by_default();

        if self.critical.is_none() {
            self.critical = Some(Default::default());
        }
        let critical = self.critical.as_mut().unwrap();
        critical.fill_empty_by_default();
        critical.foreground.as_mut().unwrap().red = 255;
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Colors {
    background: Option<Color>,
    foreground: Option<Color>,
}

impl Colors {
    pub fn background(&self) -> &Color {
        self.background.as_ref().unwrap()
    }

    pub fn foreground(&self) -> &Color {
        self.foreground.as_ref().unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.background.is_none() {
            self.background = Some(Color {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            });
        }

        if self.foreground.is_none() {
            self.foreground = Some(Color {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            });
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(from = "String")]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl From<String> for Color {
    fn from(value: String) -> Self {
        if value.len() == 4 {
            let mut chars = value.chars();
            chars.next(); // Skip the hashtag
            let next_digit = |chars: &mut Chars| chars.next().unwrap().to_digit(16).unwrap() as u8;

            Color {
                red: next_digit(&mut chars),
                green: next_digit(&mut chars),
                blue: next_digit(&mut chars),
                alpha: 255,
            }
        } else {
            let data = &value[1..];
            Color {
                red: u8::from_str_radix(&data[0..2], 16).unwrap(),
                green: u8::from_str_radix(&data[2..4], 16).unwrap(),
                blue: u8::from_str_radix(&data[4..6], 16).unwrap(),
                alpha: if data.len() == 8 {
                    u8::from_str_radix(&data[6..8], 16).unwrap()
                } else {
                    255
                },
            }
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Border {
    size: Option<u8>,
    color: Option<Color>,
}

impl Border {
    pub fn size(&self) -> u8 {
        self.size.unwrap()
    }

    pub fn color(&self) -> &Color {
        self.color.as_ref().unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.size.is_none() {
            self.size = Some(0);
        }

        if self.color.is_none() {
            self.color = Some(Default::default());
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    pub name: String,
    pub display: Option<DisplayConfig>,
}

impl AppConfig {
    fn merge(&mut self, other: &DisplayConfig) {
        if let Some(display) = self.display.as_mut() {
            display.image_size = display.image_size.or(other.image_size);

            display.rounding = display.rounding.or(other.rounding);
            display.padding = display.padding.or(other.padding);

            if let Some(border) = display.border.as_mut() {
                let other_border = other.border(); // The other type shall have border
                border.size = border.size.or(other_border.size);
                border.color = border.color.clone().or(other_border.color.clone());
            } else {
                display.border = other.border.clone();
            }

            if let Some(colors) = display.colors.as_mut() {
                let other_colors = other.colors(); // The other type shall have
                                                   // colors
                colors.low = colors.low.clone().or(other_colors.low.clone());
                colors.normal = colors.normal.clone().or(other_colors.normal.clone());
                colors.critical = colors.critical.clone().or(other_colors.critical.clone());
            } else {
                display.colors = other.colors.clone();
            }

            display.timeout = display.timeout.or(other.timeout);
        } else {
            self.display = Some(other.clone());
        }
    }
}
