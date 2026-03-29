use skia_safe::runtime_effect::ChildPtr;
use std::time::Duration;

use super::{Animated, ShaderBuilder, UniformValue};
use crate::{animation::Easing, Draw, Widget};

pub trait FadeDirection {
    fn alpha_value(elapsed_ns: u64, duration: Duration) -> f32;
}

pub struct In;
pub struct Out;

impl FadeDirection for In {
    fn alpha_value(elapsed_ns: u64, duration: Duration) -> f32 {
        (elapsed_ns as f64 / duration.as_nanos() as f64).clamp(0.0, 1.0) as f32
    }
}

impl FadeDirection for Out {
    fn alpha_value(elapsed_ns: u64, duration: Duration) -> f32 {
        1.0 - (elapsed_ns as f64 / duration.as_nanos() as f64).clamp(0.0, 1.0) as f32
    }
}

/// A fade animation effect for widgets.
///
/// This effect gradually changes the alpha channel of a widget’s colors,
/// producing a smooth fade-in or fade-out transition.  
/// 
/// The direction of fading (in or out) is controlled by the [`FadeDirection`] trait,  
/// with type aliases [`FadeIn`] and [`FadeOut`] for convenience.  
/// The transition speed is influenced by the chosen [`Easing`] function.
pub struct Fade<Direction: FadeDirection> {
    widget: Widget,
    elapsed_ns: u64,
    duration: Duration,
    easing: Easing,
    shader_builder: ShaderBuilder,
    _marker: std::marker::PhantomData<Direction>,
}

pub type FadeIn = Fade<In>;
pub type FadeOut = Fade<Out>;

const SHADER: &str = r#"
uniform shader image;
uniform float alpha;

half4 main(float2 coord) {
    return image.eval(coord) * alpha;
}
"#;

impl<D: FadeDirection> Fade<D> {
    pub fn new(widget: Widget, duration: Duration, easing: Easing) -> Self {
        Self {
            widget,
            duration,
            elapsed_ns: 0,
            shader_builder: {
                let mut builder = ShaderBuilder::new(SHADER).unwrap();
                builder.set_uniform(
                    "alpha",
                    UniformValue::Float(easing.ease(D::alpha_value(0, duration))),
                );
                builder
            },
            easing,
            _marker: std::marker::PhantomData,
        }
    }

    /// Ensure that the [`Widget`] is compiled. Otherwise further actions will be invalid.
    pub(super) fn replace_widget(&mut self, widget: Widget) {
        self.widget = widget;
    }

    pub(super) fn into_widget(self) -> Widget {
        self.widget
    }

    pub(super) fn as_widget(&self) -> &Widget {
        &self.widget
    }
}

impl<D: FadeDirection> Animated for Fade<D> {
    fn update(&mut self, delta_time_ns: u64) {
        if self.is_finished() {
            return;
        }

        self.elapsed_ns += delta_time_ns;
        self.shader_builder.set_uniform(
            "alpha",
            UniformValue::Float(
                self.easing
                    .ease(D::alpha_value(self.elapsed_ns, self.duration)),
            ),
        );
    }

    fn is_finished(&self) -> bool {
        self.elapsed_ns as u128 > self.duration.as_nanos()
    }
}

impl<D: FadeDirection> Draw for Fade<D> {
    fn draw_with_offset(
        &self,
        offset: &crate::types::offset::Offset<usize>,
        drawer: &mut crate::drawer::Drawer,
    ) {
        let widget_image = drawer.draw_into_offscreen(offset, &self.widget);

        let shader = self
            .shader_builder
            .make_shader(
                &[ChildPtr::Shader(
                    widget_image
                        .to_shader(None, skia_safe::FilterMode::Linear, None)
                        .expect("A Skia image must have a possibility to convert into a shader"),
                )],
                None,
            )
            .expect("A shader must be valid to build");

        let mut paint = skia_safe::Paint::default();
        paint.set_shader(shader);
        drawer.surface.canvas().draw_paint(&paint);
    }
}
