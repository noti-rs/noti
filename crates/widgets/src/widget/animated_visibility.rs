use std::time::Duration;

use macros::widget;
use shared::unique::Unique;

use crate::{
    animations::{AnimationFilter, AnimationKind, Easing},
    context::{
        widget_data, widget_data_mut, AnimationDirection, AnimationProgress,
        ManageAnimationRegistry, ManageIntrinsic, ScopedContext, StateSubscription,
    },
    decorator::{content::Content, DecoratorExt, EventHitTestDecorator},
    events::{EventContext, EventHandling, EventHitTest, EventRouter, HitTestResult, PendingEvent},
    stage::{
        deinit::{Deinit, DeinitContext},
        draw::{Draw, DrawContext, Drawer},
        init::{Init, InitContext},
        invalidate::{Invalidate, InvalidateContext, RebuildStatus},
        layout::{Layout, LayoutContext},
        measure::{Constraints, Intrinsic, ManageMeasures, Measure, MeasureContext, SizingMode},
    },
    state::State,
    types::{
        dirty_flags::DirtyFlags, identifiers::WidgetKey, style::Configure, Extent, Offset, Point,
        WidgetClass, WidgetId, WidgetStyle,
    },
    widget::{WidgetGetType, WidgetInformationContext, WidgetSizingMode},
};

#[widget(kind = minimal)]
#[callback(on_visible)]
#[callback(on_hidden)]
#[callback(on_hover)]
#[callback(on_leave)]
#[make_widget_style(AnimatedVisibilityStyle, derive(bon::Builder, Debug, Default, Clone))]
#[derive(bon::Builder, Default)]
pub struct AnimatedVisibility {
    #[builder(into)]
    #[dirty(NEEDS_REBUILD)]
    visibility_state: Option<State<bool>>,

    #[style(required)]
    primary_animation: AnimationDefinition,

    #[style]
    secondary_animation: AnimationDefinition,

    #[style(required)]
    primary_spatial_change: SpatialChangeDefinition,

    #[style]
    secondary_spatial_change: SpatialChangeDefinition,
}

struct AnimationProperties<'a> {
    enter_animation: &'a AnimationDefinition,
    exit_animation: &'a AnimationDefinition,

    enter_spatial_change: &'a SpatialChangeDefinition,
    exit_spatial_change: &'a SpatialChangeDefinition,
}

impl<'a> AnimationProperties<'a> {
    fn resolve_spatical_change(
        &self,
        animation_appearance: AnimationAppearance,
    ) -> &'a SpatialChangeDefinition {
        match animation_appearance {
            AnimationAppearance::Enter => self.enter_spatial_change,
            AnimationAppearance::Exit => self.exit_spatial_change,
        }
    }

    fn resolve_animation(
        &self,
        animation_appearance: AnimationAppearance,
    ) -> &'a AnimationDefinition {
        match animation_appearance {
            AnimationAppearance::Enter => self.enter_animation,
            AnimationAppearance::Exit => self.exit_animation,
        }
    }
}

/// The runtime information of [AnimatedVisibility].
#[derive(Clone)]
struct AVRuntimeInformation {
    visible: bool,
    phase: VisibilityPhase,
    animation_state: AnimationState,
}

impl AVRuntimeInformation {
    fn visibility_as_target_phase(&self) -> VisibilityPhase {
        if self.visible {
            VisibilityPhase::Showing
        } else {
            VisibilityPhase::Hidden
        }
    }

    fn next_phase(&mut self) {
        self.phase.next_phase(self.visible);
    }

    fn is_fully_finished(&self) -> bool {
        self.phase.is_fully_finished(self.visible)
    }

    fn register_animation<C: ManageAnimationRegistry<WidgetId>>(
        &self,
        context: &mut C,
        id: WidgetId,
        animation_properties: AnimationProperties<'_>,
    ) {
        let direction = if self.visible {
            AnimationDirection::Forward
        } else {
            AnimationDirection::Backward
        };

        let animation_appearance = self.resolve_animation_appearance();

        match &self.phase {
            VisibilityPhase::SpatialChange => {
                let spatial_change_definition =
                    animation_properties.resolve_spatical_change(animation_appearance);

                context.register_animation(
                    id,
                    AnimationProgress::new(
                        spatial_change_definition.duration,
                        DirtyFlags::NEEDS_REBUILD | DirtyFlags::NEEDS_MEASURE,
                        direction,
                    ),
                );
            }
            VisibilityPhase::Transition => {
                let animation_definition =
                    animation_properties.resolve_animation(animation_appearance);

                context.register_animation(
                    id,
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

    fn resolve_animation_appearance(&self) -> AnimationAppearance {
        use AnimationAppearance::*;
        match (self.visible, &self.animation_state) {
            (true, AnimationState::Nothing) => Enter,
            (true, AnimationState::Play) => Enter,
            (true, AnimationState::Unwind) => Exit,
            (false, AnimationState::Nothing) => Exit,
            (false, AnimationState::Play) => Exit,
            (false, AnimationState::Unwind) => Enter,
        }
    }
}

enum AnimationAppearance {
    Enter,
    Exit,
}

#[derive(Default, Clone, PartialEq, Eq)]
pub enum VisibilityPhase {
    #[default]
    Hidden,
    SpatialChange,
    Transition,
    Showing,
}

impl VisibilityPhase {
    fn fix_with_visibility(&mut self, visibility: bool) {
        *self = match (visibility, &self) {
            (true, VisibilityPhase::SpatialChange) => VisibilityPhase::Hidden,
            (true, VisibilityPhase::Transition) => VisibilityPhase::Hidden,
            (false, VisibilityPhase::SpatialChange) => VisibilityPhase::Showing,
            (false, VisibilityPhase::Transition) => VisibilityPhase::Showing,
            (_, VisibilityPhase::Hidden) | (_, VisibilityPhase::Showing) => return,
        }
    }

    fn next_phase(&mut self, visibility: bool) {
        *self = match (visibility, &self) {
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

    fn is_fully_finished(&self, visibility: bool) -> bool {
        matches!(
            (visibility, &self),
            (true, VisibilityPhase::Showing) | (false, VisibilityPhase::Hidden)
        )
    }
}

#[derive(Default, Clone)]
enum AnimationState {
    #[default]
    Nothing,
    Play,
    Unwind,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AnimationDefinition {
    pub easing: Easing,
    pub duration: Duration,
    pub kind: AnimationKind,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SpatialChangeDefinition {
    pub easing: Easing,
    pub duration: Duration,
}

impl WidgetGetType for AnimatedVisibility {
    fn get_type(&self) -> &'static str {
        "animated_visibility"
    }
}

impl<C> WidgetSizingMode<C> for AnimatedVisibility
where
    C: WidgetInformationContext,
{
    fn sizing_mode(&self, context: &C) -> SizingMode {
        let runtime_information: &AVRuntimeInformation = widget_data(context, self.id)
            .expect("An associated runtime information must exists for AnimatedVisibility.");

        match runtime_information.phase {
            VisibilityPhase::SpatialChange => SizingMode::Dynamic,
            VisibilityPhase::Transition | VisibilityPhase::Showing | VisibilityPhase::Hidden => {
                context
                    .childrens_identifiers_of(self.id)
                    .first()
                    .and_then(|child_widget_id| context.widget_sizing_mode(child_widget_id))
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
        if let Some(WidgetStyle::AnimatedVisibility(av_style)) = context.get_style(&self.class) {
            self.configure(av_style.clone());
        }

        let mut visible = true;
        if let Some(state) = self.visibility_state {
            <C as StateSubscription<WidgetId>>::subscribe(context, self.id, state);

            if let Some(actual_visibility) = context.get(state) {
                visible = *actual_visibility;
            }
        }

        let mut runtime_information = AVRuntimeInformation {
            visible,
            phase: VisibilityPhase::default(),
            animation_state: AnimationState::default(),
        };

        let target_phase = runtime_information.visibility_as_target_phase();
        runtime_information.phase.fix_with_visibility(visible);

        if target_phase != runtime_information.phase {
            runtime_information.animation_state = AnimationState::Play;
            runtime_information.next_phase();

            runtime_information.register_animation(context, self.id, self.animation_properties());
        }

        context.set_widget_data(self.id, Box::new(runtime_information));
    }
}

impl<C> Deinit<C> for AnimatedVisibility
where
    C: DeinitContext,
{
    fn on_deinit(&mut self, context: &mut C) {
        if let Some(state) = self.visibility_state {
            <C as StateSubscription<WidgetId>>::unsubscribe(context, self.id, state);
        }
    }
}

impl<C> Invalidate<C> for AnimatedVisibility
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, _context: &mut C, style: WidgetStyle) {
        if let WidgetStyle::AnimatedVisibility(av_style) = style {
            self.configure(av_style);
        }
    }

    fn on_rebuild(&mut self, context: &mut C) -> crate::stage::invalidate::RebuildStatus {
        let mut runtime_information: Unique<AVRuntimeInformation> =
            widget_data_mut(context, self.id)
                .expect("There's must be an initialized runtime information of AnimatedVisibility");

        if let Some(new_visibility) = self
            .visibility_state
            .and_then(|state| context.get(state))
            .take_if(|new_visibility| **new_visibility != runtime_information.visible)
        {
            runtime_information.visible = *new_visibility;

            match runtime_information.animation_state {
                AnimationState::Nothing => {
                    runtime_information.animation_state = AnimationState::Play;
                    runtime_information.next_phase();

                    runtime_information.register_animation(
                        context,
                        self.id,
                        self.animation_properties(),
                    );
                }
                AnimationState::Play => {
                    runtime_information.animation_state = AnimationState::Unwind;
                    context.reverse_animation(self.id);
                }
                AnimationState::Unwind => {
                    runtime_information.animation_state = AnimationState::Play;
                    context.reverse_animation(self.id);
                }
            }
        }

        if context.is_finished(self.id) {
            runtime_information.next_phase();

            if runtime_information.is_fully_finished() {
                runtime_information.animation_state = AnimationState::Nothing;

                match runtime_information.phase {
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
                runtime_information.register_animation(
                    context,
                    self.id,
                    self.animation_properties(),
                );
            }
        }

        if let VisibilityPhase::SpatialChange = runtime_information.phase {
            RebuildStatus::NeedsMeasure
        } else {
            RebuildStatus::NothingChanged
        }
    }
}

impl<C> Measure<C, f32> for AnimatedVisibility
where
    C: MeasureContext<f32>,
{
    fn intrinsic_content(&self, context: &mut C) -> Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId>,
    {
        context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| context.widget_intrinsic(child_widget_id))
            .unwrap_or_default()
    }

    fn measure_content(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: ManageMeasures<f32, WidgetId>,
    {
        let mut used_extent = context
            .childrens_identifiers_of(self.id)
            .first()
            .and_then(|child_widget_id| context.measure_widget(child_widget_id, constraints))
            .unwrap_or_default();

        let runtime_information: &AVRuntimeInformation = widget_data(context, self.id)
            .expect("There must be an associated widget data for AnimatedVisibility.");

        if let VisibilityPhase::SpatialChange = runtime_information.phase {
            let spatial_change_easing = &self
                .animation_properties()
                .resolve_spatical_change(runtime_information.resolve_animation_appearance())
                .easing;

            used_extent *=
                spatial_change_easing.ease(context.animation_progress(self.id).unwrap_or(1.0));
        }

        if let VisibilityPhase::Hidden = runtime_information.phase {
            used_extent *= 0.0;
        }

        used_extent
    }
}

impl<C> Layout<C, f32> for AnimatedVisibility
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, _context: &mut C) {}
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
        let runtime_information: &AVRuntimeInformation = widget_data(context, self.id)
            .expect("There must be an associated runtime information for AnimatedVisibility.");

        match runtime_information.phase {
            VisibilityPhase::Hidden | VisibilityPhase::SpatialChange => (),
            VisibilityPhase::Transition => {
                if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
                    let animation_definition = self
                        .animation_properties()
                        .resolve_animation(runtime_information.resolve_animation_appearance());
                    let widget_image = drawer.draw_into_offscreen(context, offset, child_widget_id);

                    let child_extent = context.load(*child_widget_id).unwrap_or_default();
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
                if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
                    context.draw_widget(child_widget_id, offset, drawer)
                }
            }
        }
    }
}

impl AnimatedVisibility {
    fn animation_properties(&self) -> AnimationProperties<'_> {
        let enter_animation = self
            .primary_animation
            .as_ref()
            .expect("Primary animation must be set!");
        let exit_animation = self.secondary_animation.as_ref().unwrap_or(enter_animation);

        let enter_spatial_change = self
            .primary_spatial_change
            .as_ref()
            .expect("Primary spatial change must be set!");
        let exit_spatial_change = self
            .secondary_spatial_change
            .as_ref()
            .unwrap_or(enter_spatial_change);

        AnimationProperties {
            enter_animation,
            exit_animation,
            enter_spatial_change,
            exit_spatial_change,
        }
    }
}

impl<C> EventHitTest<C, f32> for AnimatedVisibility
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
            |local_coords: Point<f32>, _provided_extent: Extent<f32>, router: &mut EventRouter| {
                if let Some(hit_result) = context
                    .childrens_identifiers_of(self.id)
                    .first()
                    .and_then(|child_widget_id| {
                        context.hit_test_widget(child_widget_id, local_coords, router)
                    })
                {
                    match hit_result {
                        HitTestResult::Hit => {
                            router.set_next_index(self.id, 0);
                        }
                        HitTestResult::Missed | HitTestResult::Failed => (),
                    }

                    hit_result
                } else {
                    HitTestResult::Missed
                }
            },
        )
        .hoverable()
        .hit_test(self.id, local_coords, provided_extent, router)
    }
}

impl<C> EventHandling<C, f32> for AnimatedVisibility
where
    C: EventContext<f32>,
{
    fn handle_events(
        &mut self,
        context: &mut C,
        pending_events: Vec<PendingEvent>,
        _next_child: usize,
        router: &EventRouter,
    ) {
        for event in pending_events {
            match event {
                PendingEvent::HoverIn => {
                    if let Some(callback) = self.on_hover.as_mut() {
                        callback(ScopedContext::new(context), ());
                    }
                }
                PendingEvent::HoverOut => {
                    if let Some(callback) = self.on_leave.as_mut() {
                        callback(ScopedContext::new(context), ());
                    }
                }
                _ => (),
            }
        }

        if let Some(child_widget_id) = context.childrens_identifiers_of(self.id).first() {
            context.route_events_to_widget(child_widget_id, router);
        }
    }
}
