use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use config::text::{GBuilderTextProperty, TextProperty};
use dbus::text::{EntityKind, Text};
use log::warn;
use pangocairo::{
    pango::{
        ffi::PANGO_SCALE, AttrColor, AttrInt, AttrList, AttrSize, Context, FontDescription,
        Layout as PangoLayout,
    },
    FontMap,
};
use shared::{error::ConversionError, value::TryFromValue};

use crate::{
    color::Bgra,
    drawer::Drawer,
    types::{Offset, RectSize},
};

use super::{CompileState, Draw, WidgetConfiguration};

#[derive(macros::GenericBuilder)]
#[gbuilder(name(GBuilderWText))]
pub struct WText {
    kind: WTextKind,

    // INFO: using here Arc<Mutex<T>> is not mean that drawing is multithreading. It is made for
    // safe usage when layout creates in filetype crate.
    #[gbuilder(hidden, default(None))]
    layout: Option<Arc<Mutex<Layout>>>,

    #[gbuilder(use_gbuilder(GBuilderTextProperty), default)]
    property: TextProperty,

    #[gbuilder(hidden, default)]
    inner_size: RectSize<usize>,
}

impl Clone for WText {
    fn clone(&self) -> Self {
        // INFO: we shouldn't clone compiled info about text
        Self {
            kind: self.kind.clone(),
            layout: None,
            property: self.property.clone(),
            inner_size: RectSize::default(),
        }
    }
}

impl Clone for GBuilderWText {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.as_ref().cloned(),
            layout: None,
            property: self.property.clone(),
            inner_size: Some(RectSize::default()),
        }
    }
}

#[derive(Clone, derive_more::Display)]
pub enum WTextKind {
    #[display("title")]
    Title,
    #[display("body")]
    Body,
}

impl TryFromValue for WTextKind {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "title" | "summary" => WTextKind::Title,
            "body" => WTextKind::Body,
            _ => Err(ConversionError::InvalidValue {
                expected: "title or body",
                actual: value,
            })?,
        })
    }
}

impl WText {
    pub fn new(kind: WTextKind) -> Self {
        Self {
            kind,
            layout: None,
            property: Default::default(),
            inner_size: RectSize::default(),
        }
    }

    pub fn compile(
        &mut self,
        mut rect_size: RectSize<usize>,
        WidgetConfiguration {
            display_config,
            notification,
            pango_context,
            override_properties,
            theme,
        }: &WidgetConfiguration,
    ) -> CompileState {
        let mut override_if = |r#override: bool, property: &TextProperty| {
            if r#override {
                self.property = property.clone()
            }
        };

        let layout = Layout {
            pango_layout: PangoLayout::new(&pango_context.0),
        };

        let colors = theme.by_urgency(&notification.hints.urgency);
        let foreground: Bgra<f64> = colors.foreground.clone().into();

        let notification_content: NotificationContent = match self.kind {
            WTextKind::Title => {
                override_if(*override_properties, &display_config.title);
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

        rect_size.shrink_by(&self.property.margin);
        layout.set_width(rect_size.width as i32 * PANGO_SCALE);
        layout.set_height(rect_size.height as i32 * PANGO_SCALE);

        let (text, attributes) = notification_content.as_str_with_attributes();
        if text.trim().is_empty() {
            warn!("The text with kind {} is blank", self.kind);
            return CompileState::Failure;
        }

        layout.set_text(text);
        Self::apply_colors(&attributes, foreground.into());
        self.apply_properties(&layout, attributes);

        let (computed_width, computed_height) = layout.pixel_size();
        if computed_width > rect_size.width as i32 || computed_height > rect_size.height as i32 {
            warn!(
                "The text with kind {} doesn't fit to available space. \
                Available space: width={}, height={}.",
                self.kind, rect_size.width, rect_size.height
            );
            CompileState::Failure
        } else {
            self.inner_size = rect_size;
            self.layout = Some(Arc::new(Mutex::new(layout)));
            CompileState::Success
        }
    }

    fn apply_colors(attributes: &AttrList, foreground: Bgra<u16>) {
        attributes.insert(AttrColor::new_foreground(
            foreground.red,
            foreground.green,
            foreground.blue,
        ));
        attributes.insert(AttrInt::new_foreground_alpha(foreground.alpha));
    }

    fn apply_properties(&self, layout: &PangoLayout, attributes: AttrList) {
        fn from_px_to_pt(px: f32) -> i32 {
            ((px * 72.0) / 96.0).round() as i32
        }

        attributes.insert(AttrSize::new_size_absolute(
            from_px_to_pt(self.property.font_size as f32) * PANGO_SCALE,
        ));

        match &self.property.style {
            config::text::TextStyle::Regular => (),
            config::text::TextStyle::Bold => {
                attributes.insert(AttrInt::new_weight(pangocairo::pango::Weight::Bold))
            }
            config::text::TextStyle::Italic => {
                attributes.insert(AttrInt::new_style(pangocairo::pango::Style::Italic))
            }
            config::text::TextStyle::BoldItalic => {
                attributes.insert(AttrInt::new_weight(pangocairo::pango::Weight::Bold));
                attributes.insert(AttrInt::new_style(pangocairo::pango::Style::Italic));
            }
        }

        if self.property.wrap {
            layout.set_wrap(pangocairo::pango::WrapMode::Word);
        }

        let ellipsize = match self.property.ellipsize {
            config::text::Ellipsize::Start => pangocairo::pango::EllipsizeMode::Start,
            config::text::Ellipsize::Middle => pangocairo::pango::EllipsizeMode::Middle,
            config::text::Ellipsize::End => pangocairo::pango::EllipsizeMode::End,
            config::text::Ellipsize::None => pangocairo::pango::EllipsizeMode::None,
        };
        layout.set_ellipsize(ellipsize);

        let alignment = match self.property.alignment {
            config::text::TextAlignment::Center => pangocairo::pango::Alignment::Center,
            config::text::TextAlignment::Left => pangocairo::pango::Alignment::Left,
            config::text::TextAlignment::Right => pangocairo::pango::Alignment::Right,
        };
        layout.set_alignment(alignment);
        layout.set_justify(self.property.jutsify);
        layout.set_spacing(self.property.line_spacing as i32 * PANGO_SCALE);

        layout.set_attributes(Some(&attributes));
    }

    pub fn width(&self) -> usize {
        // INFO: the width should get all available width but height should get only renderable
        // rows.
        self.inner_size.width + self.property.margin.horizontal() as usize
    }

    pub fn height(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| {
                layout.lock().unwrap().pixel_size().1 + self.property.margin.vertical() as i32
            })
            .unwrap_or(0) as usize
    }
}

impl Draw for WText {
    fn draw_with_offset(
        &mut self,
        offset: &Offset<usize>,
        drawer: &mut Drawer,
    ) -> pangocairo::cairo::Result<()> {
        if let Some(layout) = self.layout.as_ref() {
            let layout = layout.lock().unwrap();
            //TODO: try to inject here pango context for better result
            drawer.context.move_to(
                (offset.x + self.property.margin.left() as usize) as f64,
                (offset.y + self.property.margin.top() as usize) as f64,
            );
            pangocairo::functions::show_layout(&drawer.context, &layout);
        }
        Ok(())
    }
}

enum NotificationContent<'a> {
    String(&'a str),
    Text(&'a Text),
}

impl NotificationContent<'_> {
    fn as_str_with_attributes(&self) -> (&str, AttrList) {
        fn get_attribute_style(kind: &EntityKind) -> Option<AttrInt> {
            Some(match kind {
                dbus::text::EntityKind::Bold => {
                    AttrInt::new_weight(pangocairo::pango::Weight::Bold)
                }
                dbus::text::EntityKind::Italic => {
                    AttrInt::new_style(pangocairo::pango::Style::Italic)
                }
                dbus::text::EntityKind::Underline => {
                    AttrInt::new_underline(pangocairo::pango::Underline::SingleLine)
                }
                _ => None?, // Images and Links will be ignored because they're useless
                            // for pango
            })
        }

        let string;
        let attributes = AttrList::new();
        match self {
            NotificationContent::Text(text) => {
                string = &*text.body;

                for entity in &text.entities {
                    let Some(mut attribute) = get_attribute_style(&entity.kind) else {
                        continue;
                    };

                    attribute.set_start_index(entity.offset_in_byte as u32);
                    attribute.set_end_index((entity.offset_in_byte + entity.length_in_byte) as u32);
                    attributes.insert(attribute);
                }
            }
            NotificationContent::String(str) => {
                string = *str;
            }
        }

        (string, attributes)
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

struct Layout {
    pango_layout: PangoLayout,
}

// WARN: unlike there is a Send trait that marks that the object is safe to send into another
// thread, it is not. And it made for internal purpose - using in Box<dyn Any> for filetype crate.
// Unfortunately, there yet is no effective way to deal with it.
unsafe impl Send for Layout {}

impl Deref for Layout {
    type Target = PangoLayout;

    fn deref(&self) -> &Self::Target {
        &self.pango_layout
    }
}

pub struct PangoContext(Context);

impl PangoContext {
    pub fn from_font_family(font_family: &str) -> Self {
        let context = Context::new();
        let font_map = FontMap::new();
        context.set_font_map(Some(&font_map));
        context.set_font_description(Some(&FontDescription::from_string(font_family)));

        Self(context)
    }

    pub fn update_font_family(&mut self, font_family: &str) {
        self.0
            .set_font_description(Some(&FontDescription::from_string(font_family)));
    }
}
