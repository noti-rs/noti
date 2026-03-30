use log::warn;
use shared::{
    text::{self, Entity, EntityKind},
    value::TryFromValue,
};

use crate::{
    drawer::{Drawer, UseColor},
    events::{Action, DispatchEvent, Event},
    make_configuration,
    types::{
        data::{Configure, ToConfig, WidgetConfig, WidgetData},
        extent::Extent2D,
        offset::Offset,
        spacing::Spacing,
        widget_id::WidgetId,
        Color,
    },
    Compile, CompileCtx, CompileState, Draw, WidgetInfo,
};

/// A text widget that manages layout, styling, and rendering of text
/// content within the UI.
///
/// `WText` aims to be simple but flexible, providing a consistent API
/// for compilation and querying its dimensions after layout.
#[derive(macros::GenericBuilder, derive_builder::Builder, Default)]
#[builder(pattern = "owned")]
#[gbuilder(name(TextGBuilder))]
pub struct Text {
    /// An optional identifier for this widget.
    ///
    /// If left empty, an ID will be automatically generated during
    /// compilation. Setting this manually allows the widget to be
    /// targeted by external configurations and makes the widget tree
    /// significantly easier to navigate during debugging.
    #[builder(default, setter(into))]
    #[gbuilder(default)]
    id: WidgetId,

    /// The specific typeface and sizing rules used to render this text.
    ///
    /// This field points to a [`Font`] struct that contains the font family
    /// name, the size (in px), and the style (like Bold or Italic). It ensures
    /// that the text is drawn with the correct visual weight and proportions.
    #[builder(setter(strip_option), default)]
    #[gbuilder(use_gbuilder(FontGBuilder))]
    font: Option<Font>,

    /// Determines if text should break into multiple lines when it hits the
    /// edge of its container.
    ///
    /// When enabled, long sentences will automatically "wrap" to a new line
    /// to stay within the available width. When disabled, the text remains
    /// on a single line, potentially overflowing the container if it
    /// is too long.
    #[builder(setter(strip_option), default)]
    #[gbuilder(default)]
    wrap: Option<bool>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(setter(strip_option), default)]
    margin: Option<Spacing>,

    /// Controls the horizontal distribution of text within its boundary.
    ///
    /// This tells the engine where to anchor the lines of text—whether they
    /// should hug the left side, sit in the center, or be pushed to the
    /// right. It can also spread the words out (space-between) to fill
    /// the entire width of the line.
    #[builder(setter(strip_option), default)]
    alignment: Option<TextAlignment>,

    /// The extra vertical space added between lines, measured in the same units as font size.
    ///
    /// This value is used to calculate a height multiplier for the text layout.
    /// For example, if the font size is 16 and line_spacing is 4, the total
    /// line height becomes 20 (a 1.25x multiplier).
    ///
    /// Note: Internally, this is converted to a ratio to satisfy the rendering
    /// engine's requirement for a line-height multiplier.
    #[builder(setter(strip_option), default)]
    line_spacing: Option<usize>,

    /// The foreground color applied to the text characters.
    ///
    /// This determines the color of the rendered glyphs (the letters and
    /// symbols) themselves. Unlike a background, this does not fill the
    /// bounding box; it only colors the "ink" used to draw the text.
    /// Use this to ensure your text has enough contrast against the
    /// background it is sitting on.
    #[builder(setter(strip_option), default)]
    color: Option<Color>,

    /// The actual text data that this widget is responsible for rendering.
    ///
    /// This field holds the characters and formatting instructions that
    /// make up your message. Whether it is a simple label or a complex
    /// paragraph with mixed styles, this is the primary source of
    /// information the widget uses to draw glyphs on the screen.
    #[builder(default)]
    #[gbuilder(default)]
    content: text::Text,

    /// A shared reference to the system's global font registry.
    ///
    /// Instead of each widget maintaining its own expensive font data, this
    /// field holds a reference-counted handle to a central collection. This
    /// allows the widget to efficiently resolve font styles and re-render
    /// text—such as when changing colors—without the memory overhead of
    /// duplicating font resources.
    #[builder(private, default = None)]
    #[gbuilder(hidden, default(None))]
    font_collection: Option<skia_safe::textlayout::FontCollection>,

    /// The compiled layout of the text, used for rendering and hit-testing.
    ///
    /// This field acts as the "brain" of the text widget once it has been
    /// processed. If the text has been compiled, this struct provides
    /// precise data about the line breaks, glyph positions, and the total
    /// area the text occupies. It is essential for interactive tasks,
    /// such as determining if a user is hovering over a specific link or
    /// character.
    #[builder(private, default = None)]
    #[gbuilder(hidden, default(None))]
    paragraph: Option<skia_safe::textlayout::Paragraph>,

    /// A cached count of the lines required to display the current text.
    ///
    /// This field stores the calculated line count to avoid redundant
    /// computations during the drawing stage. It is particularly useful
    /// when the text exceeds its available space, allowing the widget
    /// to quickly rebuild its layout or apply wrapping rules without
    /// starting from scratch.
    #[builder(private, default)]
    #[gbuilder(hidden, default)]
    total_lines: Option<usize>,

    /// The final dimensions that the text widget occupies within the layout.
    ///
    /// While a widget might start by trying to fill all available space,
    /// this field tracks the actual footprint the text takes up after
    /// accounting for constraints and internal margins. It serves as the
    /// source of truth for the layout engine to ensure the widget is
    /// positioned correctly relative to its neighbors.
    #[builder(private, default)]
    #[gbuilder(hidden, default)]
    extent: Extent2D<usize>,
}

make_configuration! {
    /// A targeted configuration set used to override or provide specific
    /// parameters for a Text widget based on its unique identifier.
    ///
    /// This allows for precise control over text properties—such as font
    /// choice, wrapping, and color—by associating these values with
    /// a specific widget ID. It enables the caller to update the visual
    /// presentation of text elements independently of the widget's
    /// initial construction, ensuring a clean separation between the
    /// layout structure and the final styling data.
    #[derive(Debug, Clone)]
    pub struct TextConfiguration {
        pub font: Font,
        pub wrap: bool,
        pub margin: Spacing,
        pub alignment: TextAlignment,
        pub line_spacing: usize,
        pub color: Color,
    } <<= Text
}

/// A collection of typographic settings that define the "look" and scale of a typeface.
///
/// Instead of managing font names and sizes separately, this struct groups
/// everything needed to tell the rendering engine exactly how to draw
/// each character. It covers the font family, the physical scale in points,
/// and the specific weight or slant of the text.
#[derive(macros::GenericBuilder, Debug, Clone)]
#[gbuilder(name(FontGBuilder), derive(Clone))]
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

impl Clone for Text {
    fn clone(&self) -> Self {
        // INFO: we shouldn't clone compiled info about text
        Self {
            id: self.id.clone(),
            font: self.font.clone(),
            wrap: self.wrap,
            margin: self.margin,
            alignment: self.alignment.clone(),
            color: self.color.clone(),
            content: self.content.clone(),
            line_spacing: self.line_spacing,
            font_collection: None,
            paragraph: None,
            total_lines: None,
            extent: Extent2D::default(),
        }
    }
}

impl Clone for TextGBuilder {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            font: self.font.clone(),
            wrap: self.wrap,
            margin: self.margin,
            alignment: self.alignment.clone(),
            color: self.color.clone(),
            content: self.content.clone(),
            line_spacing: self.line_spacing,
            font_collection: None,
            paragraph: None,
            total_lines: None,
            extent: Some(Extent2D::default()),
        }
    }
}

impl Text {
    // INFO: better to pass it instead of `usize::MAX` because skia's textlayout module
    // with ellipsis will make the layout in strange way — just truncate the first line
    // with available space.
    const MAX_LINES: usize = 100_000;

    pub fn new() -> Self {
        Self {
            content: text::Text::new_empty(),
            paragraph: None,
            extent: Extent2D::default(),
            ..Default::default()
        }
    }
}

impl WidgetInfo for Text {
    fn get_type(&self) -> &'static str {
        "text"
    }

    /// Returns the width + horizontal margins of the text container.
    ///
    /// Unlike [`Self::height`], this does not reflect the intrinsic text width,
    /// but rather the allocated width for text layout, since text widgets
    /// are expected to fill the entire available horizontal space.
    fn width(&self) -> usize {
        // INFO: the width should get all available width but height should get only renderable
        // rows.
        self.extent.width + self.margin.unwrap_or_default().horizontal()
    }

    /// Returns the computed height of the text after compilation.
    ///
    /// The result reflects the actual paragraph height + vertical margins, which may be
    /// smaller than the available area if the text fits without overflow.
    fn height(&self) -> usize {
        self.paragraph
            .as_ref()
            .map(|para| para.height() + self.margin.unwrap_or_default().vertical() as f32)
            .unwrap_or(0.) as usize
    }
}

impl Compile for Text {
    /// Prepares the widget for rendering by applying layout constraints
    /// and external data.
    ///
    /// This step computes the final layout using the provided [`Extent2D<usize>`]
    /// for spatial limits and the [`CompileCtx`] for shared resources
    /// and ID-linked configurations.
    fn compile(
        &mut self,
        mut available_extent: Extent2D<usize>,
        compile_ctx: &mut CompileCtx,
    ) -> CompileState {
        if self.id.is_empty() {
            self.id = compile_ctx.generate_new_id(self.get_type());
        }

        let associated_data = compile_ctx.data_pool.get(&self.id);

        if let Some(WidgetData::Text(text)) =
            associated_data.and_then(|associated| associated.data.as_ref())
        {
            self.content = text.clone();
        }

        if let Some(WidgetConfig::Text(text_config)) =
            associated_data.and_then(|associated| associated.config.as_ref())
        {
            self.configure(text_config.clone());
        }

        if self.content.body.as_str().trim().is_empty() {
            warn!("The text is blank");
            return CompileState::Failure;
        }

        available_extent.shrink_by(&self.margin.unwrap_or_default());

        let font = self.font.clone().unwrap_or_default();
        self.font_collection = compile_ctx.font_collection.clone().into();

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
        paragraph.layout(available_extent.width as f32);

        if paragraph.height() > available_extent.height as f32 {
            paragraph = match self.try_fit_paragraph(paragraph, available_extent, &text_style) {
                Some(paragraph) => paragraph,
                None => {
                    warn!(
                        "The text doesn't fit to available space. \
                Available space: width={}, height={}.",
                        available_extent.width, available_extent.height
                    );
                    return CompileState::Failure;
                }
            }
        }

        self.extent = available_extent;
        self.paragraph = Some(paragraph);
        CompileState::Success
    }
}

impl Text {
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
        &mut self,
        paragraph: skia_safe::textlayout::Paragraph,
        available_space: Extent2D<usize>,
        base_text_style: &skia_safe::textlayout::TextStyle,
    ) -> Option<skia_safe::textlayout::Paragraph> {
        let mut height = paragraph.height();
        let line_metrics = paragraph.get_line_metrics();
        let mut total_lines = line_metrics.len();

        for last_line in line_metrics.into_iter().rev() {
            height -= last_line.height as f32;
            total_lines -= 1;

            if height <= available_space.height as f32 {
                break;
            }
        }

        if total_lines == 0 {
            None
        } else {
            self.total_lines = Some(total_lines);
            let mut fitted_paragraph = self.build_paragraph(base_text_style, total_lines);
            fitted_paragraph.layout(available_space.width as f32);
            Some(fitted_paragraph)
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

        let text = &self.content.body;

        let mut cursor = 0;
        let mut end_stack = vec![text.len()];
        let mut current_entity_index = 0;

        while cursor < text.len() {
            let nearest_end = unsafe { *end_stack.last().unwrap_unchecked() };
            let entity = match self.content.entities.get(current_entity_index) {
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

impl Draw for Text {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let correct_offset: Offset<f32> = offset.into();

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
            &self.color.as_ref().cloned().unwrap_or_default(),
            correct_offset,
            self.extent.into(),
        );

        base_text_style.set_foreground_paint(&paint);

        let mut paragraph = if let Some(total_lines) = self.total_lines {
            self.build_paragraph(&base_text_style, total_lines)
        } else {
            self.build_paragraph(&base_text_style, Self::MAX_LINES)
        };
        paragraph.layout(self.extent.width as f32);

        let canvas = drawer.surface.canvas();
        paragraph.paint(canvas, (correct_offset.x, correct_offset.y));
    }
}

impl DispatchEvent for Text {
    fn dispatch_event(&self, _event: Event) -> Action {
        // TODO: implement link click
        Action::None
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
