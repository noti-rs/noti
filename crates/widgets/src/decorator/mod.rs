use std::ops::{Add, Sub};

use num_traits::FromPrimitive;

use crate::{
    decorator::{
        background::BackgroundDecorator,
        border::BorderDecorator,
        box_size::BoxSizeDecorator,
        callback::{EventDecorator, Clickable, Hoverable, Pressable},
        spacing::SpacingDecorator,
    },
    events::{EventRouter, HitTestResult},
    stage::{
        draw::Drawer,
        measure::{Constraints, Intrinsic},
    },
    types::{Border, Color, Extent, Offset, Point, Spacing, WidgetId},
};

pub(crate) mod background;
pub(crate) mod border;
pub(crate) mod box_size;
pub(crate) mod callback;
pub(crate) mod content;
pub(crate) mod spacing;

pub(crate) trait DecoratorType:
    Default + Copy + FromPrimitive + PartialOrd + Add<Output = Self> + Sub<Output = Self>
{
}

impl<T> DecoratorType for T where
    T: Default + Copy + FromPrimitive + PartialOrd + Add<Output = Self> + Sub<Output = Self>
{
}

pub(crate) trait MeasureDecorator<T>
where
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T>;
    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T>;
}

pub(crate) trait DrawDecorator<T>
where
    T: DecoratorType,
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer);
}

pub(crate) trait EventHitTestDecorator<T>
where
    T: DecoratorType,
{
    fn on_hit_test(
        &self,
        widget_id: WidgetId,
        local_coords: Point<T>,
        provided_extent: Extent<T>,
        router: &mut EventRouter,
    ) -> HitTestResult;

    fn hit_test(
        &self,
        widget_id: WidgetId,
        local_coords: Point<T>,
        provided_extent: Extent<T>,
        router: &mut EventRouter,
    ) -> HitTestResult {
        if !local_coords.is_inside_of(provided_extent) {
            return HitTestResult::Missed;
        }

        self.on_hit_test(widget_id, local_coords, provided_extent, router)
    }
}

pub(crate) trait DecoratorExt<T>:
    MeasureDecorator<T> + DrawDecorator<T> + EventHitTestDecorator<T> + Sized
where
    T: DecoratorType,
{
    fn spacing(self, spacing: Spacing) -> SpacingDecorator<Self> {
        SpacingDecorator {
            spacing,
            next: self,
        }
    }

    fn border(self, border: Border) -> BorderDecorator<Self> {
        BorderDecorator { border, next: self }
    }

    fn box_size<S: Into<Option<T>>>(self, width: S, height: S) -> BoxSizeDecorator<Self, T> {
        BoxSizeDecorator {
            width: width.into(),
            height: height.into(),
            ratio: None,
            next: self,
        }
    }

    fn box_size_with_ratio<S: Into<Option<T>>, R: Into<Option<f32>>>(
        self,
        width: S,
        height: S,
        ratio: R,
    ) -> BoxSizeDecorator<Self, T> {
        BoxSizeDecorator {
            width: width.into(),
            height: height.into(),
            ratio: ratio.into(),
            next: self,
        }
    }

    fn background(self, background_color: Color) -> BackgroundDecorator<Self> {
        BackgroundDecorator {
            background_color,
            next: self,
        }
    }

    fn hoverable(self) -> EventDecorator<Hoverable, Self> {
        EventDecorator::hoverable(self)
    }

    fn pressable(self) -> EventDecorator<Pressable, Self> {
        EventDecorator::pressable(self)
    }

    fn clickable(self) -> EventDecorator<Clickable, Self> {
        EventDecorator::clickable(self)
    }
}

impl<D, T> DecoratorExt<T> for D
where
    D: MeasureDecorator<T> + DrawDecorator<T> + EventHitTestDecorator<T> + Sized,
    T: DecoratorType,
{
}
