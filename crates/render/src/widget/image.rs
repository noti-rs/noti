use config::display::{GBuilderImageProperty, ImageProperty};
use log::warn;

use crate::{
    drawer::Drawer,
    image::Image,
    types::{Offset, RectSize},
};

use super::{CompileState, Draw, WidgetConfiguration};

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
                display_config
                    .icons
                    .size
                    .iter()
                    .find_map(|size| {
                        freedesktop_icons::lookup(&notification.app_icon)
                            .with_size(*size)
                            .with_theme(&display_config.theme)
                            .find()
                    })
                    .map(|icon_path| Image::from_path(&icon_path, &self.property, &rect_size))
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

    pub fn width(&self) -> usize {
        self.width
    }

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
    fn draw_with_offset(&mut self, offset: &Offset<usize>, drawer: &mut Drawer) -> pangocairo::cairo::Result<()> {
        if !self.content.is_exists() {
            return Ok(());
        }

        // INFO: The ImageProperty initializes with Image so we can calmly unwrap
        let offset = Offset::from(&self.property.margin) + *offset;
        self.content.draw_with_offset(&offset, drawer)
    }
}
