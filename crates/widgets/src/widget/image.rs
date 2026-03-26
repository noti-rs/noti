use config::display::{GBuilderImageProperty, ImageProperty};
use linicon::IconPath;
use log::warn;

use crate::{
    drawer::Drawer,
    events::{Action, DispatchEvent, Event},
    image::Image,
    types::{Offset, RectSize},
};

use crate::{CompileState, Draw, WidgetConfiguration};

const DEFAULT_ICON_THEME: &str = "hicolor";

/// A widget that displays an image within a notification layout.
///
/// `WImage` abstracts away the complexity of loading and preparing an image
/// for rendering. It can handle multiple image sources as defined by the
/// freedesktop notification specification, including application icons,
/// raw image data, and file paths.
///
/// This widget is responsible for:
/// - Selecting the most appropriate image source during compilation.
/// - Preparing the image for rendering (decoding and sizing).
/// - Reporting its final size after compilation so it can be positioned
///   correctly in the layout.
///
/// Typically used for application icons or media previews within a
/// notification banner.
#[derive(macros::GenericBuilder, Clone)]
#[gbuilder(name(GBuilderWImage), derive(Clone))]
pub struct WImage {
    #[gbuilder(hidden, default(Image::Unknown))]
    content: Image,

    #[gbuilder(hidden, default(0))]
    width: usize,
    #[gbuilder(hidden, default(0))]
    height: usize,

    #[gbuilder(use_gbuilder(GBuilderImageProperty), default)]
    property: ImageProperty,
}

impl WImage {
    pub fn new() -> Self {
        Self {
            content: Image::Unknown,
            width: 0,
            height: 0,
            property: Default::default(),
        }
    }

    /// Compiles the image widget by selecting and preparing the image to display.
    ///
    /// This method chooses the most appropriate image source (icon, raw data,
    /// or file path) based on the notification data and the freedesktop
    /// specification. After selection, it decodes the image and determines
    /// whether it can fit in the available space defined by [`RectSize`].
    ///
    /// # Returns
    /// Returns [`CompileState::Success`] if the image was successfully compiled.
    /// If the image cannot fit into the given space, [`CompileState::Failure`] is returned,
    /// allowing the caller to handle the case gracefully (e.g. omit the image).
    ///
    /// Call this before drawing or querying [`width`] and [`height`].
    pub fn compile(
        &mut self,
        rect_size: RectSize<usize>,
        WidgetConfiguration {
            notification,
            display_config,
            override_properties,
            ..
        }: &WidgetConfiguration,
    ) -> CompileState {
        /// Look's up nearest freedesktop icons.
        fn lookup_freedesktop_icon(icon_name: &str, theme: &str, size: u16) -> Option<IconPath> {
            linicon::lookup_icon(icon_name)
                .from_theme(theme)
                .with_size(size)
                .next()
                .and_then(|icon| icon.ok())
        }

        if *override_properties {
            self.property = display_config.image.clone();
        }

        self.content = notification
            .hints
            .image_data
            .as_ref()
            .cloned()
            .map(|image_data| Image::from_image_data(image_data, &self.property, &rect_size))
            .or_else(|| {
                notification
                    .hints
                    .image_path
                    .as_deref()
                    .map(std::path::Path::new)
                    .map(|svg_path| Image::from_svg(svg_path, &self.property, &rect_size))
            })
            .or_else(|| {
                if notification.app_icon.is_empty() {
                    return None;
                }

                display_config
                    .icons
                    .size
                    .iter()
                    .find_map(|size| {
                        lookup_freedesktop_icon(
                            &notification.app_icon,
                            &display_config.icons.theme,
                            *size,
                        )
                        .or_else(|| {
                            lookup_freedesktop_icon(
                                &notification.app_icon,
                                DEFAULT_ICON_THEME,
                                *size,
                            )
                        })
                    })
                    .map(|icon_path| Image::from_path(&icon_path.path, &self.property, &rect_size))
            })
            .unwrap_or(Image::Unknown);

        self.width = self
            .content
            .width()
            .map(|width| width + self.property.margin.horizontal() as usize)
            .unwrap_or(0);
        self.height = self
            .content
            .height()
            .map(|height| height + self.property.margin.vertical() as usize)
            .unwrap_or(0);

        if self.width > rect_size.width || self.height > rect_size.height {
            warn!(
                "The image doesn't fit to available space.\
                \nThe image size: width={}, height={}.\
                \nAvailable space: width={}, height={}.",
                self.width, self.height, rect_size.width, rect_size.height
            );
            return CompileState::Failure;
        }

        if self.content.is_exists() {
            CompileState::Success
        } else {
            CompileState::Failure
        }
    }

    /// Returns the width of the compiled image.
    ///
    /// This value is only meaningful after [`Self::compile`] has been called.
    /// If the widget has not been compiled yet, this will typically return
    /// an undefined or default value.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the compiled image.
    ///
    /// Like [`Self::width`], this is only meaningful after [`Self::compile`] has been
    /// successfully called.
    pub fn height(&self) -> usize {
        self.height
    }
}

impl Default for WImage {
    fn default() -> Self {
        Self::new()
    }
}

impl Draw for WImage {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        if !self.content.is_exists() {
            return;
        }

        let offset = Offset::from(&self.property.margin) + *offset;
        self.content.draw_with_offset(&offset, drawer)
    }
}

impl DispatchEvent for WImage {
    fn dispatch_event(&self, _event: Event) -> Action {
        Action::None
    }
}
