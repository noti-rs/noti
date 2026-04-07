use std::collections::HashMap;

use skia_safe::runtime_effect::ChildPtr;

pub struct ShaderBuilder {
    sksl: String,
    effect: skia_safe::RuntimeEffect,
    uniforms: HashMap<String, UniformValue>,
}

impl ShaderBuilder {
    pub fn new(sksl: &str) -> anyhow::Result<Self> {
        let effect = skia_safe::RuntimeEffect::make_for_shader(sksl, None)
            .map_err(|err| anyhow::anyhow!("Failed to make RuntimeEffect. Error value: {err}"))?;

        Ok(Self {
            sksl: sksl.to_owned(),
            effect,
            uniforms: HashMap::new(),
        })
    }

    pub fn set_uniform(&mut self, name: &str, value: UniformValue) {
        self.uniforms.insert(name.to_string(), value);
    }

    // TODO: implement ShaderBuilderError to have expressive error and handle them
    pub fn make_shader<U: Into<Option<HashMap<String, UniformValue>>>>(
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

    pub fn is_uniform_complete(&self, all_uniforms: &HashMap<String, UniformValue>) -> bool {
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
pub enum ShaderBuilderError {
    IncompleteUniforms,
    ShaderNotBuilt,
}

impl Clone for ShaderBuilder {
    fn clone(&self) -> Self {
        Self::new(&self.sksl).unwrap()
    }
}

#[allow(unused)]
#[derive(Clone)]
pub enum UniformValue {
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
