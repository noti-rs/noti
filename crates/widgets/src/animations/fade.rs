use std::collections::HashMap;

use skia_safe::runtime_effect::ChildPtr;

use crate::{
    animations::AnimationFilter,
    types::{ShaderBuilder, UniformValue},
};

/// A fade animation effect for widgets.
///
/// This effect gradually changes the alpha channel of a widget’s colors,
/// producing a smooth fade-in or fade-out transition.  
///
/// The direction of fading (in or out) is controlled by the [`FadeDirection`] trait,  
/// with type aliases [`FadeIn`] and [`FadeOut`] for convenience.  
/// The transition speed is influenced by the chosen [`Easing`] function.
#[derive(Debug, Clone)]
pub struct Fade {
    shader_builder: ShaderBuilder,
}

const SHADER: &str = r#"
uniform shader image;
uniform float alpha;

half4 main(float2 coord) {
    return image.eval(coord) * alpha;
}
"#;

impl Fade {
    pub fn new() -> Self {
        Self {
            shader_builder: ShaderBuilder::new(SHADER).unwrap(),
        }
    }
}

impl Default for Fade {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationFilter for Fade {
    fn filter(
        &self,
        widget_image: skia_safe::Image,
        _offset: crate::types::Offset<f32>,
        _extent: crate::types::Extent<f32>,
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
                HashMap::from([("alpha".to_string(), UniformValue::Float(progress))]),
            )
            .expect("A shader must be valid to build");

        let mut paint = skia_safe::Paint::default();
        paint.set_shader(shader);

        paint
    }
}
