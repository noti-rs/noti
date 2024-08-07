use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path, str::Chars};

pub mod sorting;
pub mod spacing;

use sorting::Sorting;
use spacing::Spacing;

use crate::data::notification::Urgency;

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

    #[allow(unused)]
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
        let pkg_name = env!("CARGO_PKG_NAME");
        std::env::var("XDG_CONFIG_HOME")
            .or(std::env::var("HOME").map(|home_path| home_path + "/.config"))
            .map(|config_path| format!("{config_path}/{pkg_name}/"))
            .map(|str| Path::new(&str).join("config.toml"))
            .map(|config_path| fs::read_to_string(&config_path).unwrap())
            .map(|content| {
                toml::from_str(&content).unwrap_or_else(|err| {
                    eprintln!("{err}");
                    std::process::exit(1);
                })
            })
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
    #[serde(default = "GeneralConfig::default_gap")]
    gap: u8,

    sorting: Sorting,
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

    pub fn gap(&self) -> u8 {
        self.gap
    }

    pub fn sorting(&self) -> &Sorting {
        &self.sorting
    }

    fn default_width() -> u16 {
        300
    }

    fn default_height() -> u16 {
        150
    }

    fn default_gap() -> u8 {
        10
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
            "top-left" | "top left" => Anchor::TopLeft,
            "top-right" | "top right" => Anchor::TopRight,
            "bottom" => Anchor::Bottom,
            "bottom-left" | "bottom left" => Anchor::BottomLeft,
            "bottom-right" | "bottom right" => Anchor::BottomRight,
            "left" => Anchor::Left,
            "right" => Anchor::Right,
            other => panic!(
                "Invalid anchor option! There are possible values:\n\
                - \"top\"\n\
                - \"top-right\" or \"top right\"\n\
                - \"top-left\" or \"top left\"\n\
                - bottom\n\
                - \"bottom-right\" or \"bottom right\"\n\
                - \"bottom-left\" or \"bottom left\"\n\
                - left\n\
                - right\n\
                Used: {other}"
            ),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct DisplayConfig {
    image: Option<ImageProperty>,

    padding: Option<Spacing>,
    border: Option<Border>,

    colors: Option<UrgencyColors>,

    title: Option<TextProperty>,
    body: Option<TextProperty>,
    ellipsize_at: Option<EllipsizeAt>,
    markup: Option<bool>,

    timeout: Option<u16>,
}

impl DisplayConfig {
    pub fn image(&self) -> &ImageProperty {
        self.image.as_ref().unwrap()
    }

    pub fn padding(&self) -> &Spacing {
        self.padding.as_ref().unwrap()
    }

    pub fn border(&self) -> &Border {
        self.border.as_ref().unwrap()
    }

    pub fn colors(&self) -> &UrgencyColors {
        self.colors.as_ref().unwrap()
    }

    pub fn title(&self) -> &TextProperty {
        self.title.as_ref().unwrap()
    }

    pub fn body(&self) -> &TextProperty {
        self.body.as_ref().unwrap()
    }

    pub fn ellipsize_at(&self) -> &EllipsizeAt {
        self.ellipsize_at.as_ref().unwrap()
    }

    pub fn markup(&self) -> bool {
        self.markup.unwrap()
    }

    pub fn timeout(&self) -> u16 {
        self.timeout.unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.image.is_none() {
            self.image = Some(Default::default());
        }
        self.image.as_mut().unwrap().fill_empty_by_default();

        if self.padding.is_none() {
            self.padding = Some(Default::default());
        }

        if self.border.is_none() {
            self.border = Some(Default::default());
        }
        self.border.as_mut().unwrap().fill_empty_by_default();

        if self.colors.is_none() {
            self.colors = Some(Default::default());
        }
        self.colors.as_mut().unwrap().fill_empty_by_default();

        if self.title.is_none() {
            self.title = Some(Default::default());
        }
        self.title.as_mut().unwrap().fill_empty_by_default("title");

        if self.body.is_none() {
            self.body = Some(Default::default());
        }
        self.body.as_mut().unwrap().fill_empty_by_default("body");

        if self.ellipsize_at.is_none() {
            self.ellipsize_at = Some(Default::default());
        }

        if self.markup.is_none() {
            self.markup = Some(true);
        }

        if self.timeout.is_none() {
            self.timeout = Some(0);
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ImageProperty {
    max_size: Option<u16>,
    margin: Option<Spacing>,
    resizing_method: Option<ResizingMethod>,
}

impl ImageProperty {
    pub fn max_size(&self) -> u16 {
        self.max_size.unwrap()
    }

    pub fn margin(&self) -> &Spacing {
        self.margin.as_ref().unwrap()
    }

    pub fn margin_mut(&mut self) -> &mut Spacing {
        self.margin.as_mut().unwrap()
    }

    pub fn resizing_method(&self) -> &ResizingMethod {
        self.resizing_method.as_ref().unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.max_size.is_none() {
            self.max_size = Some(64);
        }

        if self.margin.is_none() {
            self.margin = Some(Default::default());
        }

        if self.resizing_method.is_none() {
            self.resizing_method = Some(Default::default());
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum ResizingMethod {
    #[serde(rename = "nearest")]
    Nearest,
    #[serde(rename = "triangle")]
    Triangle,
    #[serde(rename = "catmull-rom")]
    CatmullRom,
    #[default]
    #[serde(rename = "gaussian")]
    Gaussian,
    #[serde(rename = "lanczos3")]
    Lanczos3,
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

    pub fn by_urgency(&self, urgency: &Urgency) -> &Colors {
        match urgency {
            Urgency::Low => self.low(),
            Urgency::Normal => self.normal(),
            Urgency::Critical => self.critical(),
        }
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

impl Color {
    fn pre_mul_alpha(self) -> Self {
        if self.alpha == 255 {
            return self;
        }

        let alpha = self.alpha as f32 / 255.0;
        Self {
            red: (self.red as f32 * alpha) as u8,
            green: (self.green as f32 * alpha) as u8,
            blue: (self.blue as f32 * alpha) as u8,
            alpha: self.alpha,
        }
    }
}

impl From<String> for Color {
    fn from(value: String) -> Self {
        const BASE: u32 = 16;

        if value.len() == 4 {
            let mut chars = value.chars();
            chars.next(); // Skip the hashtag
            let next_digit = |chars: &mut Chars| {
                let digit = chars.next().unwrap().to_digit(BASE).unwrap() as u8;
                digit * BASE as u8 + digit
            };

            Color {
                red: next_digit(&mut chars),
                green: next_digit(&mut chars),
                blue: next_digit(&mut chars),
                alpha: 255,
            }
        } else {
            let data = &value[1..];
            Color {
                red: u8::from_str_radix(&data[0..2], BASE).unwrap(),
                green: u8::from_str_radix(&data[2..4], BASE).unwrap(),
                blue: u8::from_str_radix(&data[4..6], BASE).unwrap(),
                alpha: if data.len() == 8 {
                    u8::from_str_radix(&data[6..8], BASE).unwrap()
                } else {
                    255
                },
            }
            .pre_mul_alpha()
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Border {
    size: Option<u8>,
    radius: Option<u8>,
    color: Option<Color>,
}

impl Border {
    pub fn size(&self) -> u8 {
        self.size.unwrap()
    }

    pub fn radius(&self) -> u8 {
        self.radius.unwrap()
    }

    pub fn color(&self) -> &Color {
        self.color.as_ref().unwrap()
    }

    fn fill_empty_by_default(&mut self) {
        if self.size.is_none() {
            self.size = Some(0);
        }

        if self.radius.is_none() {
            self.radius = Some(0);
        }

        if self.color.is_none() {
            self.color = Some(Default::default());
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct TextProperty {
    wrap: Option<bool>,
    style: Option<TextStyle>,
    margin: Option<Spacing>,
    justification: Option<TextJustification>,
    line_spacing: Option<u8>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Alignment {
    horizontal: Option<Position>,
    vertical: Option<Position>,
}

impl Alignment {
    pub fn new(horizontal: Position, vertical: Position) -> Self {
        Self {
            horizontal: Some(horizontal),
            vertical: Some(vertical),
        }
    }

    pub fn horizontal(&self) -> &Position {
        self.horizontal.as_ref().unwrap()
    }

    pub fn vertical(&self) -> &Position {
        self.vertical.as_ref().unwrap()
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum Position {
    #[serde(rename = "start")]
    Start,
    #[default]
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "end")]
    End,
    #[serde(rename = "space-between")]
    SpaceBetween,
}

impl Position {
    pub fn compute_initial_pos(&self, width: usize, element_width: usize) -> usize {
        match self {
            Position::Start | Position::SpaceBetween => 0,
            Position::Center => width / 2 - element_width / 2,
            Position::End => width - element_width,
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum TextStyle {
    #[default]
    #[serde(rename = "regular")]
    Regular,
    #[serde(rename = "bold")]
    Bold,
    #[serde(rename = "italic")]
    Italic,
    #[serde(rename = "bold italic")]
    BoldItalic,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum TextJustification {
    #[serde(rename = "center")]
    Center,
    #[default]
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "space-between")]
    SpaceBetween,
}

impl TextProperty {
    pub fn wrap(&self) -> bool {
        self.wrap.unwrap()
    }

    pub fn style(&self) -> &TextStyle {
        self.style.as_ref().unwrap()
    }

    pub fn margin(&self) -> &Spacing {
        self.margin.as_ref().unwrap()
    }

    pub fn justification(&self) -> &TextJustification {
        self.justification.as_ref().unwrap()
    }

    pub fn line_spacing(&self) -> u8 {
        self.line_spacing.unwrap()
    }

    pub fn fill_empty_by_default(&mut self, entity: &str) {
        fn is_title(entity: &str) -> bool {
            entity == "title"
        }

        if self.wrap.is_none() {
            self.wrap = Some(true);
        }

        if self.style.is_none() {
            if is_title(entity) {
                self.style = Some(TextStyle::Bold);
            } else {
                self.style = Some(Default::default());
            }
        }

        if self.margin.is_none() {
            self.margin = Some(Default::default());
        }

        if self.justification.is_none() {
            if is_title(entity) {
                self.justification = Some(TextJustification::Center);
            } else {
                self.justification = Some(Default::default());
            }
        }

        if self.line_spacing.is_none() {
            self.line_spacing = Some(0);
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum EllipsizeAt {
    #[serde(rename = "middle")]
    Middle,
    #[default]
    #[serde(rename = "end")]
    End,
}

#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    pub name: String,
    pub display: Option<DisplayConfig>,
}

impl AppConfig {
    fn merge(&mut self, other: &DisplayConfig) {
        if let Some(display) = self.display.as_mut() {
            if let Some(image) = display.image.as_mut() {
                let other_image = other.image();

                image.max_size = image.max_size.or(other_image.max_size);
                image.margin = image.margin.clone().or(other_image.margin.clone());
                image.resizing_method = image
                    .resizing_method
                    .clone()
                    .or(other_image.resizing_method.clone());
            } else {
                display.image = other.image.clone();
            }

            display.padding = display.padding.clone().or(other.padding.clone());

            if let Some(border) = display.border.as_mut() {
                let other_border = other.border(); // The other type shall have border

                border.size = border.size.or(other_border.size);
                border.radius = border.radius.or(other_border.radius);
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

            if let Some(title) = display.title.as_mut() {
                let other_title = other.title();

                title.wrap = title.wrap.or(other_title.wrap);
                title.style = title.style.clone().or(other_title.style.clone());
                title.margin = title.margin.clone().or(other_title.margin.clone());
                title.justification = title
                    .justification
                    .clone()
                    .or(other_title.justification.clone());
                title.line_spacing = title.line_spacing.or(other_title.line_spacing);
            } else {
                display.title = other.title.clone();
            }

            if let Some(body) = display.body.as_mut() {
                let other_body = other.body();

                body.wrap = body.wrap.or(other_body.wrap);
                body.style = body.style.clone().or(other_body.style.clone());
                body.margin = body.margin.clone().or(other_body.margin.clone());
                body.justification = body
                    .justification
                    .clone()
                    .or(other_body.justification.clone());
                body.line_spacing = body.line_spacing.or(other_body.line_spacing);
            } else {
                display.body = other.body.clone();
            }

            display.ellipsize_at = display.ellipsize_at.clone().or(other.ellipsize_at.clone());
            display.markup = display.markup.or(other.markup);
            display.timeout = display.timeout.or(other.timeout);
        } else {
            self.display = Some(other.clone());
        }
    }
}
