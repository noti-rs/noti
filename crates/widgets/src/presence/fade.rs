use skia_safe::runtime_effect::ChildPtr;
use std::time::Duration;

use super::{Animated, ShaderBuilder, UniformValue};
use crate::{
    presence::{Direction, Easing, Forward, Reverse},
    widget::{Draw, Widget},
};

/// A fade animation effect for widgets.
///
/// This effect gradually changes the alpha channel of a widget’s colors,
/// producing a smooth fade-in or fade-out transition.  
///
/// The direction of fading (in or out) is controlled by the [`FadeDirection`] trait,  
/// with type aliases [`FadeIn`] and [`FadeOut`] for convenience.  
/// The transition speed is influenced by the chosen [`Easing`] function.
pub struct Fade<D: Direction> {
    shader_builder: ShaderBuilder,
    _marker: std::marker::PhantomData<D>,
}

pub type FadeIn = Fade<Forward>;
pub type FadeOut = Fade<Reverse>;

const SHADER: &str = r#"
uniform shader image;
uniform float alpha;

half4 main(float2 coord) {
    return image.eval(coord) * alpha;
}
"#;

impl<D: Direction> Fade<D> {
    fn new(progress: f32) -> Self {
        Self {
            shader_builder: {
                let mut shader_builder = ShaderBuilder::new(SHADER).unwrap();
                shader_builder.set_uniform("alpha", UniformValue::Float(D::map(progress)));
                shader_builder
            },
            _marker: std::marker::PhantomData,
        }
    }
}

// impl<D: FadeDirection> Animated for Fade<D> {
//     fn update(&mut self, delta_time_ns: u64) {
//         if self.is_finished() {
//             return;
//         }
//
//         self.elapsed_ns += delta_time_ns;
//         self.shader_builder.set_uniform(
//             "alpha",
//             UniformValue::Float(
//                 self.easing
//                     .ease(D::alpha_value(self.elapsed_ns, self.duration)),
//             ),
//         );
//     }
//
//     fn is_finished(&self) -> bool {
//         self.elapsed_ns as u128 > self.duration.as_nanos()
//     }
// }
//
// impl<D: FadeDirection> Draw for Fade<D> {
//     fn draw_with_offset(
//         &self,
//         offset: &crate::types::offset::Offset<f32>,
//         drawer: &mut crate::drawer::Drawer,
//     ) {
//         let widget_image = drawer.draw_into_offscreen(offset, &self.widget);
//
//         let shader = self
//             .shader_builder
//             .make_shader(
//                 &[ChildPtr::Shader(
//                     widget_image
//                         .to_shader(None, skia_safe::FilterMode::Linear, None)
//                         .expect("A Skia image must have a possibility to convert into a shader"),
//                 )],
//                 None,
//             )
//             .expect("A shader must be valid to build");
//
//         let mut paint = skia_safe::Paint::default();
//         paint.set_shader(shader);
//         drawer.surface.canvas().draw_paint(&paint);
//     }
// }
