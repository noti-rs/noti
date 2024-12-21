use config::text::TextProperty;
use dbus::text::Text;
use log::warn;
use shared::{error::ConversionError, value::TryFromValue};

use crate::{
    color::Bgra,
    drawer::Drawer,
    text::TextRect,
    types::{Offset, RectSize},
};

use super::{CompileState, Draw, WidgetConfiguration};

#[derive(macros::GenericBuilder)]
#[gbuilder(name(GBuilderWText))]
pub struct WText {
    kind: WTextKind,
    #[gbuilder(hidden, default(None))]
    content: Option<TextRect>,

    #[gbuilder(default)]
    property: TextProperty,
}

impl Clone for WText {
    fn clone(&self) -> Self {
        // INFO: we shouldn't clone compiled info about text
        Self {
            kind: self.kind.clone(),
            content: None,
            property: self.property.clone(),
        }
    }
}

impl Clone for GBuilderWText {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.as_ref().cloned(),
            content: None,
            property: self.property.as_ref().cloned(),
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
            content: None,
            property: Default::default(),
        }
    }

    pub fn compile(
        &mut self,
        rect_size: RectSize,
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
        let foreground = Bgra::from(&colors.foreground);

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

        let px_size = self.property.font_size as f32;
        let mut content = match notification_content {
            NotificationContent::Text(text) => {
                TextRect::from_text(text, px_size, &self.property.style, font_collection)
            }
            NotificationContent::String(str) => {
                TextRect::from_str(str, px_size, &self.property.style, font_collection)
            }
        };

        Self::apply_properties(&mut content, &self.property);
        Self::apply_color(&mut content, foreground);

        content.compile(rect_size.clone());
        if content.is_empty() {
            warn!(
                "The text with kind {} doesn't fit to available space. \
                Available space: width={}, height={}.",
                self.kind, rect_size.width, rect_size.height
            );
            CompileState::Failure
        } else {
            self.content = Some(content);
            CompileState::Success
        }
    }

    fn apply_properties(element: &mut TextRect, properties: &TextProperty) {
        element.set_wrap(properties.wrap);
        element.set_margin(&properties.margin);
        element.set_line_spacing(properties.line_spacing as usize);
        element.set_ellipsize_at(&properties.ellipsize_at);
        element.set_justification(&properties.justification);
    }

    fn apply_color(element: &mut TextRect, foreground: Bgra) {
        element.set_foreground(foreground);
    }

    pub fn width(&self) -> usize {
        self.content
            .as_ref()
            .map(|content| content.width())
            .unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.content
            .as_ref()
            .map(|content| content.height())
            .unwrap_or(0)
    }
}

impl Draw for WText {
    fn draw_with_offset(&self, offset: &Offset, drawer: &mut Drawer) {
        if let Some(content) = self.content.as_ref() {
            content.draw_with_offset(offset, drawer)
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
