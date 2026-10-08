pub mod animations;
pub mod context;
pub mod decorator;
pub mod events;
pub mod forest;
pub mod stage;
pub mod state;
pub mod types;
pub mod widget;

use crate::{
    context::{
        Context, CreateState, DebugOptions, GetState, LoadExtent, ManageDirtyFlags, SetState, Tick,
    },
    events::{EventManager, RawEvent},
    stage::{draw::DrawContext, measure::Constraints, rebuild::RebuildTree},
    types::{dirty_flags::DirtyFlags, Extent, Point, WidgetId},
};

pub struct WidgetSystem {
    event_manager: EventManager,
    pub context: Context,
    cached_constraints: Constraints<Extent<f32>>,
}

impl WidgetSystem {
    pub fn new(context: Context) -> Self {
        Self {
            event_manager: EventManager::new(),
            context,
            cached_constraints: Constraints::new_tight(Extent::default()),
        }
    }

    pub fn update_debug_options(&mut self, debug_options: DebugOptions) {
        self.context.update_debug_options(debug_options);
    }

    pub fn width(&self) -> f32 {
        self.context
            .main_tree_root()
            .and_then(|root_id| {
                <Context as LoadExtent<f32, WidgetId>>::load(&self.context, *root_id)
            })
            .unwrap_or_default()
            .width
    }

    pub fn height(&self) -> f32 {
        self.context
            .main_tree_root()
            .and_then(|root_id| {
                <Context as LoadExtent<f32, WidgetId>>::load(&self.context, *root_id)
            })
            .unwrap_or_default()
            .height
    }

    pub fn set_constraints(&mut self, constraints: Constraints<Extent<f32>>) {
        if self.cached_constraints != constraints {
            self.cached_constraints = constraints;

            if let Some(root_id) = self.context.main_tree_root() {
                self.context.set_dirty_flags(
                    *root_id,
                    self.context.get_dirty_flags(*root_id) | DirtyFlags::NEEDS_MEASURE,
                );
            }
        }
    }

    pub fn update(&mut self) {
        if self.context.has_pending_tree_root() {
            self.context.rebuild_tree();
        }

        if self.context.has_invalid_widgets() {
            self.context.invalidate_widgets();
        }

        if self.context.needs_measurement() {
            self.context.measure_widgets(self.cached_constraints);
        }

        if self.context.needs_layout() {
            self.context.layout_widgets();
        }
    }

    pub fn draw(&self, offset: &types::Offset<f32>, drawer: &mut stage::draw::Drawer) {
        if let Some(root_id) = self.context.main_tree_root() {
            self.context.draw_widget(root_id, offset, drawer);
        }
    }

    pub fn dispatch_event(&mut self, event: RawEvent) {
        if let Some(root_id) = self.context.main_tree_root().copied() {
            self.event_manager
                .dispatch_event(&mut self.context, event, &root_id);
        }
    }

    pub fn resolve_pointer_shape(&self) -> events::PointerShape {
        self.event_manager.resolve_pointer_shape()
    }

    pub fn collect_input_regions(&self) -> Vec<(Point<f32>, Extent<f32>)> {
        self.context.collect_input_regions()
    }
}

impl<T> CreateState<T> for WidgetSystem {
    fn create_state(&mut self, value: T) -> state::State<T> {
        self.context.create_state(value)
    }

    fn create_state_mut(&mut self, value: T) -> state::MutableState<T> {
        self.context.create_state_mut(value)
    }
}

impl GetState for WidgetSystem {
    fn get<T, S>(&self, state: S) -> Option<&T>
    where
        T: 'static,
        S: Into<state::State<T>>,
    {
        self.context.get(state)
    }
}

impl SetState for WidgetSystem {
    fn set<T: 'static>(&mut self, mutable_state: state::MutableState<T>, value: T) {
        self.context.set(mutable_state, value);
    }
}

impl Tick for WidgetSystem {
    fn tick(&mut self, delta_ns: u128) {
        self.context.tick(delta_ns);
    }
}
