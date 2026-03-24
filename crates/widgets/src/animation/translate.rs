use std::{collections::HashMap, time::Duration};

use skia_safe::runtime_effect::ChildPtr;

use crate::{
    animation::{Animated, Easing, ShaderBuilder, UniformValue},
    Draw, Widget,
};

/// A translation animation effect for widgets.
///
/// This effect smoothly moves a widget from its start position to an end position.
/// Unlike [`super::fade::Fade`] and [`super::pop::Pop`], translation has no direction type: it always
/// progresses from start → end. The movement curve is determined by the [`Easing`] function.
///
/// The start and end points are stored in the underlying shader builder, so
/// the animation struct itself only tracks timing and easing.
pub struct Translate {
    widget: Widget,

    shader_builder: ShaderBuilder,
    current_time_ns: u64,
    duration: Duration,
    easing: Easing,
}

/// A simple 2D point used in translation effects.
///
/// Represents an `(x, y)` offset in logical coordinates.
pub struct Point {
    x: f32,
    y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

const SHADER: &str = r#"
uniform shader image;
uniform float progress;

uniform float2 start;
uniform float2 end;

uniform float2 offset;
uniform float2 resolution;

half4 main(float2 coord) {
    float2 translation_offset = (start - end) * progress;
    float2 translated = coord - start + translation_offset;

    float2 bottom_right_corner = offset + resolution;
    if (translated.x < offset.x
        || translated.y < offset.y
        || translated.x > bottom_right_corner.x
        || translated.y > bottom_right_corner.y
    ) {
        return half4(0);
    }

    return image.eval(translated);
}
"#;

impl Translate {
    pub fn new(
        widget: Widget,
        start: Point,
        end: Point,
        duration: Duration,
        easing: Easing,
    ) -> Self {
        Self {
            widget,
            shader_builder: {
                let mut builder = ShaderBuilder::new(SHADER).unwrap();
                builder.set_uniform("progress", UniformValue::Float(0.0));
                builder.set_uniform("start", UniformValue::Float2(start.x, start.y));
                builder.set_uniform("end", UniformValue::Float2(end.x, end.y));
                builder
            },
            current_time_ns: 0,
            easing,
            duration,
        }
    }

    pub fn replace_widget(&mut self, widget: Widget) {
        self.widget = widget;
    }

    pub fn into_widget(self) -> Widget {
        self.widget
    }
}

impl Animated for Translate {
    fn update(&mut self, delta_time_ns: u64) {
        if self.is_finished() {
            return;
        }

        self.current_time_ns += delta_time_ns;
        self.shader_builder.set_uniform(
            "progress",
            UniformValue::Float(self.easing.ease(
                (self.current_time_ns as f64 / self.duration.as_nanos() as f64).clamp(0.0, 1.0)
                    as f32,
            )),
        );
    }

    fn is_finished(&self) -> bool {
        self.current_time_ns as u128 > self.duration.as_nanos()
    }
}

impl Draw for Translate {
    fn draw_with_offset(
        &self,
        offset: &crate::types::Offset<usize>,
        drawer: &mut crate::drawer::Drawer,
    ) {
        let image = drawer.draw_into_offscreen(offset, &self.widget);

        let shader = self
            .shader_builder
            .make_shader(
                &[ChildPtr::Shader(
                    image
                        .to_shader(None, skia_safe::FilterMode::Linear, None)
                        .expect("A Skia image must have a possibility to convert into a shader"),
                )],
                HashMap::from([
                    (
                        "offset".to_string(),
                        UniformValue::Float2(offset.x as f32, offset.y as f32),
                    ),
                    (
                        "resolution".to_string(),
                        UniformValue::Float2(
                            self.widget.width() as f32,
                            self.widget.height() as f32,
                        ),
                    ),
                ]),
            )
            .expect("A shader must be valid to build");

        let mut paint = skia_safe::Paint::default();
        paint.set_shader(shader);
        drawer.surface.canvas().draw_paint(&paint);
    }
}
