use log::warn;

use crate::{
    context::{
        GenerateId, GetFont, GetState, GetStyle, LoadExtent, ManageDirtyFlags, ManageIntrinsic,
        RegisterKey, SaveExtent, Subscribe,
    },
    drawer::Drawer,
    events::{Action, DispatchEvent, Event, Point},
    make_configuration,
    types::{
        alignment::Alignment,
        border::{Border, BorderGBuilder},
        data::{Configure, ToConfig, WidgetStyle},
        dirty_flags::DirtyFlags,
        extent::Extent,
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
        offset::Offset,
        spacing::Spacing,
        Color,
    },
    widget::{
        flex_container::FlexContainer, Draw, Initialize, Invalidate, Layout, Widget, WidgetInfo,
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
#[derive(macros::GenericBuilder, derive_builder::Builder, Clone)]
#[builder(pattern = "owned")]
#[gbuilder(name(ContainerGBuilder), derive(Clone))]
pub struct Container {
    /// An optional identifier for this widget.
    ///
    /// If left empty, an ID will be automatically generated during
    /// compilation. Setting this manually allows the widget to be
    /// targeted by external configurations and makes the widget tree
    /// significantly easier to navigate during debugging.
    #[builder(private, default)]
    #[gbuilder(hidden, default)]
    id: WidgetId,

    #[builder(setter(strip_option, into), default)]
    #[gbuilder(default)]
    key: Option<WidgetKey>,

    #[builder(default, setter(into))]
    #[gbuilder(default)]
    class: WidgetClass,

    /// The fill color or gradient applied to the entire area of the container.
    ///
    /// This defines the visual surface that sits behind any nested child
    /// widgets. It covers the full rectangular area of the container,
    /// providing a solid or decorative base. If not set, the container
    /// is typically transparent, allowing the parent's background to
    /// show through.
    #[builder(setter(strip_option), default)]
    #[gbuilder(default)]
    background_color: Option<Color>,

    /// The visual frame and corner shaping applied to the container's edges.
    ///
    /// This field defines the stroke thickness, color, and curvature of
    /// the widget's boundary. It provides a clear visual distinction
    /// between the container's internal content and the rest of the
    /// layout.
    #[builder(setter(strip_option), default)]
    #[gbuilder(use_gbuilder(BorderGBuilder))]
    border: Option<Border>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(setter(strip_option), default)]
    #[gbuilder(default)]
    spacing: Option<Spacing>,

    /// The rules for positioning content within the available internal space.
    ///
    /// This determines how the content (like text or nested widgets)
    /// anchors itself when the container is larger than the content
    /// it holds. It manages the distribution of "extra" space along
    /// the horizontal and vertical axes.
    #[builder(setter(strip_option), default)]
    alignment: Option<Alignment>,

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
    #[gbuilder(default(None))]
    child: Option<Widget>,
}

make_configuration! {
    /// A targeted configuration set used to override or provide specific
    /// parameters for a Container widget based on its unique identifier.
    ///
    /// Instead of traversing the widget tree to modify an existing Container,
    /// this struct allows external systems to inject layout and styling
    /// data—such as alignment and borders—directly into the widget's
    /// compilation phase. If no configuration is associated with a
    /// widget's ID, it continues to use its own internal state.
    #[derive(Debug, Clone)]
    pub struct ContainerStyle {
        pub background_color: Color,
        pub border: Border,
        pub spacing: Spacing,
        pub alignment: Alignment,
    } <<= Container, FlexContainer
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

impl WidgetInfo for Container {
    fn get_id(&self) -> WidgetId {
        self.id
    }

    fn get_class(&self) -> WidgetClass {
        self.class.clone()
    }

    fn get_type(&self) -> &'static str {
        "container"
    }

    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Fixed
    }
}

impl<C> Invalidate<C> for Container
where
    C: ManageDirtyFlags<WidgetId> + GetState,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        let mut dirty_flags = context.get_dirty_flags(self.id);

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

impl<C> Initialize<C> for Container
where
    C: GenerateId
        + RegisterKey<WidgetKey, WidgetId>
        + GetState
        + Subscribe<WidgetId>
        + GetStyle
        + GetFont,
{
    fn initialize(&mut self, context: &mut C) {
        if *self.id == 0 {
            self.id = context.generate_id();
        }

        if let Some(key) = self.key.as_ref() {
            context.register_key(key.clone(), self.id);
        }

        if let Some(WidgetStyle::Container(container_style)) = context.get_style(&self.class) {
            self.configure(container_style.clone());
        }

        if let Some(child) = &mut self.child {
            child.initialize(context);
        }
    }
}

impl Measure<f32, WidgetId> for Container {
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId>,
    {
        if let Some(intrinsic) = context.load(self.id) {
            return intrinsic;
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
        if let Some(extent) = <C as LoadExtent<f32, WidgetId>>::load(context, self.id) {
            return extent;
        }

        let extent = Extent::new(self.width as f32, self.height as f32)
            .clamp_with(constraints.min, constraints.max);
        <C as SaveExtent<f32, WidgetId>>::save(context, self.id, extent);

        extent
    }
}

impl<C> Layout<C, f32, WidgetId> for Container
where
    C: LoadExtent<f32, WidgetId>,
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
    C: LoadExtent<f32, WidgetId>,
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
        let border = self.border.as_ref().cloned().unwrap_or_default();
        let border_radius = border.radius as f32;
        let rrect = skia_safe::RRect::new_rect_xy(rect, border_radius, border_radius);
        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        let background_color = self.background_color.as_ref().cloned().unwrap_or_default();
        if !background_color.is_transparent() {
            drawer.fill_background(actual_offset, actual_extent, &border, &background_color);
        }

        let inner_spacing = self.inner_spacing();
        let mut inner_extent = actual_extent;
        inner_extent.shrink_by(&inner_spacing);

        if let Some(child) = &self.child {
            let alignment = self.alignment.as_ref().cloned().unwrap_or_default();
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
    }
}

impl<C> DispatchEvent<C, f32> for Container
where
    C: LoadExtent<f32, WidgetId>,
{
    fn dispatch_event(&self, context: &C, event: Event) -> Action {
        if !event.kind.is_mouse() {
            return Action::None;
        }

        let Some(child) = &self.child else {
            return Action::None;
        };

        if event.local_coord.x > self.width as f32 || event.local_coord.y > self.height as f32 {
            return Action::None;
        }

        let inner_spacing = self.inner_spacing();
        let mut inner_extent = Extent::new(self.width as f32, self.height as f32);
        inner_extent.shrink_by(&inner_spacing);

        let alignment = self.alignment.as_ref().cloned().unwrap_or_default();
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
            return Action::None;
        }

        let mut modified_event = event.clone();
        modified_event.local_coord = Point {
            x: horizontal_start - event.local_coord.x,
            y: vertical_start - event.local_coord.y,
        };

        child.dispatch_event(context, modified_event)
    }
}
