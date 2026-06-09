use crate::{
    decorator::{DecoratorType, DrawDecorator, EventHitTestDecorator, MeasureDecorator},
    events::{EventRouter, HitTestResult},
    stage::{
        draw::{Drawer, UseColor},
        measure::{Constraints, Intrinsic},
    },
    types::{Border, Extent, Offset, Point, Spacing, WidgetId},
};

pub(crate) struct BorderDecorator<N> {
    pub(super) border: Border,
    pub(super) next: N,
}

impl<N> BorderDecorator<N> {
    fn outer_radius<T>(&self, extent: Extent<T>) -> f32
    where
        T: DecoratorType + Into<f32>,
    {
        (self.border.radius as f32)
            .min(extent.width.into() / 2.0)
            .min(extent.height.into() / 2.0)
    }

    fn outline<T>(&self, offset: &Offset<T>, extent: Extent<T>, drawer: &mut Drawer)
    where
        T: DecoratorType + Into<f32>,
    {
        if self.border.size == 0 {
            return;
        }

        let border_size = self.border.size as f32;
        let outer_radius = self.outer_radius(extent);

        let mut paint = skia_safe::Paint::default();
        paint.use_color(
            &self.border.color,
            Offset::new(offset.x.into(), offset.y.into()),
            Extent::new(extent.width.into(), extent.height.into()),
        );
        paint.set_anti_alias(true);
        paint.set_style(skia_safe::PaintStyle::Stroke);
        paint.set_stroke_width(border_size);

        let half_stroke = border_size / 2.0;
        let stroke_rect = skia_safe::Rect::from_xywh(
            offset.x.into() + half_stroke,
            offset.y.into() + half_stroke,
            (extent.width.into() - border_size).max(0.0),
            (extent.height.into() - border_size).max(0.0),
        );

        let stroke_radius = (outer_radius - half_stroke).max(0.0);

        let canvas = drawer.surface.canvas();
        if stroke_radius <= 0.0 {
            canvas.draw_rect(stroke_rect, &paint);
        } else {
            let rrect = skia_safe::RRect::new_rect_xy(stroke_rect, stroke_radius, stroke_radius);
            canvas.draw_rrect(rrect, &paint);
        }
    }
}

impl<N, T> MeasureDecorator<T> for BorderDecorator<N>
where
    N: MeasureDecorator<T>,
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        self.next.intrinsic() + Spacing::all_directional(self.border.size)
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        let spacing = Spacing::all_directional(self.border.size);
        let new_constraints = constraints.shrink_to_with(&spacing);

        let used_extent = self.next.measure(new_constraints) + spacing.into();

        used_extent.clamp_with(constraints.min, constraints.max)
    }
}

impl<N, T> DrawDecorator<T> for BorderDecorator<N>
where
    N: DrawDecorator<T>,
    T: DecoratorType + Into<f32>,
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer) {
        let canvas = drawer.surface.canvas();
        canvas.save();

        let rect = skia_safe::Rect::from_xywh(
            offset.x.into(),
            offset.y.into(),
            provided_extent.width.into(),
            provided_extent.height.into(),
        );

        let outer_radius = self.outer_radius(provided_extent);
        let rrect = skia_safe::RRect::new_rect_xy(rect, outer_radius, outer_radius);

        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        let border_spacing = Spacing::all_directional(self.border.size);
        self.next.draw(
            &(*offset + border_spacing.into()),
            provided_extent - border_spacing.into(),
            drawer,
        );

        self.outline(offset, provided_extent, drawer);

        drawer.surface.canvas().restore();
    }
}

impl<N, T> EventHitTestDecorator<T> for BorderDecorator<N>
where
    N: EventHitTestDecorator<T>,
    T: DecoratorType,
{
    fn on_hit_test(
        &self,
        widget_id: WidgetId,
        local_coords: Point<T>,
        provided_extent: Extent<T>,
        router: &mut EventRouter,
    ) -> HitTestResult {
        let spacing = Spacing::all_directional(self.border.size);

        self.next.hit_test(
            widget_id,
            local_coords - spacing.into(),
            provided_extent.shrink_to_with(&spacing),
            router,
        )
    }
}
