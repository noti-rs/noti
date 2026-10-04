use std::ops::{Add, Sub};

use num_traits::FromPrimitive;

use crate::{
    context::{
        AnimationQuery, LoadConstraints, LoadExtent, ManageConstraints, ManageDirtyFlags,
        ManageExtent, ManageIntrinsic, SaveConstraints, SaveExtent,
    },
    types::{dirty_flags::DirtyFlags, Extent, Spacing, WidgetId},
    widget::{WidgetInformation, WidgetInformationContext},
};

pub enum SizingMode {
    /// Widget size does not depend on content, parent constraints, or children sizes.
    Fixed,

    /// Widget size may depend on content, parent constraints, or children sizes.
    Dynamic,
}

impl SizingMode {
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed)
    }
}

pub trait MeasureContext<T>: WidgetInformationContext + ManageDirtyFlags<WidgetId>
where
    T: Default + Copy,
{
    fn widget_intrinsic(&mut self, widget_id: &WidgetId) -> Option<Intrinsic<T>>;
    fn measure_widget(
        &mut self,
        widget_id: &WidgetId,
        constraints: Constraints<Extent<T>>,
    ) -> Option<Extent<f32>>;
}

pub trait ManageMeasures<T, Id>:
    ManageIntrinsic<T, Id> + ManageExtent<T, Id> + ManageConstraints<T, Id> + AnimationQuery<Id>
where
    T: Default + Copy,
    Id: Into<WidgetId>,
{
}

impl<C, T, Id> ManageMeasures<T, Id> for C
where
    T: Default + Copy,
    Id: Into<WidgetId>,
    C: ManageIntrinsic<T, Id> + ManageExtent<T, Id> + ManageConstraints<T, Id> + AnimationQuery<Id>,
{
}

pub trait Measure<C, T>: WidgetInformation
where
    T: Default + Copy + PartialEq,
    C: MeasureContext<T>,
{
    fn intrinsic_content(&self, context: &mut C) -> Intrinsic<T>
    where
        C: ManageIntrinsic<T, WidgetId>;

    fn measure_content(&self, context: &mut C, constraints: Constraints<Extent<T>>) -> Extent<T>
    where
        C: ManageMeasures<T, WidgetId>;

    fn intrinsic(&self, context: &mut C) -> Intrinsic<T>
    where
        C: ManageIntrinsic<T, WidgetId>,
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

    fn measure(&self, context: &mut C, constraints: Constraints<Extent<T>>) -> Extent<T>
    where
        C: ManageMeasures<T, WidgetId>,
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
            for widget_id in context.childrens_identifiers_of(self.get_id()) {
                let child_constraints =
                    <C as LoadConstraints<T, WidgetId>>::load(context, widget_id)
                        .or_else(|| {
                            <C as LoadExtent<T, WidgetId>>::load(context, widget_id)
                                .map(Constraints::new_tight)
                        })
                        .unwrap();

                context.measure_widget(&widget_id, child_constraints);
            }

            dirty_flags -= DirtyFlags::CHILD_NEEDS_MEASURE;
            context.set_dirty_flags(self.get_id(), dirty_flags);

            return cached_extent.unwrap_or_default();
        }

        let used_extent = self.measure_content(context, constraints);

        <C as SaveConstraints<T, WidgetId>>::save(context, self.get_id(), constraints);
        <C as SaveExtent<T, WidgetId>>::save(context, self.get_id(), used_extent);

        dirty_flags -= DirtyFlags::NEEDS_MEASURE | DirtyFlags::CHILD_NEEDS_MEASURE;
        context.set_dirty_flags(self.get_id(), dirty_flags | DirtyFlags::NEEDS_LAYOUT);

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
