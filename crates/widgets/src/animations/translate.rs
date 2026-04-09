use std::collections::HashMap;

use skia_safe::runtime_effect::ChildPtr;

use crate::{
    animations::AnimationFilter,
    types::{Point, ShaderBuilder, UniformValue},
};

/// A translation animation effect for widgets.
///
/// This effect smoothly moves a widget from its start position to an end position.
/// Unlike [`super::fade::Fade`] and [`super::pop::Pop`], translation has no direction type: it always
/// progresses from start → end. The movement curve is determined by the [`Easing`] function.
///
/// The start and end points are stored in the underlying shader builder, so
/// the animation struct itself only tracks timing and easing.
#[derive(Clone)]
pub struct Translate {
    shader_builder: ShaderBuilder,
}

const SHADER: &str = r#"
uniform shader image;
uniform float progress;

uniform float2 start;
uniform float2 end;

uniform float2 offset;
uniform float2 extent;

half4 main(float2 coord) {
    float2 translation_offset = (start - end) * progress;
    float2 translated = coord - start + translation_offset;

    float2 bottom_right_corner = offset + extent;
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
    pub fn new(start: Point<f32>, end: Point<f32>) -> Self {
        Self {
            shader_builder: {
                let mut builder = ShaderBuilder::new(SHADER).unwrap();
                builder.set_uniform("start", UniformValue::Float2(start.x, start.y));
                builder.set_uniform("end", UniformValue::Float2(end.x, end.y));
                builder
            },
        }
    }

    pub fn update_path(&mut self, start: Point<f32>, end: Point<f32>) {
        self.shader_builder
            .set_uniform("start", UniformValue::Float2(start.x, start.y));
        self.shader_builder
            .set_uniform("end", UniformValue::Float2(end.x, end.y));
    }
}

impl Default for Translate {
    fn default() -> Self {
        Self::new(Point::default(), Point::default())
    }
}

impl AnimationFilter for Translate {
    fn filter(
        &self,
        widget_image: skia_safe::Image,
        offset: crate::types::Offset<f32>,
        extent: crate::types::Extent<f32>,
        progress: f32,
    ) -> skia_safe::Paint {
        let shader = self
            .shader_builder
            .make_shader(
                &[ChildPtr::Shader(
                    widget_image
                        .to_shader(None, skia_safe::FilterMode::Linear, None)
                        .expect("A Skia image must have a possibility to convert into a shader"),
                )],
                HashMap::from([
                    ("progress".to_string(), UniformValue::Float(progress)),
                    (
                        "offset".to_string(),
                        UniformValue::Float2(offset.x, offset.y),
                    ),
                    (
                        "extent".to_string(),
                        UniformValue::Float2(extent.width, extent.height),
                    ),
                ]),
            )
            .expect("A shader must be valid to build");

        let mut paint = skia_safe::Paint::default();
        paint.set_shader(shader);
        paint
    }
}
