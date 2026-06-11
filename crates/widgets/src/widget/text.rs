use std::cell::RefCell;

use log::warn;
use macros::widget;
use shared::{
    text::{self, Entity, EntityKind},
    value::TryFromValue,
};

use crate::{
    context::{ManageDirtyFlags, ManageIntrinsic, StateSubscription},
    decorator::{content::Content, DecoratorExt, DrawDecorator, MeasureDecorator},
    events::{EventContext, EventHandling, EventHitTest, EventRouter, HitTestResult, PendingEvent},
    stage::{
        draw::{draw_debug_bounds, Drawer, UseColor},
        invalidate::{InvalidateVisitor, RebuildStatus},
        layout::{Layout, LayoutContext},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
    },
    state::State,
    types::{
        extent::Extent,
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        offset::Offset,
        spacing::Spacing,
        style::{Configure, WidgetStyle},
        Color, Point,
    },
    widget::{
        Draw, DrawContext, Init, InitContext, Invalidate, InvalidateContext, WidgetGetType,
        WidgetSizingMode,
    },
};

/// A text widget that manages layout, styling, and rendering of text
/// content within the UI.
///
/// `WText` aims to be simple but flexible, providing a consistent API
/// for compilation and querying its dimensions after layout.
#[widget]
#[make_widget_style(TextStyle, derive(bon::Builder, Debug, Clone))]
#[derive(bon::Builder, Default)]
pub struct Text {
    /// The specific typeface and sizing rules used to render this text.
    ///
    /// This field points to a [`Font`] struct that contains the font family
    /// name, the size (in px), and the style (like Bold or Italic). It ensures
    /// that the text is drawn with the correct visual weight and proportions.
    #[style]
    font: Font,

    /// Determines if text should break into multiple lines when it hits the
    /// edge of its container.
    ///
    /// When enabled, long sentences will automatically "wrap" to a new line
    /// to stay within the available width. When disabled, the text remains
    /// on a single line, potentially overflowing the container if it
    /// is too long.
    #[style]
    wrap: bool,

    /// Controls the horizontal distribution of text within its boundary.
    ///
    /// This tells the engine where to anchor the lines of text—whether they
    /// should hug the left side, sit in the center, or be pushed to the
    /// right. It can also spread the words out (space-between) to fill
    /// the entire width of the line.
    #[style]
    alignment: TextAlignment,

    /// The extra vertical space added between lines, measured in the same units as font size.
    ///
    /// This value is used to calculate a height multiplier for the text layout.
    /// For example, if the font size is 16 and line_spacing is 4, the total
    /// line height becomes 20 (a 1.25x multiplier).
    ///
    /// Note: Internally, this is converted to a ratio to satisfy the rendering
    /// engine's requirement for a line-height multiplier.
    #[style]
    line_spacing: usize,

    /// The foreground color applied to the text characters.
    ///
    /// This determines the color of the rendered glyphs (the letters and
    /// symbols) themselves. Unlike a background, this does not fill the
    /// bounding box; it only colors the "ink" used to draw the text.
    /// Use this to ensure your text has enough contrast against the
    /// background it is sitting on.
    #[style]
    color: Color,

    /// The actual text data that this widget is responsible for rendering.
    ///
    /// This field holds the characters and formatting instructions that
    /// make up your message. Whether it is a simple label or a complex
    /// paragraph with mixed styles, this is the primary source of
    /// information the widget uses to draw glyphs on the screen.
    #[builder(default)]
    value: text::Text,

    #[builder(into)]
    state: Option<State<text::Text>>,

    /// A shared reference to the system's global font registry.
    ///
    /// Instead of each widget maintaining its own expensive font data, this
    /// field holds a reference-counted handle to a central collection. This
    /// allows the widget to efficiently resolve font styles and re-render
    /// text—such as when changing colors—without the memory overhead of
    /// duplicating font resources.
    #[builder(skip)]
    font_collection: Option<skia_safe::textlayout::FontCollection>,

    /// The compiled layout of the text, used for rendering and hit-testing.
    ///
    /// This field acts as the "brain" of the text widget once it has been
    /// processed. If the text has been compiled, this struct provides
    /// precise data about the line breaks, glyph positions, and the total
    /// area the text occupies. It is essential for interactive tasks,
    /// such as determining if a user is hovering over a specific link or
    /// character.
    #[builder(skip)]
    paragraph: Option<RefCell<skia_safe::textlayout::Paragraph>>,

    /// A cached count of the lines required to display the current text.
    ///
    /// This field stores the calculated line count to avoid redundant
    /// computations during the drawing stage. It is particularly useful
    /// when the text exceeds its available space, allowing the widget
    /// to quickly rebuild its layout or apply wrapping rules without
    /// starting from scratch.
    #[builder(skip)]
    total_lines: Option<usize>,
}

/// A collection of typographic settings that define the "look" and scale of a typeface.
///
/// Instead of managing font names and sizes separately, this struct groups
/// everything needed to tell the rendering engine exactly how to draw
/// each character. It covers the font family, the physical scale in points,
/// and the specific weight or slant of the text.
#[derive(Debug, Clone)]
pub struct Font {
    /// The name of the font family (e.g., "Inter", "JetBrains Mono").
    ///
    /// Default: "Noto Sans"
    pub name: String,

    /// The size of the text measured in pixels (px).
    ///
    /// Using points ensures that the text maintains a consistent physical
    /// scale regardless of the screen's pixel density.
    ///
    /// Default: 14
    pub size: usize,

    /// The specific variant of the font, such as Bold or Italic.
    ///
    /// Default: FontStyle::Regular
    pub style: FontStyle,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            name: "Noto Sans".to_string(),
            size: 14,
            style: FontStyle::default(),
        }
    }
}

/// The base typographic weight and slant applied to a text widget.
///
/// Think of this as the "starting point" for all characters in the text.
/// It acts as a foundation that can be combined with specific text
/// entities (like bold or italic spans) to create a final look.
///
/// **The Overlay Rule:**
/// If the base style is `Italic` and a specific part of the text is
/// marked as `Bold` by an entity, the two will mix together to
/// render as `BoldItalic`. This allows you to set a global tone for
/// the text without manually wrapping every single word in an entity.
#[derive(Debug, Default, Clone)]
pub enum FontStyle {
    /// Standard text with no extra weight or slant.
    #[default]
    Regular,
    /// Thickens the character strokes for emphasis.
    Bold,
    /// Slants the characters to the right.
    Italic,
    /// Applies both thickness and slant simultaneously.
    BoldItalic,
}

impl TryFromValue for FontStyle {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "regular" => FontStyle::Regular,
            "bold" => FontStyle::Bold,
            "italic" => FontStyle::Italic,
            "bold-italic" | "bold_italic" => FontStyle::BoldItalic,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "regular, bold, italic, bold-italic or bold_italic",
                actual: value,
            })?,
        })
    }
}

/// Rules for how text lines are positioned horizontally within their
/// available width.
///
/// This determines where the "anchor" of each line sits. It is particularly
/// useful for making sure text looks natural depending on its context
/// (like a centered title or a left-aligned paragraph).
#[derive(Default, Debug, Clone)]
pub enum TextAlignment {
    /// Lines are centered, leaving equal space on both sides.
    Center,
    /// Lines are anchored to the left edge (Standard for most languages).
    #[default]
    Left,
    /// Lines are anchored to the right edge.
    Right,
    /// Words are spread out so that every line has the exact same
    /// width, touching both the left and right edges.
    Justify,
}

impl TryFromValue for TextAlignment {
    fn try_from_string(value: String) -> Result<Self, shared::error::ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "center" => TextAlignment::Center,
            "left" => TextAlignment::Left,
            "right" => TextAlignment::Right,
            "justify" => TextAlignment::Justify,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "center, left or right",
                actual: value,
            })?,
        })
    }
}

impl Text {
    // INFO: better to pass it instead of `usize::MAX` because skia's textlayout module
    // with ellipsis will make the layout in strange way — just truncate the first line
    // with available space.
    const MAX_LINES: usize = 100_000;

    pub fn new() -> Self {
        Self {
            value: text::Text::new_empty(),
            paragraph: None,
            ..Default::default()
        }
    }
}

impl WidgetGetType for Text {
    fn get_type(&self) -> &'static str {
        "text"
    }
}

impl WidgetSizingMode for Text {
    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Dynamic
    }
}

impl<C> Init<C> for Text
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        if let Some(state) = self.state {
            <C as StateSubscription<WidgetId>>::subscribe(context, self.id, state);

            if let Some(text) = context.get(state) {
                self.value = text.clone();
            }
        }

        if let Some(WidgetStyle::Text(text_style)) = context.get_style(&self.class) {
            self.configure(text_style.clone());
        }

        let font = self.font.clone().unwrap_or_default();
        self.font_collection = context.get_font().into();

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_font_families(&[&font.name]);
        text_style.set_color(skia_safe::Color::BLACK);
        text_style.set_font_style(match font.style {
            FontStyle::Regular => skia_safe::FontStyle::normal(),
            FontStyle::Bold => skia_safe::FontStyle::bold(),
            FontStyle::Italic => skia_safe::FontStyle::italic(),
            FontStyle::BoldItalic => skia_safe::FontStyle::bold_italic(),
        });

        let mut paragraph = self.build_paragraph(&text_style, Self::MAX_LINES);
        paragraph.layout(paragraph.max_intrinsic_width());

        self.paragraph = Some(RefCell::new(paragraph));
    }
}

impl<C> Invalidate<C> for Text
where
    C: InvalidateContext,
{
    fn on_style_update(&mut self, _context: &mut C, style: WidgetStyle) {
        if let WidgetStyle::Text(text_style) = style {
            self.configure(text_style);
        }
    }

    fn on_rebuild(&mut self, context: &mut C) -> RebuildStatus {
        if let Some(text) = self
            .state
            .and_then(|state| context.get(state))
            .take_if(|text| **text != self.value)
        {
            self.value = text.clone();

            RebuildStatus::NeedsMeasure
        } else {
            RebuildStatus::NothingChanged
        }
    }

    fn invalidate_children(&mut self, _visitor: &mut impl InvalidateVisitor<C>) {}
}

impl Measure<f32> for Text {
    fn intrinsic_content<C>(&self, _context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        Content::intrinsic_fn(|| {
            let Some(mut paragraph) = self.paragraph.as_ref().map(RefCell::borrow_mut) else {
                return measure::Intrinsic::default();
            };

            /// The Paragraph from skia has some rounding error and because of this text wraps in not
            /// desired manner. We should explicitly round and make the max intrinsic larger to avoid
            /// this case.
            const EPSILON: f32 = 1.0;
            let min_width = paragraph.min_intrinsic_width();
            let max_width = paragraph.max_intrinsic_width().ceil() + EPSILON;

            let min_height = {
                paragraph.layout(max_width);
                paragraph.height()
            };
            let max_height = {
                paragraph.layout(min_width);
                paragraph.height()
            };

            paragraph.layout(max_width);

            measure::Intrinsic::new(
                Extent::new(min_width, min_height),
                Extent::new(max_width, max_height),
            )
        })
        .spacing(self.margin.unwrap_or_default())
        .intrinsic()
    }

    fn measure_children(&self, _visitor: &mut impl measure::MeasureVisitor<f32>) {}

    fn measure_content<C>(
        &self,
        _context: &mut C,
        constraints: Constraints<Extent<f32>>,
    ) -> Extent<f32>
    where
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        Content::measure_fn(|child_constraints| {
            let Some(mut paragraph) = self.paragraph.as_ref().map(RefCell::borrow_mut) else {
                return Extent::default();
            };

            let width = child_constraints.max.width;
            paragraph.layout(width);
            let height = paragraph.height();

            let max_intrinsic_width = paragraph.max_intrinsic_width();
            paragraph.layout(max_intrinsic_width);

            Extent::new(width, height)
        })
        .spacing(self.margin.unwrap_or_default())
        .measure(constraints)
    }
}

impl<C> Layout<C, f32> for Text
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &C) {
        let Some(extent) = context.load(self.id) else {
            warn!(
                "Text widget with id {} isn't measured! The widget may be incorrectly drawn.",
                *self.id
            );
            return;
        };

        let inner_spacing = self.margin.unwrap_or_default();
        let inner_extent = Extent::new(
            extent.width - inner_spacing.horizontal() as f32,
            extent.height - inner_spacing.vertical() as f32,
        );

        let text_style = self.base_text_style();

        if let Some(mut paragraph) = self.paragraph.as_ref().map(|para| para.borrow_mut()) {
            paragraph.layout(inner_extent.width);

            if paragraph.height() > inner_extent.height {
                match self.try_fit_paragraph(&paragraph, inner_extent, &text_style) {
                    Some((new_paragraph, total_lines)) => {
                        *paragraph = new_paragraph;
                        self.total_lines = Some(total_lines);
                    }
                    None => {
                        warn!(
                            "Text widght with id {} didn't fit in provided inner_extent.",
                            *self.id
                        );
                    }
                }
            }
        }
    }
}

impl Text {
    fn base_text_style(&self) -> skia_safe::textlayout::TextStyle {
        let font = self.font.clone().unwrap_or_default();

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_font_families(&[&font.name]);
        text_style.set_color(skia_safe::Color::BLACK);
        text_style.set_font_style(match font.style {
            FontStyle::Regular => skia_safe::FontStyle::normal(),
            FontStyle::Bold => skia_safe::FontStyle::bold(),
            FontStyle::Italic => skia_safe::FontStyle::italic(),
            FontStyle::BoldItalic => skia_safe::FontStyle::bold_italic(),
        });

        text_style
    }

    /// Attempts to make the current paragraph layout fit within the available
    /// vertical space by iteratively removing the last lines until it fits.
    ///
    /// Returns `None` if fitting is impossible without removing all text.
    ///
    /// # Assumptions
    /// * The paragraph is already known to fit horizontally within the
    ///   available width.
    /// * Called after paragraph construction and layout calculation.
    fn try_fit_paragraph(
        &self,
        paragraph: &skia_safe::textlayout::Paragraph,
        available_space: Extent<f32>,
        base_text_style: &skia_safe::textlayout::TextStyle,
    ) -> Option<(skia_safe::textlayout::Paragraph, usize)> {
        let mut height = paragraph.height();
        let line_metrics = paragraph.get_line_metrics();
        let mut total_lines = line_metrics.len();

        for last_line in line_metrics.into_iter().rev() {
            height -= last_line.height as f32;
            total_lines -= 1;

            if height <= available_space.height {
                break;
            }
        }

        if total_lines == 0 {
            None
        } else {
            let mut fitted_paragraph = self.build_paragraph(base_text_style, total_lines);
            fitted_paragraph.layout(available_space.width);
            Some((fitted_paragraph, total_lines))
        }
    }

    /// Builds a Skia `Paragraph` from the notification's text and entities.
    ///
    /// This method assumes that all text entities are valid and non-overlapping.
    /// If entities overlap or are malformed, the resulting paragraph may have
    /// incorrect or conflicting styles.
    ///
    /// Intended for internal use during `compile`.
    fn build_paragraph(
        &self,
        base_text_style: &skia_safe::textlayout::TextStyle,
        max_lines: usize,
    ) -> skia_safe::textlayout::Paragraph {
        assert!(
            self.font_collection.is_some(),
            "FontCollection must be already set before building paragraph"
        );

        let paragraph_style = self.make_paragraph_style(max_lines);

        let mut paragraph_builder = skia_safe::textlayout::ParagraphBuilder::new(
            &paragraph_style,
            self.font_collection.as_ref().unwrap(),
        );
        paragraph_builder.push_style(base_text_style);

        let text = &self.value.body;

        let mut cursor = 0;
        let mut end_stack = vec![text.len()];
        let mut current_entity_index = 0;

        while cursor < text.len() {
            let nearest_end = unsafe { *end_stack.last().unwrap_unchecked() };
            let entity = match self.value.entities.get(current_entity_index) {
                Some(entity) => entity,
                None => {
                    paragraph_builder.add_text(&text[cursor..nearest_end]);
                    paragraph_builder.pop();
                    cursor = nearest_end;
                    end_stack.pop();
                    continue;
                }
            };

            if entity.offset_in_byte > nearest_end {
                paragraph_builder.add_text(&text[cursor..nearest_end]);
                paragraph_builder.pop();
                cursor = nearest_end;
                end_stack.pop();
            } else {
                paragraph_builder.add_text(&text[cursor..entity.offset_in_byte]);
                cursor = entity.offset_in_byte;
                end_stack.push(entity.offset_in_byte + entity.length_in_byte);
                current_entity_index += 1;

                let text_style = paragraph_builder.overlay_style(entity);
                paragraph_builder.push_style(&text_style);
            }
        }

        paragraph_builder.build()
    }

    /// Creates a [`skia_safe::textlayout::ParagraphStyle`] based on the current
    /// widget's text properties and user configuration.
    ///
    /// This method is responsible for setting up alignment, line spacing,
    /// and other high-level paragraph attributes before building the text
    /// content itself.
    fn make_paragraph_style(&self, max_lines: usize) -> skia_safe::textlayout::ParagraphStyle {
        let mut paragraph_style = skia_safe::textlayout::ParagraphStyle::new();

        let max_lines = if self.wrap.unwrap_or(true) {
            max_lines
        } else {
            1
        };
        paragraph_style.set_max_lines(max_lines);
        paragraph_style.set_ellipsis("…");

        let text_align = match self.alignment.clone().unwrap_or_default() {
            TextAlignment::Justify => skia_safe::textlayout::TextAlign::Justify,
            TextAlignment::Center => skia_safe::textlayout::TextAlign::Center,
            TextAlignment::Left => skia_safe::textlayout::TextAlign::Left,
            TextAlignment::Right => skia_safe::textlayout::TextAlign::Right,
        };
        paragraph_style.set_text_align(text_align);

        {
            let mut strut_style = skia_safe::textlayout::StrutStyle::new();
            strut_style.set_strut_enabled(true);

            let font = self.font.clone().unwrap_or_default();
            strut_style.set_font_size(font.size as f32);

            let line_spacing = self.line_spacing.unwrap_or_default();
            strut_style.set_height(line_spacing as f32 / font.size as f32 + 1.0);
            strut_style.set_height_override(true);

            paragraph_style.set_strut_style(strut_style);
        }

        paragraph_style
    }
}

impl<C> Draw<C, f32> for Text
where
    C: DrawContext<f32>,
{
    fn draw_content(
        &self,
        context: &C,
        offset: &Offset<f32>,
        provided_extent: Extent<f32>,
        drawer: &mut Drawer,
    ) {
        Content::draw_fn(
            |offset: &Offset<f32>, provided_extent: Extent<f32>, drawer: &mut Drawer| {
                // INFO: as you see, there's re-building paragraph. Since I want to use `Paint` type for
                // coloring the text, it requires the exact position on surface, and because of this I
                // cannot determine once the position. Especially when a banner moves from one place to
                // another.

                let font = self.font.clone().unwrap_or_default();
                let mut base_text_style = skia_safe::textlayout::TextStyle::new();
                base_text_style.set_font_families(&[&font.name]);
                base_text_style.set_font_style(match font.style {
                    FontStyle::Regular => skia_safe::FontStyle::normal(),
                    FontStyle::Bold => skia_safe::FontStyle::bold(),
                    FontStyle::Italic => skia_safe::FontStyle::italic(),
                    FontStyle::BoldItalic => skia_safe::FontStyle::bold_italic(),
                });

                let mut paint = skia_safe::Paint::default();
                paint.use_color(
                    &self.color.clone().unwrap_or_default(),
                    *offset,
                    provided_extent,
                );

                base_text_style.set_foreground_paint(&paint);

                let mut paragraph = if let Some(total_lines) = self.total_lines {
                    self.build_paragraph(&base_text_style, total_lines)
                } else {
                    self.build_paragraph(&base_text_style, Self::MAX_LINES)
                };
                paragraph.layout(provided_extent.width);

                let canvas = drawer.surface.canvas();
                paragraph.paint(canvas, (offset.x, offset.y));

                if context.get_debug_options().show_layout_bounds {
                    draw_debug_bounds(canvas, *offset, provided_extent);
                }
            },
        )
        .spacing(self.margin.unwrap_or_default())
        .draw(offset, provided_extent, drawer);
    }
}

impl<C> EventHitTest<f32, C> for Text
where
    C: EventContext<f32>,
{
    fn on_hit_test(
        &self,
        _context: &C,
        _local_coords: Point<f32>,
        _provided_extent: Extent<f32>,
        _router: &mut EventRouter,
    ) -> HitTestResult {
        HitTestResult::Missed
    }
}

impl<C> EventHandling<f32, C> for Text
where
    C: EventContext<f32>,
{
    fn handle_events(
        &mut self,
        _context: &mut C,
        _pending_events: Vec<PendingEvent>,
        _next_child: usize,
        _router: &EventRouter,
    ) {
        // TODO: implement link click
    }
}

/// A helper trait that allows stacking and combining multiple text styles
/// to mimic HTML-like nested styling.
///
/// By default, Skia's [`skia_safe::textlayout::TextStyle`] does not overlay
/// previous styles — each style fully replaces the last one. This trait
/// provides a way to build a combined [`skia_safe::textlayout::TextStyle`]
/// from an [`Entity`], making it possible to support constructs like
/// `<b><i>bold italic text</i></b>` where multiple styles apply simultaneously.
trait OverlayStyle {
    fn overlay_style(&mut self, entity: &Entity) -> skia_safe::textlayout::TextStyle;
}

impl OverlayStyle for skia_safe::textlayout::ParagraphBuilder {
    fn overlay_style(&mut self, entity: &Entity) -> skia_safe::textlayout::TextStyle {
        let last_text_style = self.peek_style();

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_color(last_text_style.color());
        text_style.set_foreground_paint(&last_text_style.foreground());
        text_style.set_font_families(&last_text_style.font_families().iter().collect::<Vec<_>>());

        let last_font_style = last_text_style.font_style();
        match &entity.kind {
            EntityKind::Bold => {
                if last_font_style == skia_safe::FontStyle::italic()
                    || last_font_style == skia_safe::FontStyle::bold_italic()
                {
                    text_style.set_font_style(skia_safe::FontStyle::bold_italic());
                } else {
                    text_style.set_font_style(skia_safe::FontStyle::bold());
                }
            }
            EntityKind::Italic => {
                let last_style = self.peek_style().font_style();
                if last_style == skia_safe::FontStyle::bold()
                    || last_style == skia_safe::FontStyle::bold_italic()
                {
                    text_style.set_font_style(skia_safe::FontStyle::bold_italic());
                } else {
                    text_style.set_font_style(skia_safe::FontStyle::italic());
                }
            }
            EntityKind::Underline => {
                text_style.set_font_style(last_font_style);
                text_style.set_decoration_type(skia_safe::textlayout::TextDecoration::UNDERLINE);
            }
            EntityKind::Link { .. } | EntityKind::Image { .. } => (),
        }

        text_style
    }
}
