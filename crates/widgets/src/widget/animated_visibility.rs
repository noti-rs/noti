use std::time::Duration;

use crate::{
    animations::{AnimationFilter, AnimationKind, Easing},
    context::{
        AnimationDirection, AnimationProgress, LoadConstraints, LoadExtent,
        ManageAnimationRegistry, ManageDirtyFlags, ManageIntrinsic, SaveConstraints, SaveExtent,
        ScopedContext,
    },
    drawer::Drawer,
    events::{Callback, DispatchContext, DispatchEvent, Event, FunctionCallback},
    state::State,
    types::{
        dirty_flags::DirtyFlags,
        identifiers::WidgetKey,
        measure::{Constraints, Intrinsic, Measure, MeasureContext, SizingMode},
        Extent, Offset, WidgetClass, WidgetId,
    },
    widget::{
        Draw, DrawContext, Init, InitContext, Invalidate, InvalidateContext, Layout, LayoutContext,
        Widget, WidgetBase,
    },
};

#[derive(derive_builder::Builder, Default)]
#[builder(pattern = "owned")]
pub struct AnimatedVisibility {
    #[builder(private, default)]
    id: WidgetId,

    #[builder(setter(strip_option, into), default)]
    key: Option<WidgetKey>,

    #[builder(setter(into), default)]
    class: WidgetClass,

    #[builder(default = true)]
    visible: bool,

    #[builder(setter(strip_option, into), default)]
    state: Option<State<bool>>,

    #[builder(default)]
    phase: VisibilityPhase,

    #[builder(private, default)]
    animation_state: AnimationState,

    primary_animation: AnimationDefinition,

    #[builder(setter(strip_option), default)]
    secondary_animation: Option<AnimationDefinition>,

    primary_spatial_change: SpatialChangeDefinition,

    #[builder(setter(strip_option), default)]
    secondary_spatial_change: Option<SpatialChangeDefinition>,

    #[builder(setter(strip_option, into), default)]
    child: Option<Widget>,

    #[builder(setter(custom), default)]
    on_visible: Option<Callback<()>>,

    #[builder(setter(custom), default)]
    on_hidden: Option<Callback<()>>,

    #[builder(setter(custom), default)]
    on_hover: Option<Callback<()>>,
}

impl AnimatedVisibilityBuilder {
    pub fn on_visible<F>(mut self, callback: F) -> Self
    where
        F: for<'a> FunctionCallback<'a, ()> + 'static,
    {
        self.on_visible = Some(Some(Box::new(callback)));
        self
    }

    pub fn on_hidden<F>(mut self, callback: F) -> Self
    where
        F: for<'a> FunctionCallback<'a, ()> + 'static,
    {
        self.on_hidden = Some(Some(Box::new(callback)));
        self
    }

    pub fn on_hover<F>(mut self, callback: F) -> Self
    where
        F: for<'a> FunctionCallback<'a, ()> + 'static,
    {
        self.on_hover = Some(Some(Box::new(callback)));
        self
    }
}

impl Clone for AnimatedVisibility {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            class: self.class.clone(),
            visible: self.visible,
            state: self.state,
            phase: self.phase.clone(),
            animation_state: self.animation_state.clone(),
            primary_animation: self.primary_animation.clone(),
            secondary_animation: self.secondary_animation.clone(),
            primary_spatial_change: self.primary_spatial_change.clone(),
            secondary_spatial_change: self.secondary_spatial_change.clone(),
            child: self.child.clone(),
            on_visible: None,
            on_hidden: None,
            on_hover: None,
        }
    }
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

#[derive(Default, Clone)]
pub struct AnimationDefinition {
    pub easing: Easing,
    pub duration: Duration,
    pub kind: AnimationKind,
}

#[derive(Default, Clone)]
pub struct SpatialChangeDefinition {
    pub easing: Easing,
    pub duration: Duration,
}

impl WidgetBase for AnimatedVisibility {
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
        "animated_visibility"
    }

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
        if let Some(state) = self.state {
            context.subscribe(self.id, state);

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

impl Measure<f32, WidgetId> for AnimatedVisibility {
    fn get_intrinsic<C>(&self, context: &mut C) -> Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let dirty_flags = context.get_dirty_flags(self.id);
        let cached_intrinsic = context.load(self.id);

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) && cached_intrinsic.is_some() {
            return cached_intrinsic.unwrap_or_default();
        }

        let intrinsic = self
            .child
            .as_ref()
            .map(|child| child.get_intrinsic(context))
            .unwrap_or_default();
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

        <C as SaveConstraints<f32, WidgetId>>::save(context, self.id, constraints);
        <C as SaveExtent<f32, WidgetId>>::save(context, self.id, used_extent);

        dirty_flags -= DirtyFlags::CHILD_NEEDS_MEASURE;
        context.set_dirty_flags(self.id, dirty_flags);

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
    fn draw_on(&self, context: &C, offset: &Offset<f32>, drawer: &mut Drawer) {
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
        let enter_spatial_change = &self.primary_spatial_change;
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
        let enter_animation = &self.primary_animation;
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
        if matches!(event.kind, crate::events::EventKind::MouseHover) && self.on_hover.is_some() {
            (self.on_hover.as_mut().unwrap())(ScopedContext::new(context), ())
        }

        if let Some(child) = &mut self.child {
            child.dispatch_event(context, event);
        }
    }
}
