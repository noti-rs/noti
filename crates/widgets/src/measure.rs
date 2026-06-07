use std::ops::{Add, Sub};

use num_traits::FromPrimitive;

use crate::{
    context::{
        AnimationQuery, LoadConstraints, LoadExtent, ManageConstraints, ManageDirtyFlags,
        ManageExtent, ManageIntrinsic, SaveConstraints, SaveExtent,
    },
    types::{dirty_flags::DirtyFlags, Extent, Spacing, WidgetId},
    widget::WidgetInformation,
};

pub enum SizingMode {
    Fixed,
    Dynamic,
}

impl SizingMode {
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed)
    }
}

pub(crate) trait MeasureContext<T, Id>:
    ManageIntrinsic<T, Id> + ManageExtent<T, Id> + ManageConstraints<T, Id> + AnimationQuery<Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> MeasureContext<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: ManageIntrinsic<T, Id> + ManageExtent<T, Id> + ManageConstraints<T, Id> + AnimationQuery<Id>,
{
}

pub(crate) trait MeasureVisitor<T>
where
    T: Default + Copy + PartialEq,
{
    fn visit<W: Measure<T>>(&mut self, child: &W);
}

pub(crate) trait Measure<T>: WidgetInformation
where
    T: Default + Copy + PartialEq,
{
    fn intrinsic_content<C>(&self, context: &mut C) -> Intrinsic<T>
    where
        C: ManageIntrinsic<T, WidgetId> + ManageDirtyFlags<WidgetId>;

    fn visit_children(&self, visitor: &mut impl MeasureVisitor<T>);

    fn measure_content<C>(&self, context: &mut C, constraints: Constraints<Extent<T>>) -> Extent<T>
    where
        C: MeasureContext<T, WidgetId> + ManageDirtyFlags<WidgetId>;

    fn intrinsic<C>(&self, context: &mut C) -> Intrinsic<T>
    where
        C: ManageIntrinsic<T, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let dirty_flags = context.get_dirty_flags(self.get_id());
        let cached_intrinsic = context.load(self.get_id());

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) && cached_intrinsic.is_some() {
            return cached_intrinsic.unwrap_or_default();
        }

        let intrinsic = self.intrinsic_content(context);
        context.save(self.get_id(), intrinsic);

        intrinsic
    }

    fn measure<C>(&self, context: &mut C, constraints: Constraints<Extent<T>>) -> Extent<T>
    where
        C: MeasureContext<T, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let mut dirty_flags = context.get_dirty_flags(self.get_id());
        let constraints_changed =
            Some(constraints) != <C as LoadConstraints<T, WidgetId>>::load(context, self.get_id());
        let cached_extent = <C as LoadExtent<T, WidgetId>>::load(context, self.get_id());

        if !dirty_flags.intersects(DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            return cached_extent.unwrap_or_default();
        }

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE)
            && dirty_flags.contains(DirtyFlags::CHILD_NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            struct ChildrenVisitor<'a, C> {
                context: &'a mut C,
            }

            impl<'a, T, C> MeasureVisitor<T> for ChildrenVisitor<'a, C>
            where
                T: Default + Copy + PartialEq,
                C: MeasureContext<T, WidgetId> + ManageDirtyFlags<WidgetId>,
            {
                fn visit<W: Measure<T>>(&mut self, child: &W) {
                    let child_constraints =
                        <C as LoadConstraints<T, WidgetId>>::load(self.context, child.get_id())
                            .or_else(|| {
                                <C as LoadExtent<T, WidgetId>>::load(self.context, child.get_id())
                                    .map(Constraints::new_tight)
                            })
                            .unwrap();

                    child.measure(self.context, child_constraints);
                }
            }

            let mut visitor = ChildrenVisitor { context };
            self.visit_children(&mut visitor);

            dirty_flags -= DirtyFlags::CHILD_NEEDS_MEASURE;
            context.set_dirty_flags(self.get_id(), dirty_flags);

            return cached_extent.unwrap_or_default();
        }

        let used_extent = self.measure_content(context, constraints);

        <C as SaveConstraints<T, WidgetId>>::save(context, self.get_id(), constraints);
        <C as SaveExtent<T, WidgetId>>::save(context, self.get_id(), used_extent);

        dirty_flags -= DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE;
        context.set_dirty_flags(self.get_id(), dirty_flags);

        used_extent
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Intrinsic<T>
where
    T: Default + Copy,
{
    pub min: Extent<T>,
    pub max: Extent<T>,
}

impl<T> Intrinsic<T>
where
    T: Default + Copy,
{
    pub fn new(min: Extent<T>, max: Extent<T>) -> Self {
        Self { min, max }
    }

    pub fn is_fixed(&self) -> bool
    where
        T: PartialEq,
    {
        self.min == self.max
    }
}

impl<T> Add<Spacing> for Intrinsic<T>
where
    T: Default + Copy + FromPrimitive + Add<Output = T>,
{
    type Output = Intrinsic<T>;

    fn add(mut self, rhs: Spacing) -> Self::Output {
        let horizontal_spacing = T::from_usize(rhs.horizontal()).unwrap_or_default();
        let vertical_spacing = T::from_usize(rhs.vertical()).unwrap_or_default();

        let spacing_extent = Extent::new(horizontal_spacing, vertical_spacing);

        self.min = self.min + spacing_extent;
        self.max = self.max + spacing_extent;

        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Constraints<T>
where
    T: Default + Copy,
{
    pub min: T,
    pub max: T,
}

impl<T> Constraints<T>
where
    T: Default + Copy,
{
    pub fn new_tight(val: T) -> Self {
        Self { min: val, max: val }
    }

    pub fn new_soft(val: T) -> Self {
        Self {
            min: T::default(),
            max: val,
        }
    }
}

impl<T> Constraints<Extent<T>>
where
    T: Default + Copy,
{
    pub fn shrink_to_with(&self, spacing: &Spacing) -> Self
    where
        T: PartialOrd + Sub<Output = T> + FromPrimitive,
    {
        let mut other = *self;

        other.min.shrink_by(spacing);
        other.max.shrink_by(spacing);

        other
    }
}
