use crate::{
    decorator::{DecoratorType, DrawDecorator, MeasureDecorator},
    draw::{Drawer, UseColor},
    measure::{Constraints, Intrinsic},
    types::{Color, Extent, Offset},
};

pub(crate) struct BackgroundDecorator<N> {
    pub(super) background_color: Color,
    pub(super) next: N,
}

impl<N, T> MeasureDecorator<T> for BackgroundDecorator<N>
where
    N: MeasureDecorator<T>,
    T: DecoratorType,
{
    fn intrinsic(&mut self) -> Intrinsic<T> {
        self.next.intrinsic()
    }

    fn measure(&mut self, constraints: Constraints<Extent<T>>) -> Extent<T> {
        self.next.measure(constraints)
    }
}

impl<N, T> DrawDecorator<T> for BackgroundDecorator<N>
where
    N: DrawDecorator<T>,
    T: DecoratorType + Into<f32>,
{
    fn draw(&self, offset: &Offset<T>, provided_extent: Extent<T>, drawer: &mut Drawer) {
        let canvas = drawer.surface.canvas();

        let mut paint = skia_safe::Paint::default();

        paint.use_color(
            &self.background_color,
            Offset::new(offset.x.into(), offset.y.into()),
            Extent::new(provided_extent.width.into(), provided_extent.height.into()),
        );
        paint.set_anti_alias(true);

        canvas.draw_rect(
            skia_safe::Rect::from_xywh(
                offset.x.into(),
                offset.y.into(),
                provided_extent.width.into(),
                provided_extent.height.into(),
            ),
            &paint,
        );

        self.next.draw(offset, provided_extent, drawer);
    }
}
