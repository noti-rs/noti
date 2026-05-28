mod fade;
mod pop;
mod translate;

use crate::types::{Extent, Offset};
pub use {fade::Fade, pop::Pop, translate::Translate};

pub(crate) trait AnimationFilter {
    fn filter(
        &self,
        widget_image: skia_safe::Image,
        offset: Offset<f32>,
        extent: Extent<f32>,
        progress: f32,
    ) -> skia_safe::Paint;
}

pub trait Direction {
    fn map(progress: f32) -> f32;
}

pub struct Forward;
pub struct Reverse;

impl Direction for Forward {
    fn map(progress: f32) -> f32 {
        progress
    }
}

impl Direction for Reverse {
    fn map(progress: f32) -> f32 {
        1.0 - progress
    }
}

#[derive(Debug, Clone)]
pub enum AnimationKind {
    Fade(Fade),
    Pop(Pop),
    Translate(Translate),
}

impl Default for AnimationKind {
    fn default() -> Self {
        AnimationKind::Fade(Default::default())
    }
}

impl AnimationFilter for AnimationKind {
    fn filter(
        &self,
        widget_image: skia_safe::Image,
        offset: Offset<f32>,
        extent: Extent<f32>,
        progress: f32,
    ) -> skia_safe::Paint {
        match self {
            AnimationKind::Fade(fade) => fade.filter(widget_image, offset, extent, progress),
            AnimationKind::Pop(pop) => pop.filter(widget_image, offset, extent, progress),
            AnimationKind::Translate(translate) => {
                translate.filter(widget_image, offset, extent, progress)
            }
        }
    }
}

/// Describes the pacing curve of an animation.
#[derive(Debug, Default, Clone)]
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
    pub(crate) fn ease(&self, progress: f32) -> f32 {
        match self {
            Easing::Linear => progress,
            Easing::EaseOut => f32::sin((core::f32::consts::PI / 2.0) * progress),
            Easing::EaseInOut => (1.0 - f32::cos(core::f32::consts::PI * progress)) / 2.0,
        }
    }
}
