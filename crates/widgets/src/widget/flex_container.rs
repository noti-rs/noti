use std::ops::{Add, AddAssign, Sub, SubAssign};

use log::warn;

use crate::{
    context::{GenerateId, GetData, GetFont, GetStyle, LoadExtent, SaveExtent},
    drawer::Drawer,
    events::{Action, DispatchEvent, Event, Point},
    types::{
        alignment::{Alignment, Position},
        border::{Border, BorderGBuilder},
        data::{Configure, WidgetStyle},
        direction::Direction,
        extent::{Extent, FlexExtent},
        identifiers::{WidgetClass, WidgetId},
        measure::{self, Constraints, ExtentManagement, IntrinsicManagement, Measure, SizingMode},
        offset::Offset,
        spacing::Spacing,
        Color,
    },
    widget::{Compile, CompileCtx, CompileResult, Draw, Initialize, Layout, Widget, WidgetInfo},
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
    #[builder(private, default)]
    #[gbuilder(hidden, default)]
    id: WidgetId,

    #[builder(default, setter(into))]
    #[gbuilder(default)]
    class: WidgetClass,

    /// The computed size of the container after compilation.
    ///
    /// `None` means the container has not been compiled yet, so its
    /// dimensions are unknown. After a successful call to [`Self::compile`],
    /// this will contain the resolved [`Extent2D`].
    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default)]
    extent: Extent<f32>,

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
    pub fn inner_width(&self) -> f32 {
        let widths = self.children.iter().map(|child| child.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.reduce(|a, b| a.max(b)).unwrap_or_default(),
        }
    }

    /// Calculates the total space required by all children along a specific axis.
    ///
    /// - **Horizontal Direction:** the height of the tallest child.
    /// - **Vertical Direction:** the sum of all children's heights.
    pub fn inner_height(&self) -> f32 {
        let heights = self.children.iter().map(|child| child.height());

        match self.direction {
            Direction::Horizontal => heights.reduce(|a, b| a.max(b)).unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    /// Returns the total size occupied by children along the primary axis.
    ///
    /// The **Main Extent** follows the container's `direction` (e.g., total
    /// width in a row).
    fn main_inner_extent(&self) -> f32 {
        match &self.direction {
            Direction::Horizontal => self.inner_width(),
            Direction::Vertical => self.inner_height(),
        }
    }

    /// Returns the total size occupied by children along the perpendicular axis.
    ///
    /// **Cross Extent** measures the "thickness" of the layout (e.g., the height of a row).
    #[allow(unused)]
    fn cross_inner_extent(&self) -> f32 {
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
    fn get_plane<T>(&self, mut extent: Extent<T>) -> FCPlane<T>
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
    fn get_start_and_incrementor(&self, restricted_extent: f32) -> (f32, f32) {
        // INFO: if flex container is not expands, then arrange children consecutively without
        // gaps.
        if !self.expand {
            return (0.0, 0.0);
        }

        let start = self
            .main_axis_alignment()
            .get_start(restricted_extent, self.main_inner_extent());

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0.0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0.0
                } else {
                    (restricted_extent - self.main_inner_extent())
                        / self.children.len().saturating_sub(1) as f32
                }
            }
        };

        (start, incrementor)
    }
}

impl WidgetInfo for FlexContainer {
    fn get_class(&self) -> WidgetClass {
        self.class.clone()
    }

    fn get_type(&self) -> &'static str {
        "flex_container"
    }

    /// Returns the final, compiled dimensions of the entire FlexContainer.
    ///
    /// This includes the combined size of all children (`inner_width`) plus
    /// the additional padding and border thickness provided by [`inner_spacing`].
    fn width(&self) -> f32 {
        self.extent.width
    }

    /// Returns the final, compiled dimensions of the entire FlexContainer.
    ///
    /// This includes the combined size of all children (`inner_height`) plus
    /// the additional padding and border thickness provided by [`inner_spacing`].
    fn height(&self) -> f32 {
        self.extent.height
    }

    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Dynamic
    }
}

impl<C> Initialize<C> for FlexContainer
where
    C: GenerateId + GetData + GetStyle + GetFont,
{
    fn initialize(&mut self, context: &mut C) {
        if *self.id == 0 {
            self.id = context.generate_id();
        }

        if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
            self.configure(container_style.clone());
        }

        self.children.iter_mut().for_each(|child| {
            child.initialize(context);
        });
    }
}

impl Measure<f32, WidgetId> for FlexContainer {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: IntrinsicManagement<f32, WidgetId>,
    {
        if let Some(intrinsic) = context.load(self.id) {
            return intrinsic;
        }

        let inner_spacing = self.inner_spacing();
        let spacing_size = Extent::new(
            inner_spacing.horizontal() as f32,
            inner_spacing.vertical() as f32,
        );

        if self.children.is_empty() {
            return measure::Intrinsic::new(spacing_size, spacing_size);
        }

        let mut main_min_intrinsic = 0.0;
        let mut main_max_intrinsic = 0.0;
        let mut cross_min_intrinsic = 0.0;
        let mut cross_max_intrinsic = 0.0;
        for child in &self.children {
            let child_intrinsic = child.get_intrinsic(context);

            let main_min = child_intrinsic.min.by_direction(&self.direction);
            let cross_min = child_intrinsic
                .min
                .by_direction(&self.direction.orthogonalize());

            let main_max = child_intrinsic.max.by_direction(&self.direction);
            let cross_max = child_intrinsic
                .max
                .by_direction(&self.direction.orthogonalize());

            main_min_intrinsic += main_min;
            main_max_intrinsic += main_max;

            if cross_min_intrinsic < cross_min {
                cross_min_intrinsic = cross_min;
            }

            if cross_max_intrinsic < cross_max {
                cross_max_intrinsic = cross_max;
            }
        }

        let intrinsic = match self.direction {
            Direction::Horizontal => measure::Intrinsic::new(
                Extent::new(main_min_intrinsic, cross_min_intrinsic) + spacing_size,
                Extent::new(main_max_intrinsic, cross_max_intrinsic) + spacing_size,
            ),
            Direction::Vertical => measure::Intrinsic::new(
                Extent::new(cross_min_intrinsic, main_min_intrinsic) + spacing_size,
                Extent::new(cross_max_intrinsic, main_max_intrinsic) + spacing_size,
            ),
        };

        context.save(self.id, intrinsic);
        intrinsic
    }

    fn measure<C>(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: IntrinsicManagement<f32, WidgetId> + ExtentManagement<f32, WidgetId>,
    {
        if let Some(extent) = <C as LoadExtent<f32, WidgetId>>::load(context, self.id) {
            return extent;
        }

        let inner_spacing = self.inner_spacing();
        let spacing_size = Extent::new(
            inner_spacing.horizontal() as f32,
            inner_spacing.vertical() as f32,
        );

        let mut fixed_children = vec![];
        let mut dynamic_children = vec![];

        for child in &self.children {
            let intrinsic = child.get_intrinsic(context);

            match child.sizing_mode() {
                SizingMode::Fixed => fixed_children.push((child, intrinsic)),
                SizingMode::Dynamic => dynamic_children.push((child, intrinsic)),
            }
        }

        let mut inner_constraints = constraints;
        inner_constraints.max.shrink_by(&inner_spacing);
        inner_constraints.min.shrink_by(&inner_spacing);

        inner_constraints.max.width = inner_constraints.max.width.max(0.0);
        inner_constraints.max.height = inner_constraints.max.height.max(0.0);

        inner_constraints.min.width = inner_constraints
            .min
            .width
            .max(0.0)
            .min(inner_constraints.max.height);
        inner_constraints.min.height = inner_constraints
            .min
            .height
            .max(0.0)
            .min(inner_constraints.max.height);

        let inner_constraints = Constraints {
            min: inner_constraints.min.to_flex(&self.direction),
            max: inner_constraints.max.to_flex(&self.direction),
        };

        let mut used_extent = <FlexExtent<f32>>::default();

        for (child, child_intrinsic) in fixed_children {
            let child_constraints = Constraints::new_tight(
                FlexExtent {
                    main: child_intrinsic.max.by_direction(&self.direction),
                    cross: inner_constraints.max.cross,
                }
                .to_normal(&self.direction),
            );
            let child_used = child
                .measure(context, child_constraints)
                .to_flex(&self.direction);
            used_extent.main += child_used.main;
            used_extent.cross = used_extent.cross.max(child_used.cross);
        }

        let mut remainder = inner_constraints.max.main - used_extent.main;
        let mut freezed_childs = vec![];
        let mut base_fair_share = 0.0;

        'outer: while !dynamic_children.is_empty() {
            let fair_share = remainder / dynamic_children.len() as f32;

            for i in 0..dynamic_children.len() {
                let (child, child_intrinsic) = dynamic_children[i];

                let child_min_main = child_intrinsic.min.by_direction(&self.direction);
                let child_max_main = child_intrinsic.max.by_direction(&self.direction);

                if fair_share > child_max_main {
                    dynamic_children.remove(i);

                    freezed_childs.push((
                        child,
                        FlexExtent {
                            main: child_max_main,
                            cross: inner_constraints.max.cross,
                        },
                    ));

                    remainder -= child_max_main;
                    continue 'outer;
                } else if fair_share < child_min_main {
                    dynamic_children.remove(i);

                    freezed_childs.push((
                        child,
                        FlexExtent {
                            main: child_min_main,
                            cross: inner_constraints.max.cross,
                        },
                    ));

                    remainder = (remainder - child_min_main).max(0.0);
                    continue 'outer;
                }
            }

            base_fair_share = fair_share;
            break;
        }

        for (child, fair_share_extent) in freezed_childs {
            let child_constraints =
                Constraints::new_soft(fair_share_extent.to_normal(&self.direction));
            let child_used = child
                .measure(context, child_constraints)
                .to_flex(&self.direction);
            used_extent.main += child_used.main;
            used_extent.cross = used_extent.cross.max(child_used.cross);
        }

        for (child, _) in dynamic_children {
            let child_constraints = Constraints::new_soft(
                FlexExtent {
                    main: base_fair_share,
                    cross: inner_constraints.max.cross,
                }
                .to_normal(&self.direction),
            );
            let child_used = child
                .measure(context, child_constraints)
                .to_flex(&self.direction);
            used_extent.main += child_used.main;
            used_extent.cross = used_extent.cross.max(child_used.cross);
        }

        let mut used_extent = (used_extent.to_normal(&self.direction) + spacing_size)
            .clamp_with(constraints.min, constraints.max);

        if self.expand {
            match self.direction {
                Direction::Horizontal => used_extent.width = constraints.max.width,
                Direction::Vertical => used_extent.height = constraints.max.height,
            }
        }

        <C as SaveExtent<f32, WidgetId>>::save(context, self.id, used_extent);
        used_extent
    }
}

impl<C> Layout<C, f32, WidgetId> for FlexContainer
where
    C: LoadExtent<f32, WidgetId>,
{
    fn layout(&mut self, context: &C) {
        if let Some(extent) = context.load(self.id) {
            self.extent = extent;
        } else {
            warn!("FlexContainer with id {} didn't measured!", *self.id);
        }

        for child in &mut self.children {
            child.layout(context);
        }
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
        constraints: Constraints<Extent<f32>>,
        compile_ctx: &mut CompileCtx,
    ) -> CompileResult {
        if self.class.is_empty() {
            self.class = compile_ctx.generate_new_class(self.get_type());
        }

        if let Some(WidgetStyle::Container(container_config)) = compile_ctx
            .data_pool
            .get(&self.class)
            .and_then(|associated_data| associated_data.style.as_ref())
        {
            self.configure(container_config.clone());
        }

        let available_space = Extent::new(constraints.max.width, constraints.max.height);
        let mut plane = self.get_plane(available_space);

        self.children
            .iter_mut()
            .filter(|widget| widget.sizing_mode().is_fixed())
            .for_each(|child| {
                let constraints_for_child = Constraints::new_soft(plane.as_extent());
                match child.compile(constraints_for_child, compile_ctx) {
                    CompileResult::Success { used_extent } => {
                        plane.cut_front(used_extent.by_direction(&self.direction));
                    }
                    CompileResult::Failure => {
                        warn!("Widget failed to compile in {}.", self.class);
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
                    Extent::new_from_direction(fair_share, plane.cross.extent, &self.direction);
                let constraints_for_child = Constraints::new_soft(fair_extent);

                match child.compile(constraints_for_child, compile_ctx) {
                    CompileResult::Success { used_extent } => {
                        plane.cut_front(used_extent.by_direction(&self.direction));
                        total_dynamic_childs -= 1;
                    }
                    CompileResult::Failure => {
                        warn!("{} failed to compile in {}.", child.get_class(), self.class);
                    }
                }
            });

        if self.children.iter().all(|child| child.is_unknown()) {
            warn!(
                "{} widget have all failed to compile child widgets. Maybe they cannot be fitted in available space.", self.class
            );
        }

        let used_extent = if self.expand {
            Extent::new(constraints.max.width, constraints.max.height)
        } else {
            let inner_spacing = self.inner_spacing();
            Extent::new(
                (inner_spacing.horizontal() as f32 + self.inner_width()).max(constraints.min.width),
                (inner_spacing.vertical() as f32 + self.inner_height()).max(constraints.min.height),
            )
        };
        self.extent = used_extent;

        CompileResult::Success { used_extent }
    }
}

impl Draw for FlexContainer {
    fn draw_with_offset(&self, offset: &Offset<f32>, drawer: &mut Drawer) {
        let inner_spacing = self.inner_spacing();
        if self.extent.width < inner_spacing.horizontal() as f32
            || self.extent.height <= inner_spacing.vertical() as f32
        {
            return;
        }

        let mut plane = self.get_plane(self.extent);
        let (start, incrementor) = self.get_start_and_incrementor(plane.main.extent);
        plane.main.start += start;

        let background_color = self.background_color.as_ref().cloned().unwrap_or_default();
        let border = self.border.as_ref().cloned().unwrap_or_default();

        let canvas = drawer.surface.canvas();
        canvas.save();

        let rect =
            skia_safe::Rect::from_xywh(offset.x, offset.y, self.extent.width, self.extent.height);
        let rrect = skia_safe::RRect::new_rect_xy(rect, border.radius as f32, border.radius as f32);

        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        if !background_color.is_transparent() {
            drawer.fill_background(*offset, self.extent, &border, &background_color);
        }

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

        drawer.outline_border(*offset, self.extent, &border);

        drawer.surface.canvas().restore();
    }
}

impl DispatchEvent for FlexContainer {
    fn dispatch_event(&self, event: Event) -> Action {
        if !event.kind.is_mouse() {
            return Action::None;
        }

        if event.local_coord.x > self.extent.width || event.local_coord.y > self.extent.height {
            return Action::None;
        }

        let (mouse_main, mouse_cross) = if let Direction::Horizontal = self.direction {
            (event.local_coord.x, event.local_coord.y)
        } else {
            (event.local_coord.y, event.local_coord.x)
        };

        let mut plane = self.get_plane(self.extent);
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
        R: Into<Extent<T>>,
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
    fn as_extent(&self) -> Extent<T> {
        let (mut width, mut height) = (self.main.extent, self.cross.extent);

        if let Direction::Vertical = self.direction {
            (width, height) = (height, width);
        }

        Extent::new(width, height)
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
