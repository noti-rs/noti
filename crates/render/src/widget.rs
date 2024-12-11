use derive_builder::Builder;

use config::{spacing::Spacing, text::TextProperty, Border, DisplayConfig, ImageProperty};
use dbus::{notification::Notification, text::Text};
use log::warn;
use shared::{
    error::ConversionError,
    value::{TryDowncast, Value},
};

use crate::{border::BorderBuilder, drawer::Drawer};

use super::{
    border::Border as RenderableBorder,
    color::Bgra,
    font::FontCollection,
    image::Image,
    text::TextRect,
    types::{Offset, RectSize},
};
#[derive(Clone)]
pub struct Coverage(pub f32);

#[derive(Clone)]
pub enum DrawColor {
    Replace(Bgra),
    Overlay(Bgra),
    OverlayWithCoverage(Bgra, Coverage),
    Transparent(Coverage),
}

pub trait Draw {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer);

    fn draw(&self, drawer: &mut Drawer) {
        self.draw_with_offset(&Default::default(), drawer);
    }
}

#[derive(Clone)]
pub enum Widget {
    Image(WImage),
    Text(WText),
    FlexContainer(FlexContainer),
    Unknown,
}

impl Widget {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Widget::Unknown)
    }

    fn get_type(&self) -> &'static str {
        match self {
            Widget::Image(_) => "image",
            Widget::Text(_) => "text",
            Widget::FlexContainer(_) => "flex container",
            Widget::Unknown => "unknown",
        }
    }

    pub fn compile(&mut self, rect_size: RectSize, configuration: &WidgetConfiguration) {
        let state = match self {
            Widget::Image(image) => image.compile(rect_size, configuration),
            Widget::Text(text) => text.compile(rect_size, configuration),
            Widget::FlexContainer(container) => container.compile(rect_size, configuration),
            Widget::Unknown => CompileState::Success,
        };

        if let CompileState::Failure = state {
            warn!(
                "A {wtype} widget is not compiled due errors!",
                wtype = self.get_type()
            );
            *self = Widget::Unknown;
        }
    }

    pub fn len_by_direction(&self, direction: &Direction) -> usize {
        match direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    pub fn width(&self) -> usize {
        match self {
            Widget::Image(image) => image.width(),
            Widget::Text(text) => text.width(),
            Widget::FlexContainer(container) => container.max_width(),
            Widget::Unknown => 0,
        }
    }

    pub fn height(&self) -> usize {
        match self {
            Widget::Image(image) => image.height(),
            Widget::Text(text) => text.height(),
            Widget::FlexContainer(container) => container.max_height(),
            Widget::Unknown => 0,
        }
    }
}

impl Draw for Widget {
    fn draw_with_offset(&self, offset: &Offset, output: &mut Drawer) {
        match self {
            Widget::Image(image) => image.draw_with_offset(offset, output),
            Widget::Text(text) => text.draw_with_offset(offset, output),
            Widget::FlexContainer(container) => container.draw_with_offset(offset, output),
            Widget::Unknown => (),
        }
    }
}

pub enum CompileState {
    Success,
    Failure,
}

pub struct WidgetConfiguration<'a> {
    pub notification: &'a Notification,
    pub font_collection: &'a FontCollection,
    pub font_size: f32,
    pub display_config: &'a DisplayConfig,
    pub override_properties: bool,
}

impl From<WImage> for Widget {
    fn from(value: WImage) -> Self {
        Widget::Image(value)
    }
}

impl From<WText> for Widget {
    fn from(value: WText) -> Self {
        Widget::Text(value)
    }
}

impl From<FlexContainer> for Widget {
    fn from(value: FlexContainer) -> Self {
        Widget::FlexContainer(value)
    }
}

#[derive(macros::GenericBuilder, Builder, Clone)]
#[builder(pattern = "owned")]
#[gbuilder(name(GBuilderFlexContainer))]
pub struct FlexContainer {
    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default(None))]
    rect_size: Option<RectSize>,

    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default(Bgra::new()))]
    background_color: Bgra,

    #[builder(default = "usize::MAX")]
    #[gbuilder(default(usize::MAX))]
    max_width: usize,

    #[builder(default = "usize::MAX")]
    #[gbuilder(default(usize::MAX))]
    max_height: usize,

    #[gbuilder(default)]
    spacing: Spacing,

    #[gbuilder(default)]
    border: Border,

    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default)]
    compiled_border: Option<RenderableBorder>,

    direction: Direction,
    alignment: Alignment,

    children: Vec<Widget>,
}

impl FlexContainer {
    pub fn compile(
        &mut self,
        mut rect_size: RectSize,
        configuration: &WidgetConfiguration,
    ) -> CompileState {
        self.max_width = self.max_width.min(rect_size.width);
        self.max_height = self.max_height.min(rect_size.height);
        rect_size = RectSize {
            width: self.max_width,
            height: self.max_height,
        };
        self.rect_size = Some(rect_size.clone());

        self.background_color = Bgra::from(
            &configuration
                .display_config
                .colors
                .by_urgency(&configuration.notification.hints.urgency)
                .background,
        );

        self.compiled_border = Some(
            BorderBuilder::default()
                .color((&self.border.color).into())
                .frame_width(rect_size.width)
                .frame_height(rect_size.height)
                .size(self.border.size)
                .radius(self.border.radius)
                .compile()
                .expect("Border should be have possibility to compile"),
        );

        rect_size.shrink_by(&(self.spacing.clone() + Spacing::all_directional(self.border.size)));
        let mut container_axes = FlexContainerPlane::new(rect_size, &self.direction);

        self.children.iter_mut().for_each(|child| {
            child.compile(container_axes.as_rect_size(), configuration);

            container_axes.main_len = container_axes
                .main_len
                .saturating_sub(child.len_by_direction(&self.direction));
        });
        self.children.retain(|child| !child.is_unknown());

        if self.children.is_empty() {
            warn!(
                "The flex container is empty! Did you add the widgets? \
                Or check them, maybe they doesn't fit available space."
            );
            CompileState::Failure
        } else {
            CompileState::Success
        }
    }

    pub(super) fn max_width(&self) -> usize {
        self.max_width
    }

    pub(super) fn max_height(&self) -> usize {
        self.max_height
    }

    pub fn width(&self) -> usize {
        let widths = self.children.iter().map(|child| child.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.max().unwrap_or_default(),
        }
    }

    pub fn height(&self) -> usize {
        let heights = self.children.iter().map(|child| child.height());

        match self.direction {
            Direction::Horizontal => heights.max().unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    #[allow(unused)]
    fn max_main_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.max_width(),
            Direction::Vertical => self.max_height(),
        }
    }

    #[allow(unused)]
    fn max_auxiliary_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.max_height(),
            Direction::Vertical => self.max_width(),
        }
    }

    fn main_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    #[allow(unused)]
    fn auxiliary_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.height(),
            Direction::Vertical => self.width(),
        }
    }

    fn main_axis_alignment(&self) -> &Position {
        match &self.direction {
            Direction::Horizontal => &self.alignment.horizontal,
            Direction::Vertical => &self.alignment.vertical,
        }
    }

    fn auxiliary_axis_alignment(&self) -> &Position {
        match &self.direction {
            Direction::Horizontal => &self.alignment.vertical,
            Direction::Vertical => &self.alignment.horizontal,
        }
    }
}

impl Draw for FlexContainer {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        let Some(mut rect_size) = self.rect_size.as_ref().cloned() else {
            panic!(
                "The rectangle size must be computed by `compile()` method of parent container!"
            );
        };

        let mut subdrawer = Drawer::new(self.background_color, rect_size.clone());

        rect_size.shrink_by(&(self.spacing.clone() + Spacing::all_directional(self.border.size)));
        let mut plane = FlexContainerPlane::new(rect_size, &self.direction);

        let initial_offset = Offset::new(
            self.spacing.left() as usize + self.border.size as usize,
            self.spacing.top() as usize + self.border.size as usize,
        );
        let initial_plane = FlexContainerPlane::new_only_offset(initial_offset, &self.direction);
        plane.relocate(&initial_plane.as_offset());

        plane.main_axis_offset += self
            .main_axis_alignment()
            .compute_initial_pos(plane.main_len, self.main_len());

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0
                } else {
                    (plane.main_len - self.main_len()) / self.children.len().saturating_sub(1)
                }
            }
        };

        self.children.iter().for_each(|child| {
            plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset
                + self.auxiliary_axis_alignment().compute_initial_pos(
                    plane.auxiliary_len,
                    child.len_by_direction(&self.direction.orthogonalize()),
                );

            child.draw_with_offset(&plane.as_offset(), &mut subdrawer);

            plane.main_axis_offset += child.len_by_direction(&self.direction) + incrementor;
            plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset;
        });

        if let Some(compiled_border) = self.compiled_border.as_ref() {
            compiled_border.draw_with_offset(&Offset::no_offset(), &mut subdrawer);
        }

        drawer.draw_area(offset, subdrawer);
    }
}

#[derive(macros::GenericBuilder, Debug, Default, Clone)]
#[gbuilder(name(GBuilderAlignment))]
pub struct Alignment {
    pub horizontal: Position,
    pub vertical: Position,
}

impl Alignment {
    pub fn new(horizontal: Position, vertical: Position) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

impl TryFrom<Value> for Alignment {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum Position {
    Start,
    #[default]
    Center,
    End,
    SpaceBetween,
}

impl TryFrom<Value> for Position {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "start" => Position::Start,
                "center" => Position::Center,
                "end" => Position::End,
                "space-between" | "space_between" => Position::SpaceBetween,
                _ => Err(ConversionError::InvalidValue {
                    expected: "start, center, end, space-between or space_between",
                    actual: str,
                })?,
            }),
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
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

#[derive(Clone)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    fn orthogonalize(&self) -> Direction {
        match self {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        }
    }
}

impl TryFrom<Value> for Direction {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(str) => Ok(match str.to_lowercase().as_str() {
                "horizontal" => Direction::Horizontal,
                "vertical" => Direction::Vertical,
                _ => Err(ConversionError::InvalidValue {
                    expected: "horizontal or vertical",
                    actual: str,
                })?,
            }),
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

struct FlexContainerPlane<'a> {
    main_len: usize,
    auxiliary_len: usize,

    main_axis_offset: usize,
    auxiliary_axis_offset: usize,

    direction: &'a Direction,
}

impl<'a> FlexContainerPlane<'a> {
    fn new(
        RectSize {
            mut width,
            mut height,
        }: RectSize,
        direction: &'a Direction,
    ) -> Self {
        if let Direction::Vertical = direction {
            (width, height) = (height, width);
        }

        Self {
            main_len: width,
            auxiliary_len: height,
            main_axis_offset: 0,
            auxiliary_axis_offset: 0,
            direction,
        }
    }

    fn new_only_offset(Offset { mut x, mut y }: Offset, direction: &'a Direction) -> Self {
        if let Direction::Vertical = direction {
            (x, y) = (y, x);
        }

        Self {
            main_len: 0,
            auxiliary_len: 0,
            main_axis_offset: x,
            auxiliary_axis_offset: y,
            direction,
        }
    }

    fn relocate(&mut self, Offset { mut x, mut y }: &Offset) {
        if let Direction::Vertical = self.direction {
            (x, y) = (y, x);
        }

        self.main_axis_offset = x;
        self.auxiliary_axis_offset = y;
    }

    fn as_rect_size(&self) -> RectSize {
        let (mut width, mut height) = (self.main_len, self.auxiliary_len);

        if let Direction::Vertical = self.direction {
            (width, height) = (height, width);
        }

        RectSize::new(width, height)
    }

    fn as_offset(&self) -> Offset {
        let (mut x, mut y) = (self.main_axis_offset, self.auxiliary_axis_offset);

        if let Direction::Vertical = self.direction {
            (x, y) = (y, x);
        }

        Offset::new(x, y)
    }
}

#[derive(macros::GenericBuilder, Clone)]
#[gbuilder(name(GBuilderWImage))]
pub struct WImage {
    #[gbuilder(hidden, default(Image::Unknown))]
    content: Image,

    #[gbuilder(hidden, default(0))]
    width: usize,
    #[gbuilder(hidden, default(0))]
    height: usize,

    #[gbuilder(default)]
    property: ImageProperty,
}

impl WImage {
    pub fn new() -> Self {
        Self {
            content: Image::Unknown,
            width: 0,
            height: 0,
            property: Default::default(),
        }
    }

    pub fn compile(
        &mut self,
        rect_size: RectSize,
        WidgetConfiguration {
            notification,
            display_config,
            override_properties,
            ..
        }: &WidgetConfiguration,
    ) -> CompileState {
        if *override_properties {
            self.property = display_config.image.clone();
        }

        self.content = notification
            .hints
            .image_data
            .as_ref()
            .cloned()
            .map(|image_data| Image::from_image_data(image_data, &self.property, &rect_size))
            .or_else(|| {
                notification
                    .hints
                    .image_path
                    .as_deref()
                    .or(Some(notification.app_icon.as_str()))
                    .map(|svg_path| Image::from_svg(svg_path, &self.property, &rect_size))
            })
            .unwrap_or(Image::Unknown);

        self.width = self
            .content
            .width()
            .map(|width| width + self.property.margin.horizontal() as usize)
            .unwrap_or(0);
        self.height = self
            .content
            .height()
            .map(|height| height + self.property.margin.vertical() as usize)
            .unwrap_or(0);

        if self.width > rect_size.width || self.height > rect_size.height {
            warn!(
                "The image doesn't fit to available space.\
                \nThe image size: width={}, height={}.\
                \nAvailable space: width={}, height={}.",
                self.width, self.height, rect_size.width, rect_size.height
            );
            return CompileState::Failure;
        }

        if self.content.is_exists() {
            CompileState::Success
        } else {
            CompileState::Failure
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

impl Default for WImage {
    fn default() -> Self {
        Self::new()
    }
}

impl Draw for WImage {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        if !self.content.is_exists() {
            return;
        }

        // INFO: The ImageProperty initializes with Image so we can calmly unwrap
        let offset = Offset::from(&self.property.margin) + offset.clone();
        self.content.draw_with_offset(&offset, drawer);
    }
}

#[derive(macros::GenericBuilder)]
#[gbuilder(name(GBuilderWText))]
pub struct WText {
    kind: WTextKind,
    #[gbuilder(hidden, default(None))]
    content: Option<TextRect>,

    #[gbuilder(default)]
    property: TextProperty,
}

impl Clone for WText {
    fn clone(&self) -> Self {
        // INFO: we shouldn't clone compiled info about text
        Self {
            kind: self.kind.clone(),
            content: None,
            property: self.property.clone(),
        }
    }
}

#[derive(Clone, derive_more::Display)]
pub enum WTextKind {
    #[display("title")]
    Title,
    #[display("body")]
    Body,
}

impl TryFrom<Value> for WTextKind {
    type Error = ConversionError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(val) => Ok(match val.to_lowercase().as_str() {
                "title" | "summary" => WTextKind::Title,
                "body" => WTextKind::Body,
                _ => Err(ConversionError::InvalidValue {
                    expected: "title or body",
                    actual: val,
                })?,
            }),
            Value::Any(dyn_value) => dyn_value.try_downcast(),
            _ => Err(ConversionError::CannotConvert),
        }
    }
}

impl WText {
    pub fn new(kind: WTextKind) -> Self {
        Self {
            kind,
            content: None,
            property: Default::default(),
        }
    }

    pub fn compile(
        &mut self,
        rect_size: RectSize,
        WidgetConfiguration {
            display_config,
            notification,
            font_size,
            font_collection,
            override_properties,
        }: &WidgetConfiguration,
    ) -> CompileState {
        let mut override_if = |r#override: bool, property: &TextProperty| {
            if r#override {
                self.property = property.clone()
            }
        };

        let colors = display_config
            .colors
            .by_urgency(&notification.hints.urgency);
        let foreground = Bgra::from(&colors.foreground);

        let notification_content: NotificationContent = match self.kind {
            WTextKind::Title => {
                override_if(*override_properties, &display_config.title);
                notification.summary.as_str().into()
            }
            WTextKind::Body => {
                override_if(*override_properties, &display_config.body);
                if display_config.markup {
                    (&notification.body).into()
                } else {
                    notification.body.body.as_str().into()
                }
            }
        };

        let mut content = match notification_content {
            NotificationContent::Text(text) => {
                TextRect::from_text(text, *font_size, &self.property.style, font_collection)
            }
            NotificationContent::String(str) => {
                TextRect::from_str(str, *font_size, &self.property.style, font_collection)
            }
        };

        Self::apply_properties(&mut content, &self.property);
        Self::apply_color(&mut content, foreground);

        content.compile(rect_size.clone());
        if content.is_empty() {
            warn!(
                "The text with kind {} doesn't fit to available space. \
                Available space: width={}, height={}.",
                self.kind, rect_size.width, rect_size.height
            );
            CompileState::Failure
        } else {
            self.content = Some(content);
            CompileState::Success
        }
    }

    fn apply_properties(element: &mut TextRect, properties: &TextProperty) {
        element.set_wrap(properties.wrap);
        element.set_margin(&properties.margin);
        element.set_line_spacing(properties.line_spacing as usize);
        element.set_ellipsize_at(&properties.ellipsize_at);
        element.set_justification(&properties.justification);
    }

    fn apply_color(element: &mut TextRect, foreground: Bgra) {
        element.set_foreground(foreground);
    }

    pub fn width(&self) -> usize {
        self.content
            .as_ref()
            .map(|content| content.width())
            .unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.content
            .as_ref()
            .map(|content| content.height())
            .unwrap_or(0)
    }
}

impl Draw for WText {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        if let Some(content) = self.content.as_ref() {
            content.draw_with_offset(offset, drawer)
        }
    }
}

enum NotificationContent<'a> {
    String(&'a str),
    Text(&'a Text),
}

impl<'a> From<&'a str> for NotificationContent<'a> {
    fn from(value: &'a str) -> Self {
        NotificationContent::String(value)
    }
}

impl<'a> From<&'a Text> for NotificationContent<'a> {
    fn from(value: &'a Text) -> Self {
        NotificationContent::Text(value)
    }
}
