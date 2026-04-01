use crate::{
    events::{Action, DispatchEvent},
    types::{constraints::Constraints, Extent2D},
    Compile, CompileResult, Draw, WidgetInfo,
};

/// A zero-sized, "no-op" widget used as a structural placeholder.
///
/// The `Unknown` widget represents a state where a widget is either
/// missing, failed to compile, or serves as a functional "stub." Because
/// it is a Zero-Sized Type (ZST), it occupies no memory and performs
/// no actual rendering.
///
/// **Design Intent:**
/// This widget exists primarily to simplify the `Widget` enum logic. By
/// providing a valid widget that "does nothing," the layout engine and
/// declarative macros can treat every variant uniformly. Instead of
/// checking for null pointers or empty variants, the system can
/// safely call `compile()` or `draw()` on an `Unknown` widget,
/// knowing it will simply return constant "zero" values.
#[derive(Clone)]
pub struct Unknown;

impl WidgetInfo for Unknown {
    fn get_type(&self) -> &'static str {
        "unknown"
    }

    /// Returns a constant value of 0.
    ///
    /// For the purposes of layout math (like sums in a `FlexContainer`),
    /// this widget contributes nothing to the total dimensions,
    /// effectively making it invisible to its parent containers.
    fn width(&self) -> usize {
        0
    }

    /// Returns a constant value of 0.
    ///
    /// For the purposes of layout math (like sums in a `FlexContainer`),
    /// this widget contributes nothing to the total dimensions,
    /// effectively making it invisible to its parent containers.
    fn height(&self) -> usize {
        0
    }

    /// Always returns [`SizingMode::Fixed`] value.
    ///
    /// For the purposes of layout math (like sums in a `FlexContainer`),
    /// this widget contributes nothing to the total dimensions,
    /// effectively making it invisible to its parent containers.
    fn sizing_mode(&self) -> crate::types::constraints::SizingMode {
        crate::types::constraints::SizingMode::Fixed
    }
}

impl Compile for Unknown {
    /// Always returns a successful "stub" state.
    ///
    /// Since an unknown widget has no content and no requirements, it
    /// technically "fits" everywhere. It reports a size of zero and
    /// completes instantly, ensuring the rest of the layout remains
    /// stable and unaffected.
    fn compile(
        &mut self,
        _constraints: Constraints<f32>,
        _compile_ctx: &mut crate::CompileCtx,
    ) -> crate::CompileResult {
        CompileResult::Success {
            used_extent: Extent2D::new(0., 0.),
        }
    }
}

impl Draw for Unknown {
    /// Performs no operations on the Skia canvas.
    ///
    /// Calling draw on this widget is a "no-op" (no operation). It
    /// consumes no GPU resources and leaves the canvas exactly
    /// as it found it.
    fn draw_with_offset(
        &self,
        _offset: &crate::types::offset::Offset<usize>,
        _drawer: &mut crate::drawer::Drawer,
    ) {
        // Do nothing
    }
}

impl DispatchEvent for Unknown {
    fn dispatch_event(&self, _event: crate::events::Event) -> crate::events::Action {
        Action::None
    }
}
