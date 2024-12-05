use derive_builder::Builder;

use config::{spacing::Spacing, text::TextProperty, DisplayConfig, ImageProperty};
use dbus::{notification::Notification, text::Text};
use log::warn;

use super::{
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
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    );

    fn draw<Output: FnMut(usize, usize, DrawColor)>(&self, output: &mut Output) {
        self.draw_with_offset(&Default::default(), output);
    }
}

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
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
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

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct FlexContainer {
    #[builder(private, setter(skip))]
    rect_size: Option<RectSize>,

    #[builder(default = "usize::MAX")]
    max_width: usize,
    #[builder(default = "usize::MAX")]
    max_height: usize,

    spacing: Spacing,

    direction: Direction,
    alignment: Alignment,

    elements: Vec<Widget>,
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

        rect_size.shrink_by(&self.spacing);
        let mut container_axes = FlexContainerPlane::new(rect_size, &self.direction);

        self.elements.iter_mut().for_each(|element| {
            element.compile(container_axes.as_rect_size(), configuration);

            container_axes.main_len = container_axes
                .main_len
                .saturating_sub(element.len_by_direction(&self.direction));
        });
        self.elements.retain(|element| !element.is_unknown());

        if self.elements.is_empty() {
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
        let widths = self.elements.iter().map(|element| element.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.max().unwrap_or_default(),
        }
    }

    pub fn height(&self) -> usize {
        let heights = self.elements.iter().map(|element| element.height());

        match self.direction {
            Direction::Horizontal => heights.max().unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

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
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        assert!(
            self.rect_size.is_some(),
            "The rectangle size must be computed by `compile()` methot of parent container."
        );

        let mut plane = FlexContainerPlane::new(
            unsafe { self.rect_size.as_ref().cloned().unwrap_unchecked() },
            &self.direction,
        );

        let initial_plane = FlexContainerPlane::new_only_offset(
            Offset::from(&self.spacing) + offset.clone(),
            &self.direction,
        );
        plane.relocate(&initial_plane.as_offset());

        plane.main_axis_offset += self
            .main_axis_alignment()
            .compute_initial_pos(plane.main_len, self.main_len());

        plane.shrink_rect_size_by(&self.spacing);

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0,
            Position::SpaceBetween => {
                if self.elements.len() <= 1 {
                    0
                } else {
                    (plane.main_len - self.max_main_len()) / self.elements.len().saturating_sub(1)
                }
            }
        };

        self.elements.iter().for_each(|element| {
            plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset
                + self.auxiliary_axis_alignment().compute_initial_pos(
                    plane.auxiliary_len,
                    element.len_by_direction(&self.direction.orthogonalize()),
                );

            element.draw_with_offset(&plane.as_offset(), output);

            plane.main_axis_offset += element.len_by_direction(&self.direction) + incrementor;
            plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset;
        });
    }
}

#[derive(Debug, Default, Clone)]
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

#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub enum Position {
    Start,
    #[default]
    Center,
    End,
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

    fn shrink_rect_size_by(&mut self, spacing: &Spacing) {
        let (mut width, mut height) = (&mut self.main_len, &mut self.auxiliary_len);

        if let Direction::Vertical = self.direction {
            (width, height) = (height, width);
        }

        spacing.shrink(width, height);
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

pub struct WImage {
    content: Image,

    width: usize,
    height: usize,

    // TODO: make decision about deletion this line
    property: Option<ImageProperty>,
}

impl WImage {
    pub fn new() -> Self {
        Self {
            content: Image::Unknown,
            width: 0,
            height: 0,
            property: None,
        }
    }

    pub fn compile(
        &mut self,
        rect_size: RectSize,
        WidgetConfiguration {
            notification,
            display_config,
            ..
        }: &WidgetConfiguration,
    ) -> CompileState {
        let property = display_config.image.clone();
        self.content = notification
            .hints
            .image_data
            .as_ref()
            .cloned()
            .map(|image_data| Image::from_image_data(image_data, &property, &rect_size))
            .or_else(|| {
                notification
                    .hints
                    .image_path
                    .as_deref()
                    .or(Some(notification.app_icon.as_str()))
                    .map(|svg_path| Image::from_svg(svg_path, &property, &rect_size))
            })
            .unwrap_or(Image::Unknown);

        self.width = self
            .content
            .width()
            .map(|width| width + property.margin.horizontal() as usize)
            .unwrap_or(0);
        self.height = self
            .content
            .height()
            .map(|height| height + property.margin.vertical() as usize)
            .unwrap_or(0);

        self.property = Some(property);

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

impl Draw for WImage {
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        if !self.content.is_exists() {
            return;
        }

        // INFO: The ImageProperty initializes with Image so we can calmly unwrap
        let offset = Offset::from(&self.property.as_ref().unwrap().margin) + offset.clone();
        self.content.draw_with_offset(&offset, output);
    }
}

pub struct WText {
    kind: WTextKind,
    content: Option<TextRect>,

    //TODO: make decision about deletion this field
    #[allow(dead_code)]
    property: Option<TextProperty>,
}

pub enum WTextKind {
    Title,
    Body,
}

impl WText {
    pub fn new(kind: WTextKind) -> Self {
        Self {
            kind,
            content: None,
            property: None,
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
        }: &WidgetConfiguration,
    ) -> CompileState {
        let colors = display_config
            .colors
            .by_urgency(&notification.hints.urgency);
        let foreground = Bgra::from(&colors.foreground);

        let (text_cfg, notification_content): (TextProperty, NotificationContent) = match self.kind
        {
            WTextKind::Title => (
                display_config.title.clone(),
                notification.summary.as_str().into(),
            ),
            WTextKind::Body => (
                display_config.body.clone(),
                if display_config.markup {
                    (&notification.body).into()
                } else {
                    notification.body.body.as_str().into()
                },
            ),
        };

        let mut content = match notification_content {
            NotificationContent::Text(text) => {
                TextRect::from_text(text, *font_size, &text_cfg.style, font_collection)
            }
            NotificationContent::String(str) => {
                TextRect::from_str(str, *font_size, &text_cfg.style, font_collection)
            }
        };

        Self::apply_properties(&mut content, &text_cfg);
        Self::apply_color(&mut content, foreground);

        content.compile(rect_size);
        if content.is_empty() {
            CompileState::Failure
        } else {
            self.content = Some(content);
            self.property = Some(text_cfg);
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
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        if let Some(content) = self.content.as_ref() {
            content.draw_with_offset(offset, output)
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
