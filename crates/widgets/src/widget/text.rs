use config::text::{self, GBuilderTextProperty, TextProperty};
use dbus::text::{Entity, EntityKind, Text};
use log::warn;
use shared::{error::ConversionError, value::TryFromValue};
use skia_safe::{textlayout::FontCollection, Color};

use crate::{
    color::Bgra,
    drawer::Drawer,
    events::{Action, DispatchEvent, Event},
    types::{extent::Extent2D, offset::Offset},
    Compile, CompileState, Draw, WidgetConfiguration,
};

/// A text widget that manages layout, styling, and rendering of text
/// content within the UI.
///
/// `WText` aims to be simple but flexible, providing a consistent API
/// for compilation and querying its dimensions after layout.
#[derive(macros::GenericBuilder)]
#[gbuilder(name(GBuilderWText))]
pub struct WText {
    kind: WTextKind,

    #[gbuilder(hidden, default(None))]
    paragraph: Option<skia_safe::textlayout::Paragraph>,

    #[gbuilder(use_gbuilder(GBuilderTextProperty), default)]
    property: TextProperty,

    #[gbuilder(hidden, default)]
    extent: Extent2D<usize>,
}

impl Clone for WText {
    fn clone(&self) -> Self {
        // INFO: we shouldn't clone compiled info about text
        Self {
            kind: self.kind.clone(),
            paragraph: None,
            property: self.property.clone(),
            extent: Extent2D::default(),
        }
    }
}

impl Clone for GBuilderWText {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.as_ref().cloned(),
            paragraph: None,
            property: self.property.clone(),
            extent: Some(Extent2D::default()),
        }
    }
}

/// Describes what kind of text a `WText` widget represents.
///
/// Currently there are only two kinds:
/// * `Summary` — the notification title, plain text without HTML markup.
/// * `Body` — the notification body, which may contain HTML tags,
///   entities, and complex styling.
///
/// # Why only two kinds?
/// In the context of desktop notifications, there is usually a short
/// title (summary) and a longer message (body). Supporting just these
/// two kinds simplifies styling, layout, and rendering logic while
/// still covering the common use cases.
#[derive(Clone, derive_more::Display)]
pub enum WTextKind {
    #[display("summary")]
    Summary,
    #[display("body")]
    Body,
}

impl TryFromValue for WTextKind {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "summary" => WTextKind::Summary,
            "body" => WTextKind::Body,
            _ => Err(ConversionError::InvalidValue {
                expected: "summary or body",
                actual: value,
            })?,
        })
    }
}

impl WText {
    pub fn new(kind: WTextKind) -> Self {
        Self {
            kind,
            paragraph: None,
            property: Default::default(),
            extent: Extent2D::default(),
        }
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
        paragraph: skia_safe::textlayout::Paragraph,
        restricted_space: Extent2D<usize>,
        notification_content: &NotificationContent,
        base_text_style: &skia_safe::textlayout::TextStyle,
        font_collection: FontCollection,
    ) -> Option<skia_safe::textlayout::Paragraph> {
        let mut height = paragraph.height();
        let line_metrics = paragraph.get_line_metrics();
        let mut total_lines = line_metrics.len();

        for last_line in line_metrics.into_iter().rev() {
            height -= last_line.height as f32;
            total_lines -= 1;

            if height <= restricted_space.height as f32 {
                break;
            }
        }

        if total_lines == 0 {
            None
        } else {
            let mut fitted_paragraph = self.build_paragraph(
                notification_content,
                base_text_style,
                font_collection.clone(),
                total_lines,
            );
            fitted_paragraph.layout(restricted_space.width as f32);
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
        notification_content: &NotificationContent,
        base_text_style: &skia_safe::textlayout::TextStyle,
        font_collection: FontCollection,
        max_lines: usize,
    ) -> skia_safe::textlayout::Paragraph {
        let paragraph_style = self.make_paragraph_style(max_lines);

        let mut paragraph_builder =
            skia_safe::textlayout::ParagraphBuilder::new(&paragraph_style, font_collection);
        paragraph_builder.push_style(base_text_style);

        let notification_text = notification_content.as_str();
        match notification_content.entities() {
            Some(entities) => {
                let mut cursor = 0;
                let mut end_stack = vec![notification_text.len()];
                let mut current_entity_index = 0;

                while cursor < notification_text.len() {
                    let nearest_end = unsafe { *end_stack.last().unwrap_unchecked() };
                    let entity = match entities.get(current_entity_index) {
                        Some(entity) => entity,
                        None => {
                            paragraph_builder.add_text(&notification_text[cursor..nearest_end]);
                            paragraph_builder.pop();
                            cursor = nearest_end;
                            end_stack.pop();
                            continue;
                        }
                    };

                    if entity.offset_in_byte > nearest_end {
                        paragraph_builder.add_text(&notification_text[cursor..nearest_end]);
                        paragraph_builder.pop();
                        cursor = nearest_end;
                        end_stack.pop();
                    } else {
                        paragraph_builder
                            .add_text(&notification_text[cursor..entity.offset_in_byte]);
                        cursor = entity.offset_in_byte;
                        end_stack.push(entity.offset_in_byte + entity.length_in_byte);
                        current_entity_index += 1;

                        let text_style = paragraph_builder.overlay_style(entity);
                        paragraph_builder.push_style(&text_style);
                    }
                }
            }
            None => {
                paragraph_builder.add_text(notification_text);
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

        let max_lines = if self.property.wrap { max_lines } else { 1 };
        paragraph_style.set_max_lines(max_lines);
        paragraph_style.set_ellipsis("…");

        let text_align = match self.property.alignment {
            text::TextAlignment::Justify => skia_safe::textlayout::TextAlign::Justify,
            text::TextAlignment::Center => skia_safe::textlayout::TextAlign::Center,
            text::TextAlignment::Left => skia_safe::textlayout::TextAlign::Left,
            text::TextAlignment::Right => skia_safe::textlayout::TextAlign::Right,
        };
        paragraph_style.set_text_align(text_align);

        {
            let mut strut_style = skia_safe::textlayout::StrutStyle::new();
            strut_style.set_strut_enabled(true);
            strut_style.set_font_size(self.property.font_size as f32);
            strut_style.set_height(
                self.property.line_spacing as f32 / self.property.font_size as f32 + 1.0,
            );
            strut_style.set_height_override(true);

            paragraph_style.set_strut_style(strut_style);
        }

        paragraph_style
    }

    /// Returns the width + horizontal margins of the text container.
    ///
    /// Unlike [`Self::height`], this does not reflect the intrinsic text width,
    /// but rather the allocated width for text layout, since text widgets
    /// are expected to fill the entire available horizontal space.
    pub fn width(&self) -> usize {
        // INFO: the width should get all available width but height should get only renderable
        // rows.
        self.extent.width + self.property.margin.horizontal() as usize
    }

    /// Returns the computed height of the text after compilation.
    ///
    /// The result reflects the actual paragraph height + vertical margins, which may be
    /// smaller than the available area if the text fits without overflow.
    pub fn height(&self) -> usize {
        self.paragraph
            .as_ref()
            .map(|para| para.height() + self.property.margin.vertical() as f32)
            .unwrap_or(0.) as usize
    }
}

impl Compile for WText {
    /// Prepares the text for rendering by building a Skia `Paragraph`
    /// with the correct style and layout constraints.
    ///
    /// This step computes the layout according to the available space
    /// [`Extent2D<usize>`] and applies the provided [`WidgetConfiguration`].
    /// After compilation, the widget knows exactly how much space the
    /// text will occupy and can be drawn at the right position.
    ///
    /// Must be called before querying the text's dimensions or drawing it.
    fn compile(
        &mut self,
        mut available_extent: Extent2D<usize>,
        WidgetConfiguration {
            display_config,
            notification,
            font_collection,
            override_properties,
            theme,
        }: &WidgetConfiguration,
    ) -> CompileState {
        let mut override_if = |r#override: bool, property: &TextProperty| {
            if r#override {
                self.property = property.clone()
            }
        };

        let colors = theme.by_urgency(&notification.hints.urgency);
        let foreground: Bgra<u8> = colors.foreground.clone().into();

        let notification_content: NotificationContent = match self.kind {
            WTextKind::Summary => {
                override_if(*override_properties, &display_config.summary);
                notification.summary.as_str().into()
            }
            WTextKind::Body => {
                override_if(*override_properties, &display_config.body);
                if display_config.markup {
                    (&notification.body).into()
                } else {
                    notification.body.body.as_str().into()
                }
            }
        };

        if notification_content.as_str().trim().is_empty() {
            warn!("The text with kind {} is blank", self.kind);
            return CompileState::Failure;
        }

        let mut text_style = skia_safe::textlayout::TextStyle::new();
        text_style.set_font_families(&[&self.property.font.name]);
        text_style.set_color(Color::from_argb(
            foreground.alpha,
            foreground.red,
            foreground.green,
            foreground.blue,
        ));

        text_style.set_font_style(match self.property.style {
            text::TextStyle::Regular => skia_safe::FontStyle::normal(),
            text::TextStyle::Bold => skia_safe::FontStyle::bold(),
            text::TextStyle::Italic => skia_safe::FontStyle::italic(),
            text::TextStyle::BoldItalic => skia_safe::FontStyle::bold_italic(),
        });

        available_extent.shrink_by(&self.property.margin);

        // INFO: better to pass it instead of `usize::MAX` because skia's textlayout module
        // with ellipsis will make the layout in strange way — just truncate the first line
        // with available space.
        const MAX_LINES: usize = 100_000;
        let mut paragraph = self.build_paragraph(
            &notification_content,
            &text_style,
            font_collection.clone(),
            MAX_LINES,
        );
        paragraph.layout(available_extent.width as f32);

        if paragraph.height() > available_extent.height as f32 {
            paragraph = match self.try_fit_paragraph(
                paragraph,
                available_extent,
                &notification_content,
                &text_style,
                font_collection.clone(),
            ) {
                Some(paragraph) => paragraph,
                None => {
                    warn!(
                        "The text with kind {} doesn't fit to available space. \
                Available space: width={}, height={}.",
                        self.kind, available_extent.width, available_extent.height
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

impl Draw for WText {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        if let Some(paragraph) = &self.paragraph {
            let correct_offset: Offset<f32> = (*offset).into();
            paragraph.paint(
                drawer.surface.canvas(),
                (correct_offset.x, correct_offset.y),
            );
        }
    }
}

impl DispatchEvent for WText {
    fn dispatch_event(&self, _event: Event) -> Action {
        // TODO: implement link click
        Action::None
    }
}

/// Represents the content of a notification, abstracting over whether
/// the text is plain or contains entities for styled rendering.
///
/// This type makes it easy to handle both cases uniformly: callers can
/// access the raw string via [`as_str`] or get structured entities via
/// [`entities`] when available.
enum NotificationContent<'a> {
    String(&'a str),
    Text(&'a Text),
}

impl NotificationContent<'_> {
    /// Returns the raw text content, regardless of whether it's a simple
    /// string or a `Text` object with entities.
    fn as_str(&self) -> &str {
        match self {
            NotificationContent::String(string) => string,
            NotificationContent::Text(text) => &text.body,
        }
    }

    /// Returns the list of entities if the content supports styling.
    /// Returns `None` for plain string content.
    fn entities(&self) -> Option<&[Entity]> {
        match self {
            NotificationContent::String(_) => None,
            NotificationContent::Text(text) => Some(&text.entities),
        }
    }
}

impl<'a> From<&'a str> for NotificationContent<'a> {
    fn from(value: &'a str) -> Self {
        NotificationContent::String(value)
    }
}

impl<'a> From<&'a Text> for NotificationContent<'a> {
    fn from(value: &'a Text) -> Self {
        NotificationContent::Text(value)
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
