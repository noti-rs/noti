use config::spacing::Spacing;
use log::warn;
use shared::{error::ConversionError, value::TryFromValue};

use crate::{
    color::{Bgra, Color},
    drawer::{Drawer, MakeRounding, SetSourceColor},
    types::{Offset, RectSize},
};

use super::{CompileState, Draw, Widget, WidgetConfiguration};

#[derive(macros::GenericBuilder, derive_builder::Builder, Clone)]
#[builder(pattern = "owned")]
#[gbuilder(name(GBuilderFlexContainer), derive(Clone))]
pub struct FlexContainer {
    #[builder(private, setter(skip))]
    #[gbuilder(hidden, default(None))]
    rect_size: Option<RectSize<usize>>,

    #[builder(private, default)]
    #[gbuilder(hidden, default(Bgra::default().into()))]
    background_color: Color,

    #[builder(private, default)]
    #[gbuilder(hidden, default(Bgra::default().into()))]
    border_color: Color,

    #[builder(default = "false")]
    #[gbuilder(default(false))]
    transparent_background: bool,

    #[builder(default = "usize::MAX")]
    #[gbuilder(default(usize::MAX))]
    max_width: usize,

    #[builder(default = "usize::MAX")]
    #[gbuilder(default(usize::MAX))]
    max_height: usize,

    #[gbuilder(default)]
    spacing: Spacing,

    #[gbuilder(default)]
    border: config::display::Border,

    direction: Direction,
    alignment: Alignment,

    children: Vec<Widget>,
}

impl FlexContainer {
    pub fn compile(
        &mut self,
        mut rect_size: RectSize<usize>,
        configuration: &WidgetConfiguration,
    ) -> CompileState {
        self.max_width = self.max_width.min(rect_size.width);
        self.max_height = self.max_height.min(rect_size.height);
        rect_size = RectSize {
            width: self.max_width,
            height: self.max_height,
        };
        self.rect_size = Some(rect_size);

        let colors = &configuration
            .theme
            .by_urgency(&configuration.notification.hints.urgency);

        self.background_color = colors.background.clone().into();
        self.border_color = colors.border.clone().into();

        rect_size.shrink_by(&(self.spacing.clone() + Spacing::all_directional(self.border.size)));
        let mut container_axes = FlexContainerPlane::new(rect_size, &self.direction);

        self.children.iter_mut().for_each(|child| {
            child.compile(container_axes.as_rect_size(), configuration);

            container_axes.main_len = container_axes
                .main_len
                .saturating_sub(child.len_by_direction(&self.direction));
        });
        self.children.retain(|child| !child.is_unknown());

        if self.children.is_empty() {
            warn!(
                "The flex container is empty! Did you add the widgets? \
                Or check them, maybe they doesn't fit available space."
            );
        }

        CompileState::Success
    }

    pub(super) fn max_width(&self) -> usize {
        self.max_width
    }

    pub(super) fn max_height(&self) -> usize {
        self.max_height
    }

    pub fn width(&self) -> usize {
        let widths = self.children.iter().map(|child| child.width());

        match self.direction {
            Direction::Horizontal => widths.sum(),
            Direction::Vertical => widths.max().unwrap_or_default(),
        }
    }

    pub fn height(&self) -> usize {
        let heights = self.children.iter().map(|child| child.height());

        match self.direction {
            Direction::Horizontal => heights.max().unwrap_or_default(),
            Direction::Vertical => heights.sum(),
        }
    }

    #[allow(unused)]
    fn max_main_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.max_width(),
            Direction::Vertical => self.max_height(),
        }
    }

    #[allow(unused)]
    fn max_auxiliary_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.max_height(),
            Direction::Vertical => self.max_width(),
        }
    }

    fn main_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.width(),
            Direction::Vertical => self.height(),
        }
    }

    #[allow(unused)]
    fn auxiliary_len(&self) -> usize {
        match &self.direction {
            Direction::Horizontal => self.height(),
            Direction::Vertical => self.width(),
        }
    }

    fn main_axis_alignment(&self) -> &Position {
        match &self.direction {
            Direction::Horizontal => &self.alignment.horizontal,
            Direction::Vertical => &self.alignment.vertical,
        }
    }

    fn auxiliary_axis_alignment(&self) -> &Position {
        match &self.direction {
            Direction::Horizontal => &self.alignment.vertical,
            Direction::Vertical => &self.alignment.horizontal,
        }
    }

    fn rounded_fill(&self, offset: Offset<f64>, rect_size: RectSize<f64>, drawer: &mut Drawer) {
        let radius = self.border.radius as f64 - self.border.size as f64;
        drawer.context.new_sub_path();
        drawer.make_rounding(offset, rect_size, self.border.radius as f64, radius);
        drawer.context.close_path();

        drawer.set_source_color(&self.background_color, rect_size);
        drawer.context.fill().unwrap();
    }

    fn outline_border(&self, offset: Offset<f64>, rect_size: RectSize<f64>, drawer: &mut Drawer) {
        // INFO: if we use half of border size, a visible area between fill and border will be
        // wisible, so need avoid it by making radius slightly nearer (divide by 1.9, not 2.0)
        let radius = self.border.radius as f64 - (self.border.size as f64 / 1.9);

        drawer.context.new_sub_path();
        drawer.make_rounding(offset, rect_size, self.border.radius as f64, radius);
        drawer.context.close_path();

        drawer.set_source_color(&self.border_color, rect_size);
        drawer.context.set_line_width(self.border.size as f64);
        drawer.context.stroke().unwrap();
    }
}

impl Draw for FlexContainer {
    fn draw_with_offset(&mut self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let Some(mut rect_size) = self.rect_size.as_ref().cloned() else {
            panic!(
                "The rectangle size must be computed by `compile()` method of parent container!"
            );
        };
        let original_rect_size = rect_size;

        // NOTE: if the background color is transparent or forces to be transparent, no need to use
        // another layer as new Drawer instance. Instead of this use the current Drawer instance.
        // It will avoid to use costly methods `draw_area` and `draw_with_offset`.
        let transparent_bg = self.transparent_background || self.background_color.is_transparent();

        if !transparent_bg {
            self.rounded_fill((*offset).into(), rect_size.into(), drawer);
        }

        rect_size.shrink_by(&(self.spacing.clone() + Spacing::all_directional(self.border.size)));
        let mut plane = FlexContainerPlane::new(rect_size, &self.direction);

        let initial_offset = Offset::new(
            self.spacing.left() as usize + self.border.size as usize,
            self.spacing.top() as usize + self.border.size as usize,
        );
        let initial_plane = FlexContainerPlane::new_only_offset(initial_offset, &self.direction);
        plane.relocate(&initial_plane.as_offset());

        plane.main_axis_offset += self
            .main_axis_alignment()
            .compute_initial_pos(plane.main_len, self.main_len());

        let incrementor = match self.main_axis_alignment() {
            Position::Start | Position::Center | Position::End => 0,
            Position::SpaceBetween => {
                if self.children.len() <= 1 {
                    0
                } else {
                    (plane.main_len - self.main_len()) / self.children.len().saturating_sub(1)
                }
            }
        };

        let auxiliary_axis_alignment = self.auxiliary_axis_alignment().clone();
        self.children.iter_mut().for_each(|child| {
            plane.auxiliary_axis_offset = initial_plane.auxiliary_axis_offset
                + auxiliary_axis_alignment.compute_initial_pos(
                    plane.auxiliary_len,
                    child.len_by_direction(&self.direction.orthogonalize()),
                );

            child.draw_with_offset(&(plane.as_offset() + *offset), drawer);

            plane.main_axis_offset += child.len_by_direction(&self.direction) + incrementor;
        });

        self.outline_border((*offset).into(), original_rect_size.into(), drawer);
    }
}

#[derive(macros::GenericBuilder, Debug, Default, Clone)]
#[gbuilder(name(GBuilderAlignment), derive(Clone), constructor)]
pub struct Alignment {
    #[gbuilder(aliases(diagonal))]
    pub horizontal: Position,

    #[gbuilder(aliases(diagonal))]
    pub vertical: Position,
}

impl Alignment {
    pub fn new(horizontal: Position, vertical: Position) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

impl TryFromValue for Alignment {}

#[derive(Debug, Default, Clone)]
pub enum Position {
    Start,
    #[default]
    Center,
    End,
    SpaceBetween,
}

impl TryFromValue for Position {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "start" => Position::Start,
            "center" => Position::Center,
            "end" => Position::End,
            "space-between" | "space_between" => Position::SpaceBetween,
            _ => Err(ConversionError::InvalidValue {
                expected: "start, center, end, space-between or space_between",
                actual: value,
            })?,
        })
    }
}

impl Position {
    pub fn compute_initial_pos(&self, width: usize, element_width: usize) -> usize {
        match self {
            Position::Start | Position::SpaceBetween => 0,
            Position::Center => width / 2 - element_width / 2,
            Position::End => width - element_width,
        }
    }
}

#[derive(Clone)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    fn orthogonalize(&self) -> Direction {
        match self {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        }
    }
}

impl TryFromValue for Direction {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "horizontal" => Direction::Horizontal,
            "vertical" => Direction::Vertical,
            _ => Err(ConversionError::InvalidValue {
                expected: "horizontal or vertical",
                actual: value,
            })?,
        })
    }
}

struct FlexContainerPlane<'a> {
    main_len: usize,
    auxiliary_len: usize,

    main_axis_offset: usize,
    auxiliary_axis_offset: usize,

    direction: &'a Direction,
}

impl<'a> FlexContainerPlane<'a> {
    fn new(
        RectSize {
            mut width,
            mut height,
        }: RectSize<usize>,
        direction: &'a Direction,
    ) -> Self {
        if let Direction::Vertical = direction {
            (width, height) = (height, width);
        }

        Self {
            main_len: width,
            auxiliary_len: height,
            main_axis_offset: 0,
            auxiliary_axis_offset: 0,
            direction,
        }
    }

    fn new_only_offset(Offset { mut x, mut y }: Offset<usize>, direction: &'a Direction) -> Self {
        if let Direction::Vertical = direction {
            (x, y) = (y, x);
        }

        Self {
            main_len: 0,
            auxiliary_len: 0,
            main_axis_offset: x,
            auxiliary_axis_offset: y,
            direction,
        }
    }

    fn relocate(&mut self, Offset { mut x, mut y }: &Offset<usize>) {
        if let Direction::Vertical = self.direction {
            (x, y) = (y, x);
        }

        self.main_axis_offset = x;
        self.auxiliary_axis_offset = y;
    }

    fn as_rect_size(&self) -> RectSize<usize> {
        let (mut width, mut height) = (self.main_len, self.auxiliary_len);

        if let Direction::Vertical = self.direction {
            (width, height) = (height, width);
        }

        RectSize::new(width, height)
    }

    fn as_offset(&self) -> Offset<usize> {
        let (mut x, mut y) = (self.main_axis_offset, self.auxiliary_axis_offset);

        if let Direction::Vertical = self.direction {
            (x, y) = (y, x);
        }

        Offset::new(x, y)
    }
}
