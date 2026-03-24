use std::collections::HashMap;

use config::display::EasingType;
use skia_safe::runtime_effect::ChildPtr;

use crate::{
    animation::{
        fade::{FadeIn, FadeOut},
        pop::{PopIn, PopOut},
        translate::Translate,
    },
    Draw, Widget,
};

pub mod fade;
pub mod pop;
pub mod translate;

/// A wrapper enum for all supported animation effects.
///
/// This type groups different animations under a single handle, making it
/// easier to manage them dynamically in code.
pub enum AnimatedWidget {
    FadeIn(FadeIn),
    FadeOut(FadeOut),
    PopIn(PopIn),
    PopOut(PopOut),
    Translate(Translate),
}

macro_rules! delegate {
    ($self:ident.$method_name:ident($($tokens:tt),*)) => {
        match $self {
            AnimatedWidget::FadeIn(fade) => fade.$method_name($($tokens),*),
            AnimatedWidget::FadeOut(fade) => fade.$method_name($($tokens),*),
            AnimatedWidget::PopIn(pop) => pop.$method_name($($tokens),*),
            AnimatedWidget::PopOut(pop) => pop.$method_name($($tokens),*),
            AnimatedWidget::Translate(translate) => translate.$method_name($($tokens),*),
        }
    };
}

impl AnimatedWidget {
    /// Swaps out the widget inside the animation.
    pub fn replace_widget(&mut self, widget: Widget) {
        delegate!(self.replace_widget(widget));
    }

    /// Consumes the animation and returns the underlying widget.
    pub fn into_widget(self) -> Widget {
        delegate!(self.into_widget())
    }
}

impl Animated for AnimatedWidget {
    fn is_finished(&self) -> bool {
        delegate!(self.is_finished())
    }

    fn update(&mut self, delta_time_ns: u64) {
        delegate!(self.update(delta_time_ns));
    }
}

impl Draw for AnimatedWidget {
    fn draw_with_offset(
        &self,
        offset: &crate::types::Offset<usize>,
        drawer: &mut crate::drawer::Drawer,
    ) {
        delegate!(self.draw_with_offset(offset, drawer));
    }
}

/// A minimal trait for time-based animations.
pub trait Animated: Draw {
    /// Advances the animation by a given time delta.
    fn update(&mut self, delta_time_ns: u64);

    /// Reports whether the animation has completed.
    fn is_finished(&self) -> bool;
}

macro_rules! from_impl {
    ($variant:ident => $struct:ty) => {
        impl From<$variant> for $struct {
            fn from(value: $variant) -> Self {
                Self::$variant(value)
            }
        }
    };
}

from_impl!(FadeIn => AnimatedWidget);
from_impl!(FadeOut => AnimatedWidget);
from_impl!(PopIn => AnimatedWidget);
from_impl!(PopOut => AnimatedWidget);
from_impl!(Translate => AnimatedWidget);

/// Describes the pacing curve of an animation.
#[derive(Default)]
pub enum Easing {
    /// Progresses at a constant speed.
    Linear,

    /// Starts fast and slows down toward the end.
    EaseOut,

    /// Accelerates in the middle, smooth at start and end.
    #[default]
    EaseInOut,
}

impl Easing {
    fn ease(&self, progress: f32) -> f32 {
        match self {
            Easing::Linear => progress,
            Easing::EaseOut => f32::sin((core::f32::consts::PI / 2.0) * progress),
            Easing::EaseInOut => (1.0 - f32::cos(core::f32::consts::PI * progress)) / 2.0,
        }
    }
}

impl From<EasingType> for Easing {
    fn from(value: EasingType) -> Self {
        match value {
            EasingType::Linear => Easing::Linear,
            EasingType::EaseOut => Easing::EaseOut,
            EasingType::EaseInOut => Easing::EaseInOut,
        }
    }
}

struct ShaderBuilder {
    effect: skia_safe::RuntimeEffect,
    uniforms: HashMap<String, UniformValue>,
}

impl ShaderBuilder {
    fn new(sksl: &str) -> anyhow::Result<Self> {
        let effect = skia_safe::RuntimeEffect::make_for_shader(sksl, None).unwrap();
        Ok(Self {
            effect,
            uniforms: HashMap::new(),
        })
    }

    fn set_uniform(&mut self, name: &str, value: UniformValue) {
        self.uniforms.insert(name.to_string(), value);
    }

    // TODO: implement ShaderBuilderError to have expressive error and handle them
    fn make_shader<U: Into<Option<HashMap<String, UniformValue>>>>(
        &self,
        children: &[ChildPtr],
        additional_uniforms: U,
    ) -> Result<skia_safe::Shader, ShaderBuilderError> {
        let all_uniforms = {
            let mut uniforms = self.uniforms.clone();
            uniforms.extend(additional_uniforms.into().unwrap_or_default());
            uniforms
        };

        if !self.is_uniform_complete(&all_uniforms) {
            return Err(ShaderBuilderError::IncompleteUniforms);
        }

        let mut data = vec![0u8; self.effect.uniform_size()];
        for uniform in self.effect.uniforms() {
            all_uniforms[uniform.name()]
                .write_into(&mut data[uniform.offset()..][..uniform.size_in_bytes()]);
        }

        let sk_data = skia_safe::Data::new_copy(&data);

        self.effect
            .make_shader(sk_data, children, None)
            .ok_or(ShaderBuilderError::ShaderNotBuilt)
    }

    fn is_uniform_complete(&self, all_uniforms: &HashMap<String, UniformValue>) -> bool {
        for uniform in self.effect.uniforms() {
            if all_uniforms
                .get(uniform.name())
                .is_none_or(|uniform_value| uniform_value.ty() != uniform.ty())
            {
                return false;
            }
        }

        true
    }
}

#[derive(Debug)]
enum ShaderBuilderError {
    IncompleteUniforms,
    ShaderNotBuilt,
}

#[allow(unused)]
#[derive(Clone)]
enum UniformValue {
    Float(f32),
    Float2(f32, f32),
    Float3(f32, f32, f32),
    Float4(f32, f32, f32, f32),
    Float2x2([[f32; 2]; 2]),
    Float3x3([[f32; 3]; 3]),
    Float4x4([[f32; 4]; 4]),
    Int(i32),
    Int2(i32, i32),
    Int3(i32, i32, i32),
    Int4(i32, i32, i32, i32),
}

impl UniformValue {
    fn ty(&self) -> skia_safe::runtime_effect::uniform::Type {
        use skia_safe::runtime_effect::uniform::Type;

        match self {
            UniformValue::Float(_) => Type::Float,
            UniformValue::Float2(_, _) => Type::Float2,
            UniformValue::Float3(_, _, _) => Type::Float3,
            UniformValue::Float4(_, _, _, _) => Type::Float4,
            UniformValue::Float2x2(_) => Type::Float2x2,
            UniformValue::Float3x3(_) => Type::Float3x3,
            UniformValue::Float4x4(_) => Type::Float4x4,
            UniformValue::Int(_) => Type::Int,
            UniformValue::Int2(_, _) => Type::Int2,
            UniformValue::Int3(_, _, _) => Type::Int3,
            UniformValue::Int4(_, _, _, _) => Type::Int4,
        }
    }

    fn write_into(&self, data: &mut [u8]) {
        fn copy_array<
            'a,
            Iter: IntoIterator<Item = &'a T>,
            T: LittleEndianBytes<SIZE> + 'a,
            const SIZE: usize,
        >(
            dest: &mut [u8],
            source: Iter,
        ) {
            dest.copy_from_slice(
                &source
                    .into_iter()
                    .flat_map(|val| val.little_endian_bytes())
                    .collect::<Vec<u8>>(),
            );
        }

        fn copy_matrix<
            T: LittleEndianBytes<BYTE_SIZE>,
            const BYTE_SIZE: usize,
            const SIZE: usize,
        >(
            dest: &mut [u8],
            source: &[[T; SIZE]],
        ) {
            copy_array(dest, source.iter().flat_map(<&[T; SIZE]>::into_iter));
        }

        match *self {
            UniformValue::Float(x) => copy_array(data, &[x]),
            UniformValue::Float2(x, y) => copy_array(data, &[x, y]),
            UniformValue::Float3(x, y, z) => copy_array(data, &[x, y, z]),
            UniformValue::Float4(x, y, z, h) => copy_array(data, &[x, y, z, h]),
            UniformValue::Float2x2(matrix) => copy_matrix(data, &matrix),
            UniformValue::Float3x3(matrix) => copy_matrix(data, &matrix),
            UniformValue::Float4x4(matrix) => copy_matrix(data, &matrix),
            UniformValue::Int(x) => copy_array(data, &[x]),
            UniformValue::Int2(x, y) => copy_array(data, &[x, y]),
            UniformValue::Int3(x, y, z) => copy_array(data, &[x, y, z]),
            UniformValue::Int4(x, y, z, h) => copy_array(data, &[x, y, z, h]),
        }
    }
}

trait LittleEndianBytes<const SIZE: usize> {
    fn little_endian_bytes(&self) -> [u8; SIZE];
}

impl LittleEndianBytes<4> for f32 {
    fn little_endian_bytes(&self) -> [u8; 4] {
        self.to_le_bytes()
    }
}

impl LittleEndianBytes<4> for i32 {
    fn little_endian_bytes(&self) -> [u8; 4] {
        self.to_le_bytes()
    }
}
