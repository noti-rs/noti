use std::time::Duration;

use macros::{widget, widget_style};

use crate::{
    animations::{AnimationFilter, AnimationKind, Easing},
    context::{
        AnimationDirection, AnimationProgress, ManageAnimationRegistry, ManageDirtyFlags,
        ManageIntrinsic, ScopedContext, StateSubscription, StyleSubscription,
    },
    draw::{Draw, DrawContext, Drawer},
    events::{DispatchContext, DispatchEvent, Event},
    measure::{self, Constraints, Intrinsic, Measure, MeasureContext, SizingMode},
    state::State,
    types::{
        dirty_flags::DirtyFlags, identifiers::WidgetKey, style::Configure, Extent, Offset,
        WidgetClass, WidgetId, WidgetStyle,
    },
    widget::{
        Init, InitContext, Invalidate, InvalidateContext, Layout, LayoutContext, Widget,
        WidgetGetType, WidgetInformation, WidgetSizingMode,
    },
};

#[widget(kind = minimal)]
#[callback(on_visible)]
#[callback(on_hidden)]
#[callback(on_hover)]
#[derive(bon::Builder, Default)]
pub struct AnimatedVisibility {
    #[builder(default = true)]
    visible: bool,

    #[builder(into)]
    state: Option<State<bool>>,

    #[builder(default)]
    phase: VisibilityPhase,

    #[builder(skip)]
    animation_state: AnimationState,

    #[style(required)]
    primary_animation: AnimationDefinition,

    #[style]
    secondary_animation: AnimationDefinition,

    #[style(required)]
    primary_spatial_change: SpatialChangeDefinition,

    #[style]
    secondary_spatial_change: SpatialChangeDefinition,

    #[builder(into)]
    child: Option<Widget>,
}

#[widget_style(kind = minimal,targets(AnimatedVisibility))]
#[derive(bon::Builder, Debug, Default, Clone)]
pub struct AnimatedVisibilityStyle {
    primary_animation: AnimationDefinition,
    secondary_animation: AnimationDefinition,
    primary_spatial_change: SpatialChangeDefinition,
    secondary_spatial_change: SpatialChangeDefinition,
}

#[derive(Default, Clone, PartialEq, Eq)]
pub enum VisibilityPhase {
    #[default]
    Hidden,
    SpatialChange,
    Transition,
    Showing,
}

#[derive(Default, Clone)]
enum AnimationState {
    #[default]
    Nothing,
    Play,
    Unwind,
}

#[derive(Debug, Default, Clone)]
pub struct AnimationDefinition {
    pub easing: Easing,
    pub duration: Duration,
    pub kind: AnimationKind,
}

#[derive(Debug, Default, Clone)]
pub struct SpatialChangeDefinition {
    pub easing: Easing,
    pub duration: Duration,
}

impl WidgetGetType for AnimatedVisibility {
    fn get_type(&self) -> &'static str {
        "animated_visibility"
    }
}

impl WidgetSizingMode for AnimatedVisibility {
    fn sizing_mode(&self) -> SizingMode {
        match self.phase {
            VisibilityPhase::SpatialChange => SizingMode::Dynamic,
            VisibilityPhase::Transition | VisibilityPhase::Showing | VisibilityPhase::Hidden => {
                self.child
                    .as_ref()
                    .map(|child| child.sizing_mode())
                    .unwrap_or(SizingMode::Fixed)
            }
        }
    }
}

impl<C> Init<C> for AnimatedVisibility
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

        if let Some(WidgetStyle::AnimatedVisibility(av_style)) = context.get_style(&self.class) {
            self.configure(av_style.clone());
        }

        if let Some(state) = self.state {
            <C as StateSubscription<WidgetId>>::subscribe(context, self.id, state);

            if let Some(visible) = context.get(state) {
                self.visible = *visible;
            }
        }

        let target_phase = self.visibility_as_target_phase();
        self.fix_phase();

        if target_phase != self.phase {
            self.animation_state = AnimationState::Play;
            self.next_phase();

            self.register_animation(context);
        }

        if let Some(child) = &mut self.child {
            child.init(context);
        }
    }
}

impl<C> Invalidate<C> for AnimatedVisibility
where
    C: InvalidateContext,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        let mut dirty_flags = context.get_dirty_flags(self.id);

        if dirty_flags.contains(DirtyFlags::NEEDS_UPDATE_STYLES) {
            if let Some(WidgetStyle::AnimatedVisibility(av_style)) = context.get_style(&self.class)
            {
                self.configure(av_style.clone());

                dirty_flags |= DirtyFlags::NEEDS_MEASURE;
            }

            dirty_flags -= DirtyFlags::NEEDS_UPDATE_STYLES;
        }

        if let Some(new_visibility) = self
            .state
            .and_then(|state| context.get(state))
            .take_if(|new_visibility| **new_visibility != self.visible)
        {
            self.visible = *new_visibility;

            match self.animation_state {
                AnimationState::Nothing => {
                    self.animation_state = AnimationState::Play;
                    self.next_phase();

                    self.register_animation(context);
                }
                AnimationState::Play => {
                    self.animation_state = AnimationState::Unwind;
                    context.reverse_animation(self.id);
                }
                AnimationState::Unwind => {
                    self.animation_state = AnimationState::Play;
                    context.reverse_animation(self.id);
                }
            }

            dirty_flags -= DirtyFlags::NEEDS_REBUILD;
        }

        if dirty_flags.contains(DirtyFlags::NEEDS_REBUILD) {
            if context.is_finished(self.id) {
                self.next_phase();

                if self.is_fully_finished() {
                    self.animation_state = AnimationState::Nothing;

                    match self.phase {
                        VisibilityPhase::Hidden if self.on_hidden.is_some() => {
                            (self.on_hidden.as_mut().unwrap())(ScopedContext::new(context), ())
                        }
                        VisibilityPhase::Showing if self.on_visible.is_some() => {
                            (self.on_visible.as_mut().unwrap())(ScopedContext::new(context), ())
                        }
                        _ => (),
                    }

                    context.remove_animation(self.id);
                } else {
                    self.register_animation(context);
                }
            }

            if let VisibilityPhase::SpatialChange = self.phase {
                dirty_flags |= DirtyFlags::NEEDS_MEASURE;
            }

            dirty_flags -= DirtyFlags::NEEDS_REBUILD;
        }

        if let Some(child) = &mut self.child {
            let child_flags = child.invalidate(context);

            if child_flags.intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE) {
                dirty_flags |= DirtyFlags::CHILD_NEEDS_MEASURE;
            }
        }

        context.set_dirty_flags(self.id, dirty_flags);
        dirty_flags
    }
}

impl Measure<f32> for AnimatedVisibility {
    fn intrinsic_content<C>(&self, context: &mut C) -> Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        self.child
            .as_ref()
            .map(|child| child.intrinsic(context))
            .unwrap_or_default()
    }

    fn visit_children(&self, visitor: &mut impl measure::MeasureVisitor<f32>) {
        if let Some(child) = &self.child {
            visitor.visit(child);
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
        let mut used_extent = self
            .child
            .as_ref()
            .map(|child| child.measure(context, constraints))
            .unwrap_or_default();

        if let VisibilityPhase::SpatialChange = self.phase {
            let spatial_change_easing = &self.resolve_spatial_change_definition().easing;
            used_extent *=
                spatial_change_easing.ease(context.animation_progress(self.id).unwrap_or(1.0));
        }

        if let VisibilityPhase::Hidden = self.phase {
            used_extent *= 0.0;
        }

        used_extent
    }
}

impl<C> Layout<C, f32> for AnimatedVisibility
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &C) {
        if let Some(child) = &mut self.child {
            child.layout(context);
        }
    }
}

impl<C> Draw<C, f32> for AnimatedVisibility
where
    C: DrawContext<f32>,
{
    fn draw_content(
        &self,
        context: &C,
        offset: &Offset<f32>,
        _provided_extent: Extent<f32>,
        drawer: &mut Drawer,
    ) {
        match self.phase {
            VisibilityPhase::Hidden | VisibilityPhase::SpatialChange => (),
            VisibilityPhase::Transition => {
                if let Some(child) = &self.child {
                    let animation_definition = self.resolve_animation_definition();
                    let widget_image = drawer.draw_into_offscreen(context, offset, child);

                    let child_extent = context.load(child.get_id()).unwrap_or_default();
                    let progress = context.animation_progress(self.id).unwrap_or_default();
                    let paint = animation_definition.kind.filter(
                        widget_image,
                        *offset,
                        child_extent,
                        progress,
                    );

                    drawer.surface.canvas().draw_paint(&paint);
                }
            }
            VisibilityPhase::Showing => {
                if let Some(child) = &self.child {
                    child.draw(context, offset, drawer);
                }
            }
        }
    }
}

impl AnimatedVisibility {
    fn fix_phase(&mut self) {
        self.phase = match (self.visible, &self.phase) {
            (true, VisibilityPhase::SpatialChange) => VisibilityPhase::Hidden,
            (true, VisibilityPhase::Transition) => VisibilityPhase::Hidden,
            (false, VisibilityPhase::SpatialChange) => VisibilityPhase::Showing,
            (false, VisibilityPhase::Transition) => VisibilityPhase::Showing,
            (_, VisibilityPhase::Hidden) | (_, VisibilityPhase::Showing) => return,
        }
    }

    fn visibility_as_target_phase(&self) -> VisibilityPhase {
        if self.visible {
            VisibilityPhase::Showing
        } else {
            VisibilityPhase::Hidden
        }
    }

    fn resolve_spatial_change_definition(&self) -> &SpatialChangeDefinition {
        let enter_spatial_change = self
            .primary_spatial_change
            .as_ref()
            .expect("Primary spatial change must be set!");
        let exit_spatial_change = self
            .secondary_spatial_change
            .as_ref()
            .unwrap_or(enter_spatial_change);

        match (self.visible, &self.animation_state) {
            (true, AnimationState::Nothing) => enter_spatial_change,
            (true, AnimationState::Play) => enter_spatial_change,
            (true, AnimationState::Unwind) => exit_spatial_change,
            (false, AnimationState::Nothing) => exit_spatial_change,
            (false, AnimationState::Play) => exit_spatial_change,
            (false, AnimationState::Unwind) => enter_spatial_change,
        }
    }

    fn resolve_animation_definition(&self) -> &AnimationDefinition {
        let enter_animation = self
            .primary_animation
            .as_ref()
            .expect("Primary animation must be set!");
        let exit_animation = self.secondary_animation.as_ref().unwrap_or(enter_animation);

        match (self.visible, &self.animation_state) {
            (true, AnimationState::Nothing) => enter_animation,
            (true, AnimationState::Play) => enter_animation,
            (true, AnimationState::Unwind) => exit_animation,
            (false, AnimationState::Nothing) => exit_animation,
            (false, AnimationState::Play) => exit_animation,
            (false, AnimationState::Unwind) => enter_animation,
        }
    }

    fn next_phase(&mut self) {
        self.phase = match (self.visible, &self.phase) {
            (true, VisibilityPhase::Hidden) => VisibilityPhase::SpatialChange,
            (true, VisibilityPhase::SpatialChange) => VisibilityPhase::Transition,
            (true, VisibilityPhase::Transition) => VisibilityPhase::Showing,
            (true, VisibilityPhase::Showing) => VisibilityPhase::Showing,
            (false, VisibilityPhase::Hidden) => VisibilityPhase::Hidden,
            (false, VisibilityPhase::SpatialChange) => VisibilityPhase::Hidden,
            (false, VisibilityPhase::Transition) => VisibilityPhase::SpatialChange,
            (false, VisibilityPhase::Showing) => VisibilityPhase::Transition,
        }
    }

    fn is_fully_finished(&self) -> bool {
        matches!(
            (self.visible, &self.phase),
            (true, VisibilityPhase::Showing) | (false, VisibilityPhase::Hidden)
        )
    }

    fn register_animation<C: ManageAnimationRegistry<WidgetId>>(&self, context: &mut C) {
        let direction = if self.visible {
            AnimationDirection::Forward
        } else {
            AnimationDirection::Backward
        };

        match &self.phase {
            VisibilityPhase::SpatialChange => {
                let spatial_change_definition = self.resolve_spatial_change_definition();
                context.register_animation(
                    self.id,
                    AnimationProgress::new(
                        spatial_change_definition.duration,
                        DirtyFlags::NEEDS_REBUILD | DirtyFlags::NEEDS_MEASURE,
                        direction,
                    ),
                );
            }
            VisibilityPhase::Transition => {
                let animation_definition = self.resolve_animation_definition();

                context.register_animation(
                    self.id,
                    AnimationProgress::new(
                        animation_definition.duration,
                        DirtyFlags::NEEDS_REBUILD,
                        direction,
                    ),
                );
            }
            _ => (),
        }
    }
}

impl<C> DispatchEvent<C, f32> for AnimatedVisibility
where
    C: DispatchContext<f32>,
{
    fn dispatch_event(&mut self, context: &mut C, event: Event) {
        if matches!(event.kind, crate::events::EventKind::MouseHover) {
            if let Some(on_hover) = self.on_hover.as_mut() {
                on_hover(ScopedContext::new(context), ())
            }
        }

        if let Some(child) = &mut self.child {
            child.dispatch_event(context, event);
        }
    }
}
