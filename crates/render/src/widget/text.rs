use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use config::text::{GBuilderTextProperty, TextProperty};
use dbus::text::Text;
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
}

impl Clone for WText {
    fn clone(&self) -> Self {
        // INFO: we shouldn't clone compiled info about text
        Self {
            kind: self.kind.clone(),
            layout: None,
            property: self.property.clone(),
        }
    }
}

impl Clone for GBuilderWText {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.as_ref().cloned(),
            layout: None,
            property: self.property.clone(),
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
        }
    }

    pub fn compile(
        &mut self,
        rect_size: RectSize,
        WidgetConfiguration {
            display_config,
            notification,
            pango_context,
            override_properties,
            theme,
        }: &WidgetConfiguration,
    ) -> CompileState {
        fn from_px_to_pt(px: f32) -> i32 {
            ((px * 72.0) / 96.0).round() as i32
        }

        let mut override_if = |r#override: bool, property: &TextProperty| {
            if r#override {
                self.property = property.clone()
            }
        };
        let layout = Layout {
            pango_layout: PangoLayout::new(&pango_context.0),
        };

        let colors = theme.by_urgency(&notification.hints.urgency);
        let foreground: Bgra = colors.foreground.clone().into();

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

        layout.set_width(rect_size.width as i32 * PANGO_SCALE);
        layout.set_height(rect_size.height as i32 * PANGO_SCALE);

        let attributes = AttrList::new();

        match notification_content {
            NotificationContent::Text(text) => {
                layout.set_text(&text.body);

                for entity in &text.entities {
                    let mut attribute = match entity.kind {
                        dbus::text::EntityKind::Bold => {
                            AttrInt::new_weight(pangocairo::pango::Weight::Bold)
                        }
                        dbus::text::EntityKind::Italic => {
                            AttrInt::new_style(pangocairo::pango::Style::Italic)
                        }
                        dbus::text::EntityKind::Underline => {
                            AttrInt::new_underline(pangocairo::pango::Underline::SingleLine)
                        }
                        _ => continue, // Images and Links will be ignored because they're useless
                                       // for pango
                    };
                    attribute.set_start_index(entity.offset_in_byte as u32);
                    attribute.set_end_index((entity.offset_in_byte + entity.length_in_byte) as u32);
                    attributes.insert(attribute);
                }
            }
            NotificationContent::String(str) => {
                layout.set_text(str);
            }
        }

        attributes.insert(AttrColor::new_foreground(
            (foreground.red as f64 * u16::MAX as f64).round() as u16,
            (foreground.green as f64 * u16::MAX as f64).round() as u16,
            (foreground.blue as f64 * u16::MAX as f64).round() as u16,
        ));
        attributes.insert(AttrInt::new_foreground_alpha(
            (foreground.alpha as f64 * u16::MAX as f64).round() as u16,
        ));

        attributes.insert(AttrSize::new(
            from_px_to_pt(self.property.font_size as f32) * PANGO_SCALE,
        ));
        attributes.insert(AttrInt::new_letter_spacing(PANGO_SCALE));
        layout.set_attributes(Some(&attributes));

        self.apply_properties(&layout);

        if layout.size().0 == 0 {
            warn!(
                "The text with kind {} doesn't fit to available space. \
                Available space: width={}, height={}.",
                self.kind, rect_size.width, rect_size.height
            );
            CompileState::Failure
        } else {
            self.layout = Some(Arc::new(Mutex::new(layout)));
            CompileState::Success
        }
    }

    fn apply_properties(&self, layout: &PangoLayout) {
        if self.property.wrap {
            layout.set_wrap(pangocairo::pango::WrapMode::Word);
        }
        layout.set_ellipsize(pangocairo::pango::EllipsizeMode::End);

        // TODO: add base style of text
        //
        // TODO: Carefully work with it in wigth/height layout computation
        // element.set_margin(&properties.margin);
        //
        // TODO: make a new property 'alignment' and modify `justification` to `justify`
        // layout.set_justify(true);
        // element.set_justification(&properties.justification);
        //
        // TODO: think about it
        // element.set_line_spacing(properties.line_spacing as usize);
        //
        // TODO: modify config property
        // element.set_ellipsize_at(&properties.ellipsize_at);
    }

    pub fn width(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.lock().unwrap().pixel_size().0)
            .unwrap_or(0) as usize
    }

    pub fn height(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.lock().unwrap().pixel_size().1)
            .unwrap_or(0) as usize
    }
}

impl Draw for WText {
    fn draw_with_offset(&mut self, offset: &Offset, drawer: &mut Drawer) {
        if let Some(layout) = self.layout.as_ref() {
            let layout = layout.lock().unwrap();
            drawer.context.move_to(offset.x as f64, offset.y as f64);
            pangocairo::functions::show_layout(&drawer.context, &layout);
        }
    }
}

enum NotificationContent<'a> {
    String(&'a str),
    Text(&'a Text),
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
