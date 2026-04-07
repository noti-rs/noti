use skia_safe::runtime_effect::ChildPtr;
use std::collections::HashMap;

use crate::animations::AnimationFilter;
use crate::types::{ShaderBuilder, UniformValue};

/// A pop animation effect for widgets.
///
/// Similar to [`super::fade::Fade`], but instead of changing transparency,  
/// this effect scales the widget to create a "popping" motion.  
///
/// The direction of scaling is controlled by the [`PopDirection`] trait,  
/// and the transition curve is determined by the [`Easing`] function.
#[derive(Clone)]
pub struct Pop {
    shader_builder: ShaderBuilder,
}

const SHADER: &str = r#"
uniform shader image;
uniform float progress;
uniform float2 offset;
uniform float2 extent;

half4 main(float2 coord) {
    float2 center = offset + extent / 2.0;
    float2 delta = coord - center;
    float2 scaled = delta / progress + center;

    // --- discard pixels outside scaled area ---
    // (optional, avoids sampling outside widget texture)
    float2 bottom_right_corner = offset + extent;
    if (scaled.x < offset.x || scaled.y < offset.y ||
        scaled.x > bottom_right_corner.x ||
        scaled.y > bottom_right_corner.y) {
        return half4(0);
    }
    
    return image.eval(scaled);
}
"#;

impl Pop {
    pub fn new() -> Self {
        Self {
            shader_builder: ShaderBuilder::new(SHADER).unwrap(),
        }
    }
}

impl Default for Pop {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationFilter for Pop {
    fn filter(
        &self,
        widget_image: skia_safe::Image,
        offset: crate::types::Offset<f32>,
        extent: crate::types::Extent<f32>,
        progress: f32,
    ) -> skia_safe::Paint {
        let additional_uniforms = HashMap::from([
            ("progress".to_string(), UniformValue::Float(progress)),
            (
                "offset".to_string(),
                UniformValue::Float2(offset.x, offset.y),
            ),
            (
                "extent".to_string(),
                UniformValue::Float2(extent.width, extent.height),
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
        paint
    }
}
