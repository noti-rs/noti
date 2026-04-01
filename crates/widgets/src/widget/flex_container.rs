use std::ops::{Add, AddAssign, Sub, SubAssign};

use log::warn;

use crate::{
    drawer::Drawer,
    events::{Action, DispatchEvent, Event, Point},
    types::{
        alignment::{Alignment, Position},
        border::{Border, BorderGBuilder},
        constraints::{Constraints, SizingMode},
        data::{Configure, WidgetConfig},
        direction::Direction,
        extent::Extent2D,
        offset::Offset,
        spacing::Spacing,
        widget_id::WidgetId,
        Color,
    },
    Compile, CompileCtx, CompileResult, Draw, Widget, WidgetInfo,
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
/// common layouts without duplicating positioning logic.
#[derive(macros::GenericBuilder, derive_builder::Builder, Clone)]
#[builder(pattern = "owned")]
#[gbuilder(name(FlexContainerGBuilder), derive(Clone))]
pub struct FlexContainer {
    /// An optional identifier for this widget.
    ///
    /// If left empty, an ID will be automatically generated during
    /// compilation. Setting this manually allows the widget to be
    /// targeted by external configurations and makes the widget tree
    /// significantly easier to navigate during debugging.
    #[builder(default, setter(into))]
    #[gbuilder(default)]
    id: WidgetId,

    /// The computed size of the container after compilation.
    ///
    /// `None` means the container has not been compiled yet, so its
    /// dimensions are unknown. After a successful call to [`Self::compile`],
    /// this will contain the resolved [`Extent2D`].
    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default(None))]
    extent: Option<Extent2D<usize>>,

    #[builder(default = false)]
    #[gbuilder(default(false))]
    expand: bool,

    /// The fill color or gradient applied to the entire area of the container.
    ///
    /// This defines the visual surface that sits behind any nested child
    /// widgets. It covers the full rectangular area of the container,
    /// providing a solid or decorative base. If not set, the container
    /// is typically transparent, allowing the parent's background to
    /// show through.
    #[builder(setter(strip_option), default)]
    #[gbuilder(default)]
    pub(super) background_color: Option<Color>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(setter(strip_option), default)]
    #[gbuilder(default)]
    pub(super) spacing: Option<Spacing>,

    /// The visual frame and corner shaping applied to the container's edges.
    ///
    /// This field defines the stroke thickness, color, and curvature of
    /// the widget's boundary. It provides a clear visual distinction
    /// between the container's internal content and the rest of the
    /// layout.
    #[builder(setter(strip_option), default)]
    #[gbuilder(use_gbuilder(BorderGBuilder))]
    pub(super) border: Option<Border>,

    /// The rules for positioning content within the available internal space.
    ///
    /// This determines how the content (like text or nested widgets)
    /// anchors itself when the container is larger than the content
    /// it holds. It manages the distribution of "extra" space along
    /// the horizontal and vertical axes.
    #[builder(setter(strip_option), default)]
    pub(super) alignment: Option<Alignment>,

    /// The primary axis used for arranging the child widgets.
    ///
    /// This determines the "flow" of the container. In a horizontal
    /// direction, children are laid out side-by-side in a row. In a
    /// vertical direction, they are stacked on top of each other
    /// in a column.
    direction: Direction,

    /// The list of widgets to be arranged and managed by this container.
    ///
    /// These children are positioned sequentially along the chosen
    /// `direction`. The container calculates the space for each child
    /// based on the total available area and the specific alignment
    /// rules applied to the flex layout.
    children: Vec<Widget>,
}

impl FlexContainer {
    /// Calculates the total space required by all children along a specific axis.
    ///
    /// - **Horizontal Direction:** the sum of all children's widths.
    /// - **Vertical Direction:** the width of the widest child.
    pub fn inner_width(&self) -> usize {
        let widths = self.children.iter().map(|child| child.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.max().unwrap_or_default(),
        }
    }

    /// Calculates the total space required by all children along a specific axis.
    ///
    /// - **Horizontal Direction:** the height of the tallest child.
    /// - **Vertical Direction:** the sum of all children's heights.
    pub fn inner_height(&self) -> usize {
        let heights = self.children.iter().map(|child| child.height());

        match self.direction {
            Direction::Horizontal => heights.max().unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    /// Returns the total size occupied by children along the primary axis.
    ///
    /// The **Main Extent** follows the container's `direction` (e.g., total
    /// width in a row).
    fn main_inner_extent(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.inner_width(),
            Direction::Vertical => self.inner_height(),
        }
    }

    /// Returns the total size occupied by children along the perpendicular axis.
    ///
    /// **Cross Extent** measures the "thickness" of the layout (e.g., the height of a row).
    #[allow(unused)]
    fn cross_inner_extent(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.inner_height(),
            Direction::Vertical => self.inner_width(),
        }
    }

    /// Calculates the total internal offset required to protect the container's
    /// content from its boundaries.
    ///
    /// This method provides a unified [`Spacing`] value by combining the
    /// widget's defined internal padding with the physical thickness of
    /// the [`Border`].
    ///
    /// **The Calculation:**
    /// `Total Inner Spacing = Manual Spacing + All-Directional Border Size`
    ///
    /// By using this combined value during the compilation and drawing
    /// stages, the container ensures that its children are correctly
    /// "inset." This prevents nested widgets from overlapping with the
    /// border strokes and accurately determines the remaining available
    /// area for the layout.
    ///
    /// # Returns
    /// A [`Spacing`] struct representing the total margin that must be
    fn inner_spacing(&self) -> Spacing {
        self.spacing.unwrap_or_default()
            + Spacing::all_directional(
                self.border
                    .as_ref()
                    .map(|border| border.size)
                    .unwrap_or_default(),
            )
    }

    /// Retrieves the alignment rules specifically for the primary axis.
    ///
    /// This unifies how the container treats positioning:
    /// - In a **Row**, Horizontal.
    /// - In a **Column**, Vertical.
    fn main_axis_alignment(&self) -> Position {
        let alignment = self.alignment.as_ref().cloned().unwrap_or_default();

        match &self.direction {
            Direction::Horizontal => alignment.horizontal,
            Direction::Vertical => alignment.vertical,
        }
    }

    /// Retrieves the alignment rules specifically for the secondary axis.
    ///
    /// This unifies how the container treats positioning:
    /// - In a **Row**, Vertical.
    /// - In a **Column**, Horizontal.
    fn cross_axis_alignment(&self) -> Position {
        let alignment = self.alignment.as_ref().cloned().unwrap_or_default();

        match &self.direction {
            Direction::Horizontal => alignment.vertical,
            Direction::Vertical => alignment.horizontal,
        }
    }

    /// Generates a unified coordinate mapper ([`FCPlane`]) for the current layout.
    ///
    /// This is the "logic bridge" that allows the container to arrange children
    /// without checking the `direction` constantly. It returns an `FCPlane`
    /// pre-loaded with the current inner spacing and extents, enabling
    /// the layout engine to map generic "offsets" to actual screen
    /// coordinates regardless of whether the container is a row or a column.
    fn get_plane<T>(&self, mut extent: Extent2D<T>) -> FCPlane<T>
    where
        T: Default
            + Copy
            + Add<Output = T>
            + Sub<Output = T>
            + num_traits::FromPrimitive
            + std::cmp::PartialOrd,
    {
        let inner_spacing = self.inner_spacing();
        extent.shrink_by(&inner_spacing);

        FCPlane::new(inner_spacing, extent, self.direction)
    }

    /// Determines the starting coordinate and the gap size for child placement.
    ///
    /// Based on the `main_axis_alignment`, this calculates exactly where the
    /// first child should begin on the main axis and how much space (if any)
    /// should be added between subsequent children (e.g., for `SpaceBetween`).
    /// These values are only calculated for the main axis, as cross-axis
    /// positioning is handled independently.
    fn get_start_and_incrementor(&self, restricted_extent: usize) -> (usize, usize) {
        // INFO: if flex container is not expands, then arrange children consecutively without
        // gaps.
        if !self.expand {
            return (0, 0);
        }

        let start = self
            .main_axis_alignment()
            .get_start(restricted_extent, self.main_inner_extent());

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0
                } else {
                    (restricted_extent - self.main_inner_extent())
                        / self.children.len().saturating_sub(1)
                }
            }
        };

        (start, incrementor)
    }
}

impl WidgetInfo for FlexContainer {
    fn get_type(&self) -> &'static str {
        "flex_container"
    }

    /// Returns the final, compiled dimensions of the entire FlexContainer.
    ///
    /// This includes the combined size of all children (`inner_width`) plus
    /// the additional padding and border thickness provided by [`inner_spacing`].
    fn width(&self) -> usize {
        self.extent
            .as_ref()
            .map(|extent| extent.width)
            .unwrap_or_default()
    }

    /// Returns the final, compiled dimensions of the entire FlexContainer.
    ///
    /// This includes the combined size of all children (`inner_height`) plus
    /// the additional padding and border thickness provided by [`inner_spacing`].
    fn height(&self) -> usize {
        self.extent
            .as_ref()
            .map(|extent| extent.height)
            .unwrap_or_default()
    }

    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Dynamic
    }
}

impl Compile for FlexContainer {
    /// Compiles the container and its children, determining the final size
    /// and layout arrangement.
    ///
    /// This method:
    /// 1. Iterates over all children and calls their `compile` methods,
    ///    passing them the remaining available space.
    /// 2. Shrinks the available space after each successful child
    ///    compilation.
    /// 3. Stores the resolved container size in `extent` so that future
    ///    layout operations can use it.
    ///
    /// Callers **must** invoke `compile` before calling [`Self::width`], [`Self::height`],
    /// or attempting to draw the container, since layout information is
    /// unavailable until compilation is complete.
    fn compile(
        &mut self,
        constraints: Constraints<f32>,
        compile_ctx: &mut CompileCtx,
    ) -> CompileResult {
        if self.id.is_empty() {
            self.id = compile_ctx.generate_new_id(self.get_type());
        }

        if let Some(WidgetConfig::Container(container_config)) = compile_ctx
            .data_pool
            .get(&self.id)
            .and_then(|associated_data| associated_data.config.as_ref())
        {
            self.configure(container_config.clone());
        }

        let available_space = Extent2D::new(constraints.max.width, constraints.max.height);
        let mut plane = self.get_plane(available_space);

        self.children
            .iter_mut()
            .filter(|widget| widget.sizing_mode().is_fixed())
            .for_each(|child| {
                let constraints_for_child = Constraints::from_extent_soft(plane.as_extent());
                match child.compile(constraints_for_child, compile_ctx) {
                    CompileResult::Success { used_extent } => {
                        plane.cut_front(used_extent.by_direction(&self.direction));
                    }
                    CompileResult::Failure => {
                        warn!("Widget failed to compile in {}.", self.id);
                    }
                }
            });

        let mut total_dynamic_childs = self
            .children
            .iter()
            .filter(|child| !child.sizing_mode().is_fixed())
            .count();

        self.children
            .iter_mut()
            .filter(|widget| !widget.sizing_mode().is_fixed())
            .for_each(|child| {
                let fair_share = plane.main.extent / total_dynamic_childs as f32;
                let fair_extent =
                    Extent2D::new_from_direction(fair_share, plane.cross.extent, &self.direction);
                let constraints_for_child = Constraints::from_extent_soft(fair_extent);

                match child.compile(constraints_for_child, compile_ctx) {
                    CompileResult::Success { used_extent } => {
                        plane.cut_front(used_extent.by_direction(&self.direction));
                        total_dynamic_childs -= 1;
                    }
                    CompileResult::Failure => {
                        warn!("Widget failed to compile in {}.", self.id);
                    }
                }
            });

        if self.children.iter().all(|child| child.is_unknown()) {
            warn!(
                "{} widget have all failed to compile child widgets. Maybe they cannot be fitted in available space.", self.id
            );
        }

        let used_extent = if self.expand {
            Extent2D::new(constraints.max.width, constraints.max.height).into()
        } else {
            let inner_spacing = self.inner_spacing();
            Extent2D::new(
                std::cmp::max(
                    inner_spacing.horizontal() + self.inner_width(),
                    constraints.min.width as usize,
                ),
                std::cmp::max(
                    inner_spacing.vertical() + self.inner_height(),
                    constraints.min.height as usize,
                ),
            )
        };
        self.extent = Some(used_extent);

        CompileResult::Success {
            used_extent: used_extent.into(),
        }
    }
}

impl Draw for FlexContainer {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let Some(extent) = self.extent.as_ref().cloned() else {
            warn!(
                "{} widgets possibly not compiled. Refusing to draw.",
                self.id
            );
            return;
        };

        let background_color = self.background_color.as_ref().cloned().unwrap_or_default();
        let border = self.border.as_ref().cloned().unwrap_or_default();

        if !background_color.is_transparent() {
            drawer.fill_background(offset.into(), extent.into(), &border, &background_color);
        }

        let mut plane = self.get_plane(extent);
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

            plane.cut_front(child.len_by_direction(&self.direction) + incrementor);
        }

        drawer.outline_border(offset.into(), extent.into(), &border);
    }
}

impl DispatchEvent for FlexContainer {
    fn dispatch_event(&self, event: Event) -> Action {
        let Some(extent) = self.extent.as_ref().cloned() else {
            warn!(
                "{} widgets possibly not compiled. Refusing to handle event.",
                self.id
            );
            return Action::None;
        };

        if !event.kind.is_mouse() {
            return Action::None;
        }

        if event.local_coord.x > extent.width || event.local_coord.y > extent.height {
            return Action::None;
        }

        let (mouse_main, mouse_cross) = if let Direction::Horizontal = self.direction {
            (event.local_coord.x, event.local_coord.y)
        } else {
            (event.local_coord.y, event.local_coord.x)
        };

        let mut plane = self.get_plane(extent);
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

            plane.cut_front(child_main_len + incrementor);
        }

        Action::None
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
struct FCPlane<T>
where
    T: Default + Copy,
{
    main: AxisSegment<T>,
    cross: AxisSegment<T>,

    direction: Direction,
}

impl<T> FCPlane<T>
where
    T: Default + Copy,
{
    /// Creates a new [`FCPlane`] from an offset and container extent, mapping
    /// corresponding main/cross axis values depending on [`Direction`].
    fn new<O, R>(offset: O, extent: R, direction: Direction) -> Self
    where
        O: Into<Offset<T>>,
        R: Into<Extent2D<T>>,
    {
        let mut offset = offset.into();
        let mut extent = extent.into();

        if let Direction::Vertical = direction {
            (offset.x, offset.y) = (offset.y, offset.x);
            (extent.width, extent.height) = (extent.height, extent.width);
        }

        Self {
            main: AxisSegment::new(offset.x, extent.width),
            cross: AxisSegment::new(offset.y, extent.height),
            direction,
        }
    }

    /// Uses the provided cut length to increase offset and decrease
    /// extent of main axis.
    fn cut_front(&mut self, cut_len: T)
    where
        T: Add<Output = T> + Sub<Output = T> + AddAssign + SubAssign + PartialOrd,
    {
        let new_start = self.main.start + cut_len;
        if new_start >= self.main.start {
            self.main.start = new_start;
        } // INFO: otherwise a number wraps around and better do nothing, because we don't know
        // which is the number type. In case of signed number (like i32), need to use formula
        // `start += cut_len + new_start`, but in case of unsigned number (like u32), the formula
        // changes to `start += cut_len - new_start`.

        if self.main.extent < cut_len {
            self.main.extent -= self.main.extent;
        } else {
            self.main.extent -= cut_len;
        }
    }

    /// Converts the plane’s main/cross extents back into a standard
    /// [`Extent2D`] in X/Y coordinates.
    fn as_extent(&self) -> Extent2D<T> {
        let (mut width, mut height) = (self.main.extent, self.cross.extent);

        if let Direction::Vertical = self.direction {
            (width, height) = (height, width);
        }

        Extent2D::new(width, height)
    }

    /// Converts the plane’s main/cross offsets back into a standard
    /// [`Offset`] in X/Y coordinates.
    fn as_offset(&self) -> Offset<T> {
        let (mut x, mut y) = (self.main.start, self.cross.start);

        if let Direction::Vertical = self.direction {
            (x, y) = (y, x);
        }

        Offset::new(x, y)
    }
}

/// Think of `AxisSegment` as a simple ruler laid down on a line.
///
/// Instead of worrying about whether we are talking about X (left-to-right)
/// or Y (top-to-bottom), we just care about two things: where the ruler
/// starts and how long it is.
///
/// This is super helpful for layout containers that have direction of arranging children.
/// By turning 2D coordinates into these simple segments, we can apply
/// the same logic to both the "main" direction and the "cross" direction
/// without getting confused.
struct AxisSegment<T>
where
    T: Default + Copy,
{
    /// Where the segment begins on the axis.
    start: T,
    /// How far the segment reaches from the start.
    extent: T,
}

impl<T> AxisSegment<T>
where
    T: Default + Copy,
{
    fn new(start: T, extent: T) -> Self {
        Self { start, extent }
    }
}
