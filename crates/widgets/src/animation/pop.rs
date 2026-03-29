use skia_safe::runtime_effect::ChildPtr;
use std::{collections::HashMap, time::Duration};

use super::{Animated, ShaderBuilder, UniformValue};
use crate::{animation::Easing, Draw, Widget};

pub trait PopDirection {
    fn percentage(elapsed_ns: u64, duration: Duration) -> f32;
}

pub struct In;
pub struct Out;

impl PopDirection for In {
    fn percentage(elapsed_ns: u64, duration: Duration) -> f32 {
        (elapsed_ns as f64 / duration.as_nanos() as f64).clamp(0.0, 1.0) as f32
    }
}

impl PopDirection for Out {
    fn percentage(elapsed_ns: u64, duration: Duration) -> f32 {
        1.0 - (elapsed_ns as f64 / duration.as_nanos() as f64).clamp(0.0, 1.0) as f32
    }
}

/// A pop animation effect for widgets.
///
/// Similar to [`super::fade::Fade`], but instead of changing transparency,  
/// this effect scales the widget to create a "popping" motion.  
///
/// The direction of scaling is controlled by the [`PopDirection`] trait,  
/// and the transition curve is determined by the [`Easing`] function.
pub struct Pop<Direction: PopDirection> {
    widget: Widget,
    elapsed_ns: u64,
    duration: Duration,
    easing: Easing,
    shader_builder: ShaderBuilder,
    _marker: std::marker::PhantomData<Direction>,
}

pub type PopIn = Pop<In>;
pub type PopOut = Pop<Out>;

const SHADER: &str = r#"
uniform shader image;
uniform float progress;
uniform float2 offset;
uniform float2 resolution;

half4 main(float2 coord) {
    float2 center = offset + resolution / 2.0;
    float2 delta = coord - center;
    float2 scaled = delta / progress + center;

    // --- discard pixels outside scaled area ---
    // (optional, avoids sampling outside widget texture)
    float2 bottom_right_corner = offset + resolution;
    if (scaled.x < offset.x || scaled.y < offset.y ||
        scaled.x > bottom_right_corner.x ||
        scaled.y > bottom_right_corner.y) {
        return half4(0);
    }
    
    return image.eval(scaled);
}
"#;

impl<D: PopDirection> Pop<D> {
    pub fn new(widget: Widget, duration: Duration, easing: Easing) -> Self {
        Self {
            widget,
            duration,
            elapsed_ns: 0,
            shader_builder: {
                let mut builder = ShaderBuilder::new(SHADER).unwrap();
                builder.set_uniform(
                    "progress",
                    UniformValue::Float(easing.ease(D::percentage(0, duration))),
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

impl<D: PopDirection> Animated for Pop<D> {
    fn update(&mut self, delta_time_ns: u64) {
        if self.is_finished() {
            return;
        }

        self.elapsed_ns += delta_time_ns;
        self.shader_builder.set_uniform(
            "progress",
            UniformValue::Float(
                self.easing
                    .ease(D::percentage(self.elapsed_ns, self.duration)),
            ),
        );
    }

    fn is_finished(&self) -> bool {
        self.elapsed_ns as u128 > self.duration.as_nanos()
    }
}

impl<D: PopDirection> Draw for Pop<D> {
    fn draw_with_offset(
        &self,
        offset: &crate::types::offset::Offset<usize>,
        drawer: &mut crate::drawer::Drawer,
    ) {
        let widget_image = drawer.draw_into_offscreen(offset, &self.widget);

        let additional_uniforms = HashMap::from([
            (
                "offset".to_string(),
                UniformValue::Float2(offset.x as f32, offset.y as f32),
            ),
            (
                "resolution".to_string(),
                UniformValue::Float2(self.widget.width() as f32, self.widget.height() as f32),
            ),
        ]);

        let shader = self
            .shader_builder
            .make_shader(
                &[ChildPtr::Shader(
                    widget_image
                        .to_shader(None, skia_safe::FilterMode::Linear, None)
                        .expect("A Skia image must have a possibility to convert into a shader"),
                )],
                additional_uniforms,
            )
            .expect("A shader must be valid to build");

        let mut paint = skia_safe::Paint::default();
        paint.set_shader(shader);
        drawer.surface.canvas().draw_paint(&paint);
    }
}
