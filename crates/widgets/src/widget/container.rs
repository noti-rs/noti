use log::warn;
use macros::widget_style;

use crate::{
    context::{
        LoadConstraints, LoadExtent, ManageDirtyFlags, ManageIntrinsic, SaveConstraints,
        SaveExtent, StyleSubscription,
    },
    drawer::Drawer,
    events::{DispatchContext, DispatchEvent, Event},
    types::{
        alignment::Alignment,
        border::Border,
        dirty_flags::DirtyFlags,
        extent::Extent,
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
        offset::Offset,
        spacing::Spacing,
        style::{Configure, StyleProperty, WidgetStyle},
        Color, Point,
    },
    widget::{
        draw_debug_bounds, flex_container::FlexContainer, Draw, DrawContext, Init, InitContext,
        Invalidate, InvalidateContext, Layout, LayoutContext, Widget, WidgetGetType,
        WidgetInformation, WidgetSizingMode,
    },
};

/// A simple box that stays the same size.
///
/// Unlike the [`FlexContainer`], which stretches and moves to fit many things,
/// the `Container` is a rigid frame for just **one** child widget.
///
/// The Container does three main things:
/// * It sets a fixed width and height that never change.
/// * It holds exactly one child widget inside itself.
/// * It acts as a wall, so the child inside cannot push the box to make it bigger.
///
/// Use this when you need a UI element to stay exactly the same,
/// like a fixed icon or a status light that should never grow or shrink.
#[derive(bon::Builder)]
pub struct Container {
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

    /// The fill color or gradient applied to the entire area of the container.
    ///
    /// This defines the visual surface that sits behind any nested child
    /// widgets. It covers the full rectangular area of the container,
    /// providing a solid or decorative base. If not set, the container
    /// is typically transparent, allowing the parent's background to
    /// show through.
    #[builder(with = |v: Color| StyleProperty::Explicit(v), default)]
    background_color: StyleProperty<Color>,

    /// The visual frame and corner shaping applied to the container's edges.
    ///
    /// This field defines the stroke thickness, color, and curvature of
    /// the widget's boundary. It provides a clear visual distinction
    /// between the container's internal content and the rest of the
    /// layout.
    #[builder(with = |v: Border| StyleProperty::Explicit(v), default)]
    border: StyleProperty<Border>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(with = |v: Spacing| StyleProperty::Explicit(v), default)]
    spacing: StyleProperty<Spacing>,

    /// The rules for positioning content within the available internal space.
    ///
    /// This determines how the content (like text or nested widgets)
    /// anchors itself when the container is larger than the content
    /// it holds. It manages the distribution of "extra" space along
    /// the horizontal and vertical axes.
    #[builder(with = |v: Alignment| StyleProperty::Explicit(v), default)]
    alignment: StyleProperty<Alignment>,

    /// A hard-coded, fixed dimension for this axis.
    ///
    /// When set, the container will occupy exactly this many units regardless
    /// of its content's size or the parent's constraints. This effectively
    /// "locks" the widget's size, preventing it from expanding or
    /// shrinking during the layout pass.
    width: usize,

    /// A hard-coded, fixed dimension for this axis.
    ///
    /// When set, the container will occupy exactly this many units regardless
    /// of its content's size or the parent's constraints. This effectively
    /// "locks" the widget's size, preventing it from expanding or
    /// shrinking during the layout pass.
    height: usize,

    /// The single nested widget managed by this container.
    ///
    /// As a single-child provider, the container acts as a wrapper,
    /// applying its own alignment, background, and border rules to
    /// this inner element.
    child: Option<Widget>,
}

/// A targeted configuration set used to override or provide specific
/// parameters for a Container widget based on its unique identifier.
///
/// Instead of traversing the widget tree to modify an existing Container,
/// this struct allows external systems to inject layout and styling
/// data—such as alignment and borders—directly into the widget's
/// compilation phase. If no configuration is associated with a
/// widget's ID, it continues to use its own internal state.
#[widget_style(targets(Container, FlexContainer))]
#[derive(bon::Builder, Debug, Clone)]
pub struct ContainerStyle {
    pub background_color: Color,
    pub border: Border,
    pub spacing: Spacing,
    pub alignment: Alignment,
}

impl Container {
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
    /// respected by any child widgets.
    fn inner_spacing(&self) -> Spacing {
        self.spacing.unwrap_or_default()
            + Spacing::all_directional(
                self.border
                    .as_ref()
                    .map(|border| border.size)
                    .unwrap_or_default(),
            )
    }
}

impl WidgetInformation for Container {
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

impl WidgetGetType for Container {
    fn get_type(&self) -> &'static str {
        "container"
    }
}

impl WidgetSizingMode for Container {
    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Fixed
    }
}

impl<C> Invalidate<C> for Container
where
    C: InvalidateContext,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        let mut dirty_flags = context.get_dirty_flags(self.id);

        if dirty_flags.contains(DirtyFlags::NEEDS_UPDATE_STYLES) {
            if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
                self.configure(container_style.clone());
            }

            dirty_flags -= DirtyFlags::NEEDS_UPDATE_STYLES;
        }

        if let Some(child) = &mut self.child {
            let child_flags = child.invalidate(context);

            if child_flags.intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE) {
                dirty_flags |= DirtyFlags::CHILD_NEEDS_MEASURE;
            }
        }

        context.set_dirty_flags(self.id, dirty_flags);

        if dirty_flags.intersects(DirtyFlags::CHILD_NEEDS_MEASURE) {
            DirtyFlags::CHILD_NEEDS_MEASURE
        } else {
            DirtyFlags::empty()
        }
    }
}

impl<C> Init<C> for Container
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

        if let Some(child) = &mut self.child {
            child.init(context);
        }
    }
}

impl Measure<f32, WidgetId> for Container {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let dirty_flags = context.get_dirty_flags(self.id);
        let cached_intrinsic = context.load(self.id);

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) && cached_intrinsic.is_some() {
            return cached_intrinsic.unwrap_or_default();
        }

        let exact_extent = Extent::new(self.width as f32, self.height as f32);
        let intrinsic = measure::Intrinsic::new(exact_extent, exact_extent);
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

        if !dirty_flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            return cached_extent.unwrap_or_default();
        }

        if dirty_flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            if let Some(child) = &self.child {
                let child_constraints =
                    <C as LoadConstraints<f32, WidgetId>>::load(context, child.get_id())
                        .or_else(|| {
                            <C as LoadExtent<f32, WidgetId>>::load(context, child.get_id())
                                .map(Constraints::new_tight)
                        })
                        .unwrap_or_default();

                child.measure(context, child_constraints);
            }

            dirty_flags -= DirtyFlags::CHILD_NEEDS_MEASURE;
            context.set_dirty_flags(self.id, dirty_flags);

            return cached_extent.unwrap_or_default();
        }

        let extent = Extent::new(self.width as f32, self.height as f32);
        let inner_extent = extent.shrink_to_with(&self.inner_spacing());

        if let Some(child) = &self.child {
            child.measure(context, Constraints::new_soft(inner_extent));
        }

        let clamped_extent = extent.clamp_with(constraints.min, constraints.max);

        <C as SaveConstraints<f32, WidgetId>>::save(context, self.id, constraints);
        <C as SaveExtent<f32, WidgetId>>::save(context, self.id, clamped_extent);

        dirty_flags -= DirtyFlags::CHILD_NEEDS_MEASURE;
        context.set_dirty_flags(self.id, dirty_flags);

        clamped_extent
    }
}

impl<C> Layout<C, f32> for Container
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &C) {
        if context.load(self.id).is_none() {
            warn!("Container widget with id {} didn't measured!", *self.id);
        }

        if let Some(child) = &mut self.child {
            child.layout(context);
        }
    }
}

impl<C> Draw<C, f32> for Container
where
    C: DrawContext<f32>,
{
    fn draw_on(&self, context: &C, offset: &Offset<f32>, drawer: &mut Drawer) {
        let Some(provided_extent) = <C as LoadExtent<f32, WidgetId>>::load(context, self.id) else {
            warn!(
                "Container with id {} wasn't measured. Refused to draw.",
                *self.id
            );
            return;
        };

        let actual_extent = Extent::new(self.width as f32, self.height as f32);

        if provided_extent.width < actual_extent.width
            || provided_extent.height < actual_extent.height
            || self.width == 0
            || self.height == 0
        {
            return;
        }

        let actual_offset = *offset
            + Offset::new(
                (provided_extent.width - actual_extent.width) / 2.0,
                (provided_extent.height - actual_extent.height) / 2.0,
            );

        let canvas = drawer.surface.canvas();
        canvas.save();
        let rect = skia_safe::Rect::from_xywh(
            actual_offset.x,
            actual_offset.y,
            actual_extent.width,
            actual_extent.height,
        );

        let border = self.border.clone().unwrap_or_default();
        let border_radius = border.radius as f32;
        let rrect = skia_safe::RRect::new_rect_xy(rect, border_radius, border_radius);
        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        let background_color = self.background_color.clone().unwrap_or_default();
        if !background_color.is_transparent() {
            drawer.fill_background(actual_offset, actual_extent, &border, &background_color);
        }

        let inner_spacing = self.inner_spacing();
        let mut inner_extent = actual_extent;
        inner_extent.shrink_by(&inner_spacing);

        if let Some(child) = &self.child {
            let alignment = self.alignment.clone().unwrap_or_default();
            let child_extent =
                <C as LoadExtent<f32, WidgetId>>::load(context, child.get_id()).unwrap_or_default();

            let horizontal_start = alignment
                .horizontal
                .get_start(inner_extent.width, child_extent.width)
                + inner_spacing.left as f32;
            let vertical_start = alignment
                .vertical
                .get_start(inner_extent.height, child_extent.height)
                + inner_spacing.top as f32;

            let offset_for_child = actual_offset + Offset::new(horizontal_start, vertical_start);
            child.draw(context, &offset_for_child, drawer);
        }

        drawer.outline_border(actual_offset, actual_extent, &border);

        drawer.surface.canvas().restore();

        if context.get_debug_options().show_layout_bounds {
            draw_debug_bounds(
                drawer.surface.canvas(),
                *offset,
                provided_extent,
                actual_offset,
                actual_extent,
            );
        }
    }
}

impl<C> DispatchEvent<C, f32> for Container
where
    C: DispatchContext<f32>,
{
    fn dispatch_event(&mut self, context: &mut C, event: Event) {
        if !event.kind.is_mouse() {
            return;
        }

        let inner_spacing = self.inner_spacing();
        let mut inner_extent = Extent::new(self.width as f32, self.height as f32);
        inner_extent.shrink_by(&inner_spacing);

        let Some(child) = &mut self.child else {
            return;
        };

        if event.local_coord.x > self.width as f32 || event.local_coord.y > self.height as f32 {
            return;
        }

        let alignment = self.alignment.clone().unwrap_or_default();
        let child_extent = context.load(child.get_id()).unwrap_or_default();
        let horizontal_start = alignment
            .horizontal
            .get_start(inner_extent.width, child_extent.width)
            + inner_spacing.left as f32;
        let vertical_start = alignment
            .vertical
            .get_start(inner_extent.height, child_extent.height)
            + inner_spacing.top as f32;

        if event.local_coord.x < horizontal_start
            || event.local_coord.y < vertical_start
            || event.local_coord.x > horizontal_start + child_extent.width
            || event.local_coord.y > vertical_start + child_extent.height
        {
            return;
        }

        let mut modified_event = event.clone();
        modified_event.local_coord = Point {
            x: horizontal_start - event.local_coord.x,
            y: vertical_start - event.local_coord.y,
        };

        child.dispatch_event(context, modified_event)
    }
}
