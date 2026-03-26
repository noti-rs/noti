use config::spacing::Spacing;
use log::warn;
use shared::{error::ConversionError, value::TryFromValue};

use crate::{
    color::{Bgra, Color},
    drawer::{Drawer, UseColor},
    events::{Action, DispatchEvent, Event, Point},
    types::{Offset, RectSize},
    CompileState, Draw, Widget, WidgetConfiguration,
};

/// A container widget that arranges its child widgets along a single
/// axis, inspired by CSS flexbox layout but with a simpler and more
/// predictable implementation.
///
/// The container is responsible for:
/// * Compiling each child widget and determining its size.
/// * Shrinking the available space as children are placed.
/// * Arranging children according to the configured [`Alignment`]
///   on both the main and cross axis.
///
/// This type is intentionally lightweight — it does not implement the
/// full CSS flexbox algorithm, but provides enough flexibility to build
/// common notification layouts without duplicating positioning logic.
#[derive(macros::GenericBuilder, derive_builder::Builder, Clone)]
#[builder(pattern = "owned")]
#[gbuilder(name(GBuilderFlexContainer), derive(Clone))]
pub struct FlexContainer {
    /// The computed size of the container after compilation.
    ///
    /// `None` means the container has not been compiled yet, so its
    /// dimensions are unknown. After a successful call to [`Self::compile`],
    /// this will contain the resolved [`RectSize`].
    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default(None))]
    rect_size: Option<RectSize<usize>>,

    #[builder(private, default)]
    #[gbuilder(hidden, default(Bgra::default().into()))]
    background_color: Color,

    #[builder(private, default)]
    #[gbuilder(hidden, default(Bgra::default().into()))]
    border_color: Color,

    #[builder(default = "false")]
    #[gbuilder(default(false))]
    transparent_background: bool,

    #[builder(default = "usize::MAX")]
    #[gbuilder(default(usize::MAX))]
    max_width: usize,

    #[builder(default = "usize::MAX")]
    #[gbuilder(default(usize::MAX))]
    max_height: usize,

    #[gbuilder(default)]
    spacing: Spacing,

    #[gbuilder(default)]
    border: config::display::Border,

    direction: Direction,
    alignment: Alignment,

    children: Vec<Widget>,
}

impl FlexContainer {
    /// Compiles the container and its children, determining the final size
    /// and layout arrangement.
    ///
    /// This method:
    /// 1. Iterates over all children and calls their `compile` methods,
    ///    passing them the remaining available space.
    /// 2. Shrinks the available space after each successful child
    ///    compilation.
    /// 3. Stores the resolved container size in `rect_size` so that future
    ///    layout operations can use it.
    ///
    /// Callers **must** invoke `compile` before calling [`Self::width`], [`Self::height`],
    /// or attempting to draw the container, since layout information is
    /// unavailable until compilation is complete.
    pub fn compile(
        &mut self,
        mut rect_size: RectSize<usize>,
        configuration: &WidgetConfiguration,
    ) -> CompileState {
        self.max_width = self.max_width.min(rect_size.width);
        self.max_height = self.max_height.min(rect_size.height);
        rect_size = RectSize {
            width: self.max_width,
            height: self.max_height,
        };
        self.rect_size = Some(rect_size);

        let colors = &configuration
            .theme
            .by_urgency(&configuration.notification.hints.urgency);

        self.background_color = colors.background.clone().into();
        self.border_color = colors.border.clone().into();

        let mut plane = self.get_plane();

        self.children.iter_mut().for_each(|child| {
            child.compile(plane.as_rect_size(), configuration);

            plane.saturating_cut_front(child.len_by_direction(&self.direction));
        });
        self.children.retain(|child| !child.is_unknown());

        if self.children.is_empty() {
            warn!(
                "The flex container is empty! Did you add the widgets? \
                Or check them, maybe they doesn't fit available space."
            );
        }

        CompileState::Success
    }

    pub(crate) fn max_width(&self) -> usize {
        self.max_width
    }

    pub(crate) fn max_height(&self) -> usize {
        self.max_height
    }

    pub fn actual_width(&self) -> usize {
        let widths = self.children.iter().map(|child| child.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.max().unwrap_or_default(),
        }
    }

    pub fn actual_height(&self) -> usize {
        let heights = self.children.iter().map(|child| child.height());

        match self.direction {
            Direction::Horizontal => heights.max().unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    #[allow(unused)]
    fn max_main_extent(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.max_width(),
            Direction::Vertical => self.max_height(),
        }
    }

    #[allow(unused)]
    fn max_cross_extent(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.max_height(),
            Direction::Vertical => self.max_width(),
        }
    }

    fn main_actual_extent(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.actual_width(),
            Direction::Vertical => self.actual_height(),
        }
    }

    #[allow(unused)]
    fn cross_actual_extent(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.actual_height(),
            Direction::Vertical => self.actual_width(),
        }
    }

    fn main_axis_alignment(&self) -> &Position {
        match &self.direction {
            Direction::Horizontal => &self.alignment.horizontal,
            Direction::Vertical => &self.alignment.vertical,
        }
    }

    fn cross_axis_alignment(&self) -> &Position {
        match &self.direction {
            Direction::Horizontal => &self.alignment.vertical,
            Direction::Vertical => &self.alignment.horizontal,
        }
    }

    fn get_plane(&self) -> FCPlane {
        let Some(mut rect_size) = self.rect_size.as_ref().cloned() else {
            panic!(
                "The rectangle size must be computed by `compile()` method of parent container!"
            );
        };

        let inner_spacing = self.spacing + Spacing::all_directional(self.border.size);
        rect_size.shrink_by(&inner_spacing);

        FCPlane::new(inner_spacing, rect_size, self.direction)
    }

    fn get_start_and_incrementor(&self, restricted_extent: usize) -> (usize, usize) {
        let start = self
            .main_axis_alignment()
            .get_start(restricted_extent, self.main_actual_extent());

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0
                } else {
                    (restricted_extent - self.main_actual_extent())
                        / self.children.len().saturating_sub(1)
                }
            }
        };

        (start, incrementor)
    }

    /// Fills the container’s background before drawing children.
    ///
    /// If configured, the background will be filled with rounded corners.
    /// This is the first step of the `draw` routine, ensuring that child
    /// widgets are rendered on top of a consistent background.
    fn fill_background(&self, offset: Offset<f32>, rect_size: RectSize<f32>, drawer: &mut Drawer) {
        let outer_radius = (self.border.radius as f32)
            .min(rect_size.width / 2.0)
            .min(rect_size.height / 2.0);
        let inner_radius = (outer_radius - self.border.size as f32).max(0.0);
        let difference = self.border.size as f32;

        let canvas = drawer.surface.canvas();

        let rounded_rect = skia_safe::RRect::new_rect_xy(
            skia_safe::Rect::from_xywh(
                offset.x + difference,
                offset.y + difference,
                rect_size.width - difference * 2.0,
                rect_size.height - difference * 2.0,
            ),
            inner_radius,
            inner_radius,
        );

        let mut paint = skia_safe::Paint::default();
        paint.use_color(&self.background_color, offset, rect_size);
        paint.set_anti_alias(true);

        canvas.draw_rrect(rounded_rect, &paint);
    }

    /// Draws the container’s border after all children have been rendered.
    ///
    /// This is the final step of the `draw` routine, allowing the border
    /// to visually wrap around both the background and the children.
    fn outline_border(&self, offset: Offset<f32>, rect_size: RectSize<f32>, drawer: &mut Drawer) {
        if self.border.size == 0 {
            return;
        }

        let outer_radius = (self.border.radius as f32)
            .min(rect_size.width / 2.0)
            .min(rect_size.height / 2.0);
        let inner_radius = outer_radius - self.border.size as f32;

        let mut path = skia_safe::Path::new();

        path.add_rrect(
            skia_safe::RRect::new_rect_xy(
                skia_safe::Rect::from_xywh(offset.x, offset.y, rect_size.width, rect_size.height),
                outer_radius,
                outer_radius,
            ),
            None,
        );

        let border_size = self.border.size as f32;
        let base_rect = skia_safe::Rect::from_xywh(
            offset.x + border_size,
            offset.y + border_size,
            rect_size.width - border_size * 2.0,
            rect_size.height - border_size * 2.0,
        );
        if inner_radius <= 0.0 {
            path.add_rect(base_rect, None);
        } else {
            path.add_rrect(
                skia_safe::RRect::new_rect_xy(base_rect, inner_radius, inner_radius),
                None,
            );
        }

        let mut paint = skia_safe::Paint::default();
        paint.use_color(&self.border_color, offset, rect_size);
        paint.set_anti_alias(true);

        path.set_fill_type(skia_safe::path::FillType::EvenOdd);
        drawer.surface.canvas().draw_path(&path, &paint);
    }
}

impl Draw for FlexContainer {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let Some(rect_size) = self.rect_size.as_ref().cloned() else {
            panic!(
                "The rectangle size must be computed by `compile()` method of parent container!"
            );
        };

        let transparent_bg = self.transparent_background || self.background_color.is_transparent();
        if !transparent_bg {
            self.fill_background((*offset).into(), rect_size.into(), drawer);
        }

        let mut plane = self.get_plane();
        let (start, incrementor) = self.get_start_and_incrementor(plane.main.extent);
        plane.main.start += start;

        let cross_axis_start = plane.cross.start;
        let cross_axis_alignment = self.cross_axis_alignment();
        for child in &self.children {
            plane.cross.start = cross_axis_start
                + cross_axis_alignment.get_start(
                    plane.cross.extent,
                    child.len_by_direction(&self.direction.orthogonalize()),
                );

            child.draw_with_offset(&(plane.as_offset() + *offset), drawer);

            plane.saturating_cut_front(child.len_by_direction(&self.direction) + incrementor);
        }

        self.outline_border((*offset).into(), rect_size.into(), drawer);
    }
}

impl DispatchEvent for FlexContainer {
    fn dispatch_event(&self, event: Event) -> Action {
        let Some(rect_size) = self.rect_size.as_ref().cloned() else {
            panic!(
                "The rectangle size must be computed by `compile()` method of parent container!"
            );
        };

        if !event.kind.is_mouse() {
            return Action::None;
        }

        if event.local_coord.x > rect_size.width || event.local_coord.y > rect_size.height {
            return Action::None;
        }

        let (mouse_main, mouse_cross) = if let Direction::Horizontal = self.direction {
            (event.local_coord.x, event.local_coord.y)
        } else {
            (event.local_coord.y, event.local_coord.x)
        };

        let mut plane = self.get_plane();
        let (start, incrementor) = self.get_start_and_incrementor(plane.main.extent);
        plane.main.start += start;

        if mouse_main < plane.main.start || mouse_cross < plane.cross.start {
            return Action::None;
        }

        for child in &self.children {
            let child_main_len = child.len_by_direction(&self.direction);

            if mouse_main <= plane.main.start + child_main_len {
                let child_cross_len = child.len_by_direction(&self.direction.orthogonalize());
                let widget_start = self
                    .cross_axis_alignment()
                    .get_start(plane.cross.extent, child_cross_len);

                if mouse_cross >= plane.cross.start + widget_start
                    && mouse_cross <= plane.cross.start + widget_start + child_cross_len
                {
                    let local_mouse_main = mouse_main - plane.main.start;
                    let local_mouse_cross = mouse_cross - (plane.cross.start + widget_start);
                    let (local_mouse_x, local_mouse_y) =
                        if let Direction::Horizontal = self.direction {
                            (local_mouse_main, local_mouse_cross)
                        } else {
                            (local_mouse_cross, local_mouse_main)
                        };

                    let mut modified_event = event.clone();
                    modified_event.local_coord = Point {
                        x: local_mouse_x,
                        y: local_mouse_y,
                    };
                    return child.dispatch_event(modified_event);
                }

                return Action::None;
            }

            plane.saturating_cut_front(child_main_len + incrementor);
        }
        todo!()
    }
}

/// Defines how children are placed inside a [`FlexContainer`].
///
/// [`Self::horizontal`] controls alignment along the x-axis, and
/// [`Self::vertical`] along the y-axis. These values affect the
/// final placement of child widgets when there is extra free space
/// remaining after compilation.
#[derive(macros::GenericBuilder, Debug, Default, Clone)]
#[gbuilder(name(GBuilderAlignment), derive(Clone), constructor)]
pub struct Alignment {
    #[gbuilder(aliases(diagonal))]
    pub horizontal: Position,

    #[gbuilder(aliases(diagonal))]
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

impl TryFromValue for Alignment {}

/// The position strategy used by [`Alignment`] to place children.
#[derive(Debug, Default, Clone)]
pub enum Position {
    /// Aligns children at the start of the axis.
    Start,

    /// Centers children (default).
    #[default]
    Center,

    /// Aligns children at the end of the axis.
    End,

    /// Distributes children evenly, adding spacing between them to fill available space.
    SpaceBetween,
}

impl TryFromValue for Position {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "start" => Position::Start,
            "center" => Position::Center,
            "end" => Position::End,
            "space-between" | "space_between" => Position::SpaceBetween,
            _ => Err(ConversionError::InvalidValue {
                expected: "start, center, end, space-between or space_between",
                actual: value,
            })?,
        })
    }
}

impl Position {
    /// Computes the starting offset for an element of a given width
    /// relative to the available space, based on the positioning strategy.
    ///
    /// This is the core helper for placing children at `Start`, `Center`,
    /// `End`, or evenly with `SpaceBetween`.
    pub fn get_start(&self, width: usize, element_width: usize) -> usize {
        match self {
            Position::Start | Position::SpaceBetween => 0,
            Position::Center => width / 2 - element_width / 2,
            Position::End => width - element_width,
        }
    }
}

/// Determines whether a [`FlexContainer`] arranges its children
/// horizontally or vertically.
#[derive(Clone, Copy)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    /// Returns the direction orthogonal to the current one.
    ///
    /// * Horizontal → Vertical  
    /// * Vertical → Horizontal
    ///
    /// Useful when computing cross-axis alignment or spacing.
    fn orthogonalize(&self) -> Direction {
        match self {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        }
    }
}

impl TryFromValue for Direction {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "horizontal" => Direction::Horizontal,
            "vertical" => Direction::Vertical,
            _ => Err(ConversionError::InvalidValue {
                expected: "horizontal or vertical",
                actual: value,
            })?,
        })
    }
}

/// FC stands for Flex Container.
///
/// A helper type that abstracts away the difference between horizontal
/// and vertical layout, allowing the container logic to operate on a
/// single *main axis* and a single *cross axis*.
///
/// This prevents code duplication — layout math can be written once
/// for a generic axis, and converted back into X/Y coordinates on
/// demand.
struct FCPlane {
    main: AxisSegment,
    cross: AxisSegment,

    direction: Direction,
}

impl FCPlane {
    /// Creates a new [`FCPlane`] from an offset and container size, mapping
    /// corresponding main/cross axis values depending on [`Direction`].
    /// width/height to main/cross axes depending on [`Direction`].
    fn new<O, R>(offset: O, rect_size: R, direction: Direction) -> Self
    where
        O: Into<Offset<usize>>,
        R: Into<RectSize<usize>>,
    {
        let mut offset = offset.into();
        let mut rect_size = rect_size.into();

        if let Direction::Vertical = direction {
            (offset.x, offset.y) = (offset.y, offset.x);
            (rect_size.width, rect_size.height) = (rect_size.height, rect_size.width);
        }

        Self {
            main: AxisSegment::new(offset.x, rect_size.width),
            cross: AxisSegment::new(offset.y, rect_size.height),
            direction,
        }
    }

    /// Uses the providen cut length to increase offset and decrease
    /// restricted length of main axis.
    fn saturating_cut_front(&mut self, cut_len: usize) {
        self.main.start = self.main.start.saturating_add(cut_len);
        self.main.extent = self.main.extent.saturating_sub(cut_len);
    }

    /// Converts the plane’s main/auxiliary lengths back into a standard
    /// [`RectSize`] in X/Y coordinates.
    fn as_rect_size(&self) -> RectSize<usize> {
        let (mut width, mut height) = (self.main.extent, self.cross.extent);

        if let Direction::Vertical = self.direction {
            (width, height) = (height, width);
        }

        RectSize::new(width, height)
    }

    /// Converts the plane’s main/auxiliary offsets back into a standard
    /// [`Offset`] in X/Y coordinates.
    fn as_offset(&self) -> Offset<usize> {
        let (mut x, mut y) = (self.main.start, self.cross.start);

        if let Direction::Vertical = self.direction {
            (x, y) = (y, x);
        }

        Offset::new(x, y)
    }
}

struct AxisSegment {
    start: usize,
    extent: usize,
}

impl AxisSegment {
    fn new(start: usize, extent: usize) -> Self {
        Self { start, extent }
    }
}
