use derive_builder::Builder;

use config::{spacing::Spacing, text::TextProperty, DisplayConfig, ImageProperty};
use dbus::notification::Notification;

use super::{
    color::Bgra,
    font::FontCollection,
    image::Image,
    text::TextRect,
    types::{Offset, RectSize},
};
#[derive(Clone)]
pub(super) struct Coverage(pub(super) f32);

#[derive(Clone)]
pub(super) enum DrawColor {
    Replace(Bgra),
    Overlay(Bgra),
    OverlayWithCoverage(Bgra, Coverage),
}

pub(super) trait Draw {
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    );

    fn draw<Output: FnMut(usize, usize, DrawColor)>(&self, output: &mut Output) {
        self.draw_with_offset(&Default::default(), output);
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub(super) struct Container {
    #[builder(private, setter(skip))]
    rect_size: Option<RectSize>,

    spacing: Spacing,

    direction: Direction,
    alignment: Alignment,

    elements: Vec<Widget>,
}

impl Container {
    pub(super) fn compile(&mut self, mut rect_size: RectSize) -> CompileState {
        self.rect_size = Some(rect_size.clone());

        rect_size.shrink_by(&self.spacing);
        let mut container_axis = ContainerPlane::new(rect_size, &self.direction);

        self.elements.iter_mut().for_each(|element| {
            element.compile(container_axis.as_rect_size());

            container_axis.main_len -= element.len_by_direction(&self.direction);
        });
        self.elements.retain(|element| !element.is_unknown());

        CompileState::Success
    }

    pub(super) fn width(&self) -> usize {
        let widths = self.elements.iter().map(|element| element.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.max().unwrap_or_default(),
        }
    }

    pub(super) fn height(&self) -> usize {
        let heights = self.elements.iter().map(|element| element.height());

        match self.direction {
            Direction::Horizontal => heights.max().unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    fn main_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    #[allow(dead_code)]
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

impl Draw for Container {
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        assert!(
            self.rect_size.is_some(),
            "The rectangle size must be computed by `compile()` methot of parent container."
        );

        let mut plane = ContainerPlane::new(
            unsafe { self.rect_size.as_ref().cloned().unwrap_unchecked() },
            &self.direction,
        );

        let initial_plane = ContainerPlane::new_only_offset(
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
                if self.elements.len() == 1 {
                    0
                } else {
                    (plane.main_len - self.main_len()) / self.elements.len().saturating_sub(1)
                }
            }
        };

        self.elements.iter().for_each(|element| {
            if !element.is_container() {
                plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset
                    + self.auxiliary_axis_alignment().compute_initial_pos(
                        plane.auxiliary_len,
                        element.len_by_direction(&self.direction.orthogonalize()),
                    );
            }

            element.draw_with_offset(&plane.as_offset(), output);

            plane.main_axis_offset += element.len_by_direction(&self.direction) + incrementor;
            plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset;
        });
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct Alignment {
    pub(super) horizontal: Position,
    pub(super) vertical: Position,
}

impl Alignment {
    pub(super) fn new(horizontal: Position, vertical: Position) -> Self {
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

pub(super) enum Direction {
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

struct ContainerPlane<'a> {
    main_len: usize,
    auxiliary_len: usize,

    main_axis_offset: usize,
    auxiliary_axis_offset: usize,

    direction: &'a Direction,
}

impl<'a> ContainerPlane<'a> {
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

pub(super) enum Widget {
    Image(WImage),
    Text(WText),
    Container(Container),
    Unknown,
}

impl Widget {
    pub(super) fn is_container(&self) -> bool {
        if let Widget::Container(_) = self {
            true
        } else {
            false
        }
    }

    pub(super) fn is_unknown(&self) -> bool {
        if let Widget::Unknown = self {
            true
        } else {
            false
        }
    }

    pub(super) fn compile(&mut self, rect_size: RectSize) {
        let state = match self {
            Widget::Image(image) => image.compile(rect_size),
            Widget::Text(text) => text.compile(rect_size),
            Widget::Container(container) => container.compile(rect_size),
            Widget::Unknown => CompileState::Success,
        };

        if let CompileState::Failure = state {
            *self = Widget::Unknown;
        }
    }

    pub(super) fn len_by_direction(&self, direction: &Direction) -> usize {
        match direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    pub(super) fn width(&self) -> usize {
        match self {
            Widget::Image(image) => image.width(),
            Widget::Text(text) => text.width(),
            Widget::Container(container) => container.width(),
            Widget::Unknown => 0,
        }
    }

    pub(super) fn height(&self) -> usize {
        match self {
            Widget::Image(image) => image.height(),
            Widget::Text(text) => text.height(),
            Widget::Container(container) => container.height(),
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
            Widget::Container(container) => container.draw_with_offset(offset, output),
            Widget::Unknown => (),
        }
    }
}

pub(super) enum CompileState {
    Success,
    Failure,
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

impl From<Container> for Widget {
    fn from(value: Container) -> Self {
        Widget::Container(value)
    }
}

pub(super) struct WImage {
    data: Image,

    rect_size: Option<RectSize>,
    property: ImageProperty,
}

impl WImage {
    pub(super) fn new(notification: &Notification, display_config: &DisplayConfig) -> Self {
        let property = display_config.image.clone();
        let image = Image::from_image_data(notification.hints.image_data.as_ref(), &property)
            .or_svg(
                notification
                    .hints
                    .image_path
                    .as_deref()
                    .or(Some(notification.app_icon.as_str())),
                &property,
            );

        Self {
            data: image,
            rect_size: None,
            property,
        }
    }

    pub(super) fn compile(&mut self, rect_size: RectSize) -> CompileState {
        fn reduce_spacing(first: &mut u8, second: &mut u8, diff: u8) {
            let mut swapped = false;
            if first > second {
                std::mem::swap(first, second);
                swapped = true;
            }

            if diff > *first {
                *second -= diff - *first;
            }

            *first = first.saturating_sub(diff);
            *second = second.saturating_sub(diff);

            if swapped {
                std::mem::swap(first, second);
            }
        }

        if !self.data.exists() {
            return CompileState::Failure;
        }

        let image_width = unsafe { self.data.width().unwrap_unchecked() };

        if image_width > rect_size.width {
            eprintln!("Image width exceeds the possbile width!");
            return CompileState::Failure;
        }

        let image_height = unsafe { self.data.height().unwrap_unchecked() };

        if image_height > rect_size.height {
            eprintln!("Image height exceeds the possbile height!");
            return CompileState::Failure;
        }

        let margin = &mut self.property.margin;
        let horizontal_margin = (margin.left() + margin.right()) as usize;

        if image_width + horizontal_margin > rect_size.width {
            let diff = (image_width + horizontal_margin - rect_size.width) as u8 / 2;

            let (mut left, mut right) = (margin.left(), margin.right());
            reduce_spacing(&mut left, &mut right, diff);

            margin.set_left(left);
            margin.set_right(right);
        }

        let vertical_margin = (margin.top() + margin.bottom()) as usize;
        if image_height + vertical_margin > rect_size.height {
            let diff = (image_height + vertical_margin - rect_size.height) as u8 / 2;

            let (mut top, mut bottom) = (margin.top(), margin.bottom());
            reduce_spacing(&mut top, &mut bottom, diff);

            margin.set_top(top);
            margin.set_bottom(bottom);
        }

        self.rect_size = Some(RectSize::new(
            image_width + margin.left() as usize + margin.right() as usize,
            image_height + margin.top() as usize + margin.bottom() as usize,
        ));

        CompileState::Success
    }

    pub(super) fn width(&self) -> usize {
        assert!(self.rect_size.is_some());
        unsafe { self.rect_size.as_ref().unwrap_unchecked() }.width
    }

    pub(super) fn height(&self) -> usize {
        assert!(self.rect_size.is_some());
        unsafe { self.rect_size.as_ref().unwrap_unchecked() }.height
    }
}

impl Draw for WImage {
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        let offset = Offset::from(&self.property.margin) + offset.clone();
        self.data.draw_with_offset(&offset, output);
    }
}

pub(super) struct WText {
    data: TextRect,
    #[allow(dead_code)]
    property: TextProperty,
}

impl WText {
    pub(super) fn new_title(
        notification: &Notification,
        font_collection: &FontCollection,
        font_size: f32,
        display_config: &DisplayConfig,
    ) -> Self {
        let colors = display_config
            .colors
            .by_urgency(&notification.hints.urgency);
        let foreground = Bgra::from(&colors.foreground);

        let title_cfg = display_config.title.clone();
        let mut summary = TextRect::from_str(
            &notification.summary,
            font_size,
            &title_cfg.style,
            font_collection,
        );

        Self::apply_properties(&mut summary, &title_cfg);
        Self::apply_color(&mut summary, foreground);

        Self {
            data: summary,
            property: title_cfg,
        }
    }

    pub(super) fn new_body(
        notification: &Notification,
        font_collection: &FontCollection,
        font_size: f32,
        display_config: &DisplayConfig,
    ) -> Self {
        let colors = display_config
            .colors
            .by_urgency(&notification.hints.urgency);
        let foreground = Bgra::from(&colors.foreground);

        let body_cfg = display_config.body.clone();
        let mut body = if display_config.markup {
            TextRect::from_text(
                &notification.body,
                font_size,
                &body_cfg.style,
                font_collection,
            )
        } else {
            TextRect::from_str(
                &notification.body.body,
                font_size,
                &body_cfg.style,
                font_collection,
            )
        };

        Self::apply_properties(&mut body, &body_cfg);
        Self::apply_color(&mut body, foreground);

        Self {
            data: body,
            property: body_cfg,
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

    pub(super) fn compile(&mut self, rect_size: RectSize) -> CompileState {
        self.data.compile(rect_size);
        CompileState::Success
    }

    pub(super) fn width(&self) -> usize {
        self.data.width()
    }

    pub(super) fn height(&self) -> usize {
        self.data.height()
    }
}

impl Draw for WText {
    fn draw_with_offset<Output: FnMut(usize, usize, DrawColor)>(
        &self,
        offset: &Offset,
        output: &mut Output,
    ) {
        self.data.draw_with_offset(offset, output)
    }
}
