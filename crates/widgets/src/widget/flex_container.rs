use std::ops::{Add, AddAssign, Sub, SubAssign};

use log::warn;

use crate::{
    context::{LoadExtent, ManageDirtyFlags, ManageIntrinsic},
    decorator::{
        content::Content, DecoratorExt, DrawDecorator, EventHitTestDecorator, MeasureDecorator,
    },
    events::{
        EventContext, EventHandling, EventHitTest, EventRouter, HitTestResult,
        PendingEvent,
    },
    stage::{
        draw::{draw_debug_bounds, Draw, DrawContext, Drawer},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext, InvalidateVisitor, RebuildStatus},
        layout::{Layout, LayoutContext},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
    },
    types::{
        alignment::{Alignment, Position},
        border::Border,
        direction::Direction,
        extent::{Extent, FlexExtent},
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        offset::Offset,
        spacing::Spacing,
        style::{Configure, StyleProperty, WidgetStyle},
        Color, Point,
    },
    widget::{Widget, WidgetGetType, WidgetInformation, WidgetSizingMode},
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
    pub(crate) fn children_width<C>(&self, context: &C) -> f32
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
    pub(crate) fn children_height<C>(&self, context: &C) -> f32
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
    fn main_children_extent<C>(&self, context: &C) -> f32
    where
        C: LoadExtent<f32, WidgetId>,
    {
        match &self.direction {
            Direction::Horizontal => self.children_width(context),
            Direction::Vertical => self.children_height(context),
        }
    }

    /// Returns the total size occupied by children along the perpendicular axis.
    ///
    /// **Cross Extent** measures the "thickness" of the layout (e.g., the height of a row).
    #[allow(unused)]
    fn cross_children_extent<C>(&self, context: &C) -> f32
    where
        C: LoadExtent<f32, WidgetId>,
    {
        match &self.direction {
            Direction::Horizontal => self.children_height(context),
            Direction::Vertical => self.children_width(context),
        }
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

    fn iterate_over_children<F, C>(
        &self,
        context: &C,
        provided_extent: Extent<f32>,
        callback: &mut F,
    ) where
        C: LoadExtent<f32, WidgetId>,
        F: FnMut((usize, &Widget), Offset<f32>) -> IteratorProcess,
    {
        let mut plane = FCPlane::new(Offset::<f32>::default(), provided_extent, self.direction);

        let main_children_extent = self.main_children_extent(context);
        plane.main.start = self
            .main_axis_alignment()
            .get_start(plane.main.extent, main_children_extent);

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0.0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0.0
                } else {
                    (plane.main.extent - main_children_extent)
                        / self.children.len().saturating_sub(1) as f32
                }
            }
        };

        let cross_axis_start = plane.cross.start;
        let cross_axis_alignment = self.cross_axis_alignment();

        for (index, child) in self.children.iter().enumerate() {
            let child_extent = <C as LoadExtent<f32, WidgetId>>::load(context, child.get_id())
                .unwrap_or_default()
                .to_flex(&self.direction);

            plane.cross.start = cross_axis_start
                + cross_axis_alignment.get_start(plane.cross.extent, child_extent.cross);

            match callback((index, child), plane.as_offset()) {
                IteratorProcess::Continue => (),
                IteratorProcess::Break => break,
            }

            plane.cut_front(child_extent.main + incrementor);
        }
    }
}

enum IteratorProcess {
    Continue,
    Break,
}

impl WidgetInformation for FlexContainer {
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
}

impl WidgetGetType for FlexContainer {
    fn get_type(&self) -> &'static str {
        "flex_container"
    }
}

impl WidgetSizingMode for FlexContainer {
    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Dynamic
    }
}

impl<C> Init<C> for FlexContainer
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
            self.configure(container_style.clone());
        }

        self.children.iter_mut().for_each(|child| {
            child.init(context);
        });
    }
}

impl<C> Invalidate<C> for FlexContainer
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, _context: &mut C, style: WidgetStyle) {
        if let WidgetStyle::Container(container_style) = style {
            self.configure(container_style);
        }
    }

    fn on_rebuild(&mut self, _context: &mut C) -> RebuildStatus {
        RebuildStatus::NothingChanged
    }

    fn invalidate_children(&mut self, visitor: &mut impl InvalidateVisitor<C>) {
        for child in &mut self.children {
            visitor.invalidate(child);
        }
    }
}

impl Measure<f32> for FlexContainer {
    fn intrinsic_content<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        Content::intrinsic_fn(|| {
            if self.children.is_empty() {
                return measure::Intrinsic::default();
            }

            let mut min_intrinsic = FlexExtent::<f32> {
                main: 0.0,
                cross: 0.0,
            };

            let mut max_intrinsic = FlexExtent::<f32> {
                main: 0.0,
                cross: 0.0,
            };

            for child in &self.children {
                let child_intrinsic = child.intrinsic(context);

                let child_min_intrinsic = child_intrinsic.min.to_flex(&self.direction);
                min_intrinsic.main += child_min_intrinsic.main;
                min_intrinsic.cross = min_intrinsic.cross.max(child_min_intrinsic.cross);

                let child_max_intrinsic = child_intrinsic.max.to_flex(&self.direction);
                max_intrinsic.main += child_max_intrinsic.main;
                max_intrinsic.cross = max_intrinsic.cross.max(child_max_intrinsic.cross);
            }

            measure::Intrinsic::new(
                min_intrinsic.to_normal(&self.direction),
                max_intrinsic.to_normal(&self.direction),
            )
        })
        .spacing(self.spacing.unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .intrinsic()
    }

    fn measure_children(&self, visitor: &mut impl measure::MeasureVisitor<f32>) {
        for child in &self.children {
            visitor.measure(child);
        }
    }

    fn measure_content<C>(
        &self,
        context: &mut C,
        constraints: Constraints<Extent<f32>>,
    ) -> Extent<f32>
    where
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        Content::measure_fn(|constraints| {
            let mut fixed_children = vec![];
            let mut dynamic_children = vec![];

            for child in &self.children {
                let intrinsic = child.intrinsic(context);

                match child.sizing_mode() {
                    SizingMode::Fixed => fixed_children.push((child, intrinsic)),
                    SizingMode::Dynamic => dynamic_children.push((child, intrinsic)),
                }
            }

            let flex_constraints = Constraints {
                min: constraints.min.to_flex(&self.direction),
                max: constraints.max.to_flex(&self.direction),
            };

            let mut used_extent = <FlexExtent<f32>>::default();

            for (child, child_intrinsic) in fixed_children {
                let child_constraints = Constraints::new_tight(
                    FlexExtent {
                        main: child_intrinsic.max.by_direction(&self.direction),
                        cross: flex_constraints.max.cross,
                    }
                    .to_normal(&self.direction),
                );
                let child_used = child
                    .measure(context, child_constraints)
                    .to_flex(&self.direction);
                used_extent.main += child_used.main;
                used_extent.cross = used_extent.cross.max(child_used.cross);
            }

            let mut remainder = flex_constraints.max.main - used_extent.main;
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
                                cross: flex_constraints.max.cross,
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
                                cross: flex_constraints.max.cross,
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
                        cross: flex_constraints.max.cross,
                    }
                    .to_normal(&self.direction),
                );
                let child_used = child
                    .measure(context, child_constraints)
                    .to_flex(&self.direction);
                used_extent.main += child_used.main;
                used_extent.cross = used_extent.cross.max(child_used.cross);
            }

            let mut used_extent = (used_extent.to_normal(&self.direction))
                .clamp_with(constraints.min, constraints.max);

            if self.expand {
                match self.direction {
                    Direction::Horizontal => used_extent.width = constraints.max.width,
                    Direction::Vertical => used_extent.height = constraints.max.height,
                }
            }

            used_extent
        })
        .spacing(self.spacing.unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .measure(constraints)
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
    fn draw_content(
        &self,
        context: &C,
        offset: &Offset<f32>,
        provided_extent: Extent<f32>,
        drawer: &mut Drawer,
    ) {
        Content::draw_fn(
            |offset: &Offset<f32>, provided_extent: Extent<f32>, drawer: &mut Drawer| {
                self.iterate_over_children(
                    context,
                    provided_extent,
                    &mut |(_, child), local_offset| {
                        child.draw(context, &(local_offset + *offset), drawer);

                        IteratorProcess::Continue
                    },
                );

                if context.get_debug_options().show_layout_bounds {
                    let plane =
                        FCPlane::new(Offset::<f32>::default(), provided_extent, self.direction);
                    let shift = match self.direction {
                        Direction::Horizontal => Offset::new(plane.main.start, plane.cross.start),
                        Direction::Vertical => Offset::new(plane.cross.start, plane.main.start),
                    };

                    draw_debug_bounds(
                        drawer.surface.canvas(),
                        *offset + shift,
                        FlexExtent {
                            main: plane.main.extent,
                            cross: plane.cross.extent,
                        }
                        .to_normal(&self.direction),
                    );
                }
            },
        )
        .spacing(self.spacing.unwrap_or_default())
        .background(self.background_color.clone().unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .draw(offset, provided_extent, drawer);
    }
}

impl<C> EventHitTest<f32, C> for FlexContainer
where
    C: EventContext<f32>,
{
    fn on_hit_test(
        &self,
        context: &C,
        local_coords: Point<f32>,
        provided_extent: Extent<f32>,
        router: &mut EventRouter,
    ) -> HitTestResult {
        Content::hit_test_fn(
            |local_coords: Point<f32>, provided_extent: Extent<f32>, router: &mut EventRouter| {
                let mut result = HitTestResult::Missed;
                self.iterate_over_children(
                    context,
                    provided_extent,
                    &mut |(index, child), offset| {
                        result = child.hit_test(context, local_coords - offset.into(), router);

                        match &result {
                            HitTestResult::Missed => IteratorProcess::Continue,
                            HitTestResult::Hit => {
                                router.set_next_index(self.id, index);
                                IteratorProcess::Break
                            }
                            HitTestResult::Failed => IteratorProcess::Break,
                        }
                    },
                );

                result
            },
        )
        .spacing(self.spacing.unwrap_or_default())
        .border(self.border.clone().unwrap_or_default())
        .hit_test(self.id, local_coords, provided_extent, router)
    }
}

impl<C> EventHandling<f32, C> for FlexContainer
where
    C: EventContext<f32>,
{
    fn handle_events(
        &mut self,
        context: &mut C,
        _pending_events: Vec<PendingEvent>,
        next_child: usize,
        router: &EventRouter,
    ) {
        if let Some(child) = self.children.get_mut(next_child) {
            child.route_events(context, router);
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
