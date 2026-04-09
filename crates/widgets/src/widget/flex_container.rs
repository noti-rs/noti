use std::ops::{Add, AddAssign, Sub, SubAssign};

use log::warn;

use crate::{
    context::{
        LoadConstraints, LoadExtent, ManageDirtyFlags, ManageIntrinsic, SaveConstraints,
        SaveExtent, StyleSubscription,
    },
    drawer::Drawer,
    events::{DispatchContext, DispatchEvent, Event},
    types::{
        alignment::{Alignment, Position},
        border::Border,
        direction::Direction,
        dirty_flags::DirtyFlags,
        extent::{Extent, FlexExtent},
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
        offset::Offset,
        spacing::Spacing,
        style::{Configure, StyleProperty, WidgetStyle},
        Color, Point,
    },
    widget::{
        Draw, DrawContext, Init, InitContext, Invalidate, InvalidateContext, Layout, LayoutContext,
        Widget, WidgetBase,
    },
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
#[derive(bon::Builder)]
pub struct FlexContainer {
    /// An optional identifier for this widget.
    ///
    /// If left empty, an ID will be automatically generated during
    /// compilation. Setting this manually allows the widget to be
    /// targeted by external configurations and makes the widget tree
    /// significantly easier to navigate during debugging.
    #[builder(skip)]
    id: WidgetId,

    #[builder(into)]
    key: Option<WidgetKey>,

    #[builder(into, default)]
    class: WidgetClass,

    #[builder(default = false)]
    expand: bool,

    /// The fill color or gradient applied to the entire area of the container.
    ///
    /// This defines the visual surface that sits behind any nested child
    /// widgets. It covers the full rectangular area of the container,
    /// providing a solid or decorative base. If not set, the container
    /// is typically transparent, allowing the parent's background to
    /// show through.
    #[builder(with = |v: Color| StyleProperty::Explicit(v), default)]
    pub(super) background_color: StyleProperty<Color>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(with = |v: Spacing| StyleProperty::Explicit(v), default)]
    pub(super) spacing: StyleProperty<Spacing>,

    /// The visual frame and corner shaping applied to the container's edges.
    ///
    /// This field defines the stroke thickness, color, and curvature of
    /// the widget's boundary. It provides a clear visual distinction
    /// between the container's internal content and the rest of the
    /// layout.
    #[builder(with = |v: Border| StyleProperty::Explicit(v), default)]
    pub(super) border: StyleProperty<Border>,

    /// The rules for positioning content within the available internal space.
    ///
    /// This determines how the content (like text or nested widgets)
    /// anchors itself when the container is larger than the content
    /// it holds. It manages the distribution of "extra" space along
    /// the horizontal and vertical axes.
    #[builder(with = |v: Alignment| StyleProperty::Explicit(v), default)]
    pub(super) alignment: StyleProperty<Alignment>,

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
    pub(crate) fn inner_width<C>(&self, context: &C) -> f32
    where
        C: LoadExtent<f32, WidgetId>,
    {
        let widths = self
            .children
            .iter()
            .map(|child| context.load(child.get_id()).unwrap_or_default().width);

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.reduce(|a, b| a.max(b)).unwrap_or_default(),
        }
    }

    /// Calculates the total space required by all children along a specific axis.
    ///
    /// - **Horizontal Direction:** the height of the tallest child.
    /// - **Vertical Direction:** the sum of all children's heights.
    pub(crate) fn inner_height<C>(&self, context: &C) -> f32
    where
        C: LoadExtent<f32, WidgetId>,
    {
        let heights = self
            .children
            .iter()
            .map(|child| context.load(child.get_id()).unwrap_or_default().height);

        match self.direction {
            Direction::Horizontal => heights.reduce(|a, b| a.max(b)).unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    /// Returns the total size occupied by children along the primary axis.
    ///
    /// The **Main Extent** follows the container's `direction` (e.g., total
    /// width in a row).
    fn main_inner_extent<C>(&self, context: &C) -> f32
    where
        C: LoadExtent<f32, WidgetId>,
    {
        match &self.direction {
            Direction::Horizontal => self.inner_width(context),
            Direction::Vertical => self.inner_height(context),
        }
    }

    /// Returns the total size occupied by children along the perpendicular axis.
    ///
    /// **Cross Extent** measures the "thickness" of the layout (e.g., the height of a row).
    #[allow(unused)]
    fn cross_inner_extent<C>(&self, context: &C) -> f32
    where
        C: LoadExtent<f32, WidgetId>,
    {
        match &self.direction {
            Direction::Horizontal => self.inner_height(context),
            Direction::Vertical => self.inner_width(context),
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
        let alignment = self.alignment.clone().unwrap_or_default();

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
        let alignment = self.alignment.clone().unwrap_or_default();

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
    fn get_start_and_incrementor<C>(&self, context: &C, restricted_extent: f32) -> (f32, f32)
    where
        C: LoadExtent<f32, WidgetId>,
    {
        // INFO: if flex container is not expands, then arrange children consecutively without
        // gaps.
        if !self.expand {
            return (0.0, 0.0);
        }

        let main_inner_extent = self.main_inner_extent(context);
        let start = self
            .main_axis_alignment()
            .get_start(restricted_extent, main_inner_extent);

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0.0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0.0
                } else {
                    (restricted_extent - main_inner_extent)
                        / self.children.len().saturating_sub(1) as f32
                }
            }
        };

        (start, incrementor)
    }
}

impl WidgetBase for FlexContainer {
    fn get_id(&self) -> WidgetId {
        self.id
    }

    fn set_id(&mut self, id: WidgetId) {
        self.id = id;
    }

    fn get_key(&self) -> Option<&WidgetKey> {
        self.key.as_ref()
    }

    fn get_class(&self) -> WidgetClass {
        self.class.clone()
    }

    fn get_type(&self) -> &'static str {
        "flex_container"
    }

    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Dynamic
    }
}

impl<C> Invalidate<C> for FlexContainer
where
    C: InvalidateContext,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        let mut dirty_flags = context.get_dirty_flags(self.id);

        if dirty_flags.contains(DirtyFlags::NEEDS_UPDATE_STYLES) {
            if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
                self.configure(container_style.clone());

                dirty_flags |= DirtyFlags::NEEDS_MEASURE;
            }

            dirty_flags -= DirtyFlags::NEEDS_UPDATE_STYLES;
        }

        for child in &mut self.children {
            let child_flags = child.invalidate(context);

            if child_flags.contains(DirtyFlags::NEEDS_MEASURE) {
                dirty_flags |= DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE;
            } else if child_flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE) {
                dirty_flags |= DirtyFlags::CHILD_NEEDS_MEASURE;
            }
        }

        context.set_dirty_flags(self.id, dirty_flags);
        dirty_flags
    }
}

impl<C> Init<C> for FlexContainer
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        if !self.class.is_empty() {
            <C as StyleSubscription<WidgetClass, WidgetId>>::subscribe(
                context,
                self.id,
                self.class.clone(),
            );
        }

        if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
            self.configure(container_style.clone());
        }

        self.children.iter_mut().for_each(|child| {
            child.init(context);
        });
    }
}

impl Measure<f32, WidgetId> for FlexContainer {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let dirty_flags = context.get_dirty_flags(self.id);
        let cached_intrinsic = context.load(self.id);

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) && cached_intrinsic.is_some() {
            return cached_intrinsic.unwrap_or_default();
        }

        let inner_spacing = self.inner_spacing();
        let spacing_size = Extent::from(inner_spacing);

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
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let mut dirty_flags = context.get_dirty_flags(self.id);
        let constraints_changed =
            Some(constraints) != <C as LoadConstraints<f32, WidgetId>>::load(context, self.id);
        let cached_extent = <C as LoadExtent<f32, WidgetId>>::load(context, self.id);

        if !dirty_flags.intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            return cached_extent.unwrap_or_default();
        }

        if dirty_flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            for child in &self.children {
                let child_constraints =
                    <C as LoadConstraints<f32, WidgetId>>::load(context, child.get_id())
                        .or_else(|| {
                            <C as LoadExtent<f32, WidgetId>>::load(context, child.get_id())
                                .map(Constraints::new_tight)
                        })
                        .unwrap();

                child.measure(context, child_constraints);
            }

            dirty_flags -= DirtyFlags::CHILD_NEEDS_MEASURE;
            context.set_dirty_flags(self.id, dirty_flags);

            return cached_extent.unwrap_or_default();
        }

        let inner_spacing = self.inner_spacing();
        let spacing_size = Extent::from(inner_spacing);

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

        <C as SaveConstraints<f32, WidgetId>>::save(context, self.id, constraints);
        <C as SaveExtent<f32, WidgetId>>::save(context, self.id, used_extent);

        dirty_flags -= DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE;
        context.set_dirty_flags(self.id, dirty_flags);

        used_extent
    }
}

impl<C> Layout<C, f32> for FlexContainer
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &C) {
        if context.load(self.id).is_none() {
            warn!("FlexContainer with id {} didn't measured!", *self.id);
        }

        for child in &mut self.children {
            child.layout(context);
        }
    }
}

impl<C> Draw<C, f32> for FlexContainer
where
    C: DrawContext<f32>,
{
    fn draw_on(&self, context: &C, offset: &Offset<f32>, drawer: &mut Drawer) {
        let Some(provided_extent) = <C as LoadExtent<f32, WidgetId>>::load(context, self.id) else {
            warn!(
                "FlexContainer with id {} didn't measured. Refused to draw.",
                *self.id
            );
            return;
        };

        let inner_spacing = self.inner_spacing();

        if provided_extent.width < inner_spacing.horizontal() as f32
            || provided_extent.height <= inner_spacing.vertical() as f32
        {
            return;
        }

        let mut plane = self.get_plane(provided_extent);
        let (start, incrementor) = self.get_start_and_incrementor(context, plane.main.extent);
        plane.main.start += start;

        let background_color = self.background_color.clone().unwrap_or_default();
        let border = self.border.clone().unwrap_or_default();

        let canvas = drawer.surface.canvas();
        canvas.save();

        let rect = skia_safe::Rect::from_xywh(
            offset.x,
            offset.y,
            provided_extent.width,
            provided_extent.height,
        );
        let rrect = skia_safe::RRect::new_rect_xy(rect, border.radius as f32, border.radius as f32);

        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        if !background_color.is_transparent() {
            drawer.fill_background(*offset, provided_extent, &border, &background_color);
        }

        let cross_axis_start = plane.cross.start;
        let cross_axis_alignment = self.cross_axis_alignment();
        for child in &self.children {
            let child_extent = <C as LoadExtent<f32, WidgetId>>::load(context, child.get_id())
                .unwrap_or_default()
                .to_flex(&self.direction);

            plane.cross.start = cross_axis_start
                + cross_axis_alignment.get_start(plane.cross.extent, child_extent.cross);

            child.draw(context, &(plane.as_offset() + *offset), drawer);

            plane.cut_front(child_extent.main + incrementor);
        }

        drawer.outline_border(*offset, provided_extent, &border);

        drawer.surface.canvas().restore();
    }
}

impl<C> DispatchEvent<C, f32> for FlexContainer
where
    C: DispatchContext<f32>,
{
    fn dispatch_event(&mut self, context: &mut C, event: Event) {
        let Some(provided_extent) = context.load(self.id) else {
            return;
        };

        if !event.kind.is_mouse() {
            return;
        }

        if event.local_coord.x > provided_extent.width
            || event.local_coord.y > provided_extent.height
        {
            return;
        }

        let (mouse_main, mouse_cross) = if let Direction::Horizontal = self.direction {
            (event.local_coord.x, event.local_coord.y)
        } else {
            (event.local_coord.y, event.local_coord.x)
        };

        let mut plane = self.get_plane(provided_extent);
        let (start, incrementor) = self.get_start_and_incrementor(context, plane.main.extent);
        plane.main.start += start;

        if mouse_main < plane.main.start || mouse_cross < plane.cross.start {
            return;
        }

        let cross_axis_alignment = self.cross_axis_alignment();

        for child in &mut self.children {
            let child_extent = context
                .load(child.get_id())
                .unwrap_or_default()
                .to_flex(&self.direction);

            if mouse_main <= plane.main.start + child_extent.main {
                let widget_start =
                    cross_axis_alignment.get_start(plane.cross.extent, child_extent.cross);

                if mouse_cross >= plane.cross.start + widget_start
                    && mouse_cross <= plane.cross.start + widget_start + child_extent.cross
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
                    return child.dispatch_event(context, modified_event);
                }

                return;
            }

            plane.cut_front(child_extent.main + incrementor);
        }
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
    #[allow(unused)]
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
