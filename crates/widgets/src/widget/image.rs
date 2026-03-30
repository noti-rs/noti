use linicon::IconPath;
use log::warn;

use crate::{
    drawer::Drawer,
    events::{Action, DispatchEvent, Event},
    image::{self, MipmapMode, ResizingMethod},
    make_configuration,
    types::{
        data::{Configure, ToConfig, WidgetConfig, WidgetData},
        extent::Extent2D,
        offset::Offset,
        spacing::Spacing,
        widget_id::WidgetId,
    },
    Compile, WidgetInfo,
};

use crate::{CompileCtx, CompileState, Draw};

const DEFAULT_ICON_THEME: &str = "hicolor";

/// A widget that handles the job of showing a picture.
///
/// Think of this as a "smart picture frame." Instead of you having to
/// figure out how to open a file or decode raw data yourself, you just
/// give this widget the "source" and it does the hard work for you.
///
/// You can give it a picture in three ways:
/// 1. **ImageData**: Raw bits and pieces of a picture you already have.
/// 2. **ImagePath**: A "map" (path) to where the picture lives on the computer.
/// 3. **Icon**: A simple symbol or a named icon.
///
/// This widget's main jobs are:
/// - Choosing the right way to load the picture you provided.
/// - Getting the picture ready to be drawn (like unpacking it and making it the right size).
/// - Telling the rest of the layout how much space it needs so everything stays organized.
#[derive(macros::GenericBuilder, derive_builder::Builder, Default, Clone)]
#[gbuilder(name(ImageGBuilder), derive(Clone))]
pub struct Image {
    /// An optional identifier for this widget.
    ///
    /// If left empty, an ID will be automatically generated during
    /// compilation. Setting this manually allows the widget to be
    /// targeted by external configurations and makes the widget tree
    /// significantly easier to navigate during debugging.
    #[builder(default, setter(into))]
    #[gbuilder(default)]
    id: WidgetId,

    /// The source data for the image being rendered.
    ///
    /// Unlike other widgets, this field is strictly populated via
    /// `WidgetData::Image` during the compilation phase. It holds the
    /// processed pixel data or file path information required to
    /// draw the image to the screen.
    #[builder(private, default = image::Image::Unknown)]
    #[gbuilder(hidden, default(image::Image::Unknown))]
    content: image::Image,

    /// The final calculated dimensions of the widget on the screen.
    ///
    /// This stores the actual width and height (as `Extent2D`) the
    /// image occupies after accounting for layout constraints,
    /// aspect ratios, and the `max_size` limit.
    #[builder(private, default)]
    #[gbuilder(hidden, default)]
    extent: Extent2D<usize>,

    /// The maximum allowable dimension for the image in any one direction.
    ///
    /// This ensures the image stays within a specific boundary. If the
    /// available space is a square, both sides are capped at this value.
    /// In rectangular spaces, the larger side is reduced to `max_size`,
    /// and the other side is scaled down proportionally to maintain
    /// the image's original aspect ratio.
    #[builder(setter(strip_option), default)]
    max_size: Option<u16>,

    /// The corner radius applied to the image's edges.
    ///
    /// This allows you to create rounded corners for the picture. Since
    /// images handle their own clipping independently of the standard
    /// `Border` struct, this field defines how much to "curve" the
    /// rectangular boundary of the image.
    #[builder(setter(strip_option), default)]
    rounding: Option<u16>,

    /// The internal spacing between the widget's boundary box and its actual content.
    ///
    /// This field defines a buffer zone (Top, Right, Bottom, Left) that
    /// effectively shrinks the available area for the widget's content
    /// without changing the widget's outer dimensions. It ensures
    /// content does not touch the edges of its container.
    #[builder(setter(strip_option), default)]
    margin: Option<Spacing>,

    /// The mathematical approach used to scale the image up or down.
    ///
    /// This determines how "smooth" or "sharp" the image looks when its
    /// final size doesn't match its original pixel dimensions. By default,
    /// it uses a linear approach to prevent jagged edges, but can be
    /// set to a simpler method for performance or specific aesthetic
    /// styles (like pixel art).
    #[builder(setter(strip_option), default)]
    resizing_method: Option<ResizingMethod>,

    /// The strategy for using pre-calculated, lower-resolution versions
    /// of the image.
    ///
    /// When an image is significantly shrunk, "shimmering" or visual noise
    /// can occur. This field tells the engine how to sample from these
    /// pre-scaled versions (mipmaps) to ensure the image remains clean
    /// and stable even at very small sizes.
    #[builder(setter(strip_option), default)]
    mipmap_mode: Option<MipmapMode>,
}

make_configuration! {
    /// A targeted configuration set used to override or provide specific
    /// parameters for an Image widget based on its unique identifier.
    ///
    /// This struct facilitates the fine-tuning of image-specific rendering
    /// and layout properties (like resizing methods or max dimensions)
    /// from outside the main widget tree. It is particularly useful for
    /// applying data-driven changes to specific images during the final
    /// layout pass without needing to manually find and update the widget node.
    #[derive(Debug, Clone)]
    pub struct ImageConfiguration {
        pub max_size: u16,
        pub rounding: u16,
        pub margin: Spacing,
        pub resizing_method: ResizingMethod,
        pub mipmap_mode: MipmapMode,
    } <<= Image
}

impl Image {
    pub fn new() -> Self {
        Self {
            content: image::Image::Unknown,
            ..Default::default()
        }
    }
}

impl WidgetInfo for Image {
    fn get_type(&self) -> &'static str {
        "image"
    }

    /// Returns the width of the image widget.
    ///
    /// This value is only meaningful after [`Self::compile`] has been called.
    /// If the widget has not been compiled yet, this will typically return
    /// an undefined or default value.
    fn width(&self) -> usize {
        self.extent.width
    }

    /// Returns the height of the image widget.
    ///
    /// Like [`Self::width`], this is only meaningful after [`Self::compile`] has been
    /// successfully called.
    fn height(&self) -> usize {
        self.extent.height
    }
}

impl Compile for Image {
    /// Prepares the image for rendering by resolving its data and calculating
    /// the final layout dimensions.
    ///
    /// This method attempts to retrieve image data from the provided [`CompileCtx`].
    /// Once the image is obtained, the widget calculates the optimal scale
    /// to fit within the provided [`Extent2D`] boundary, ensuring that
    /// `max_size` and aspect ratio constraints are respected.
    ///
    /// This must be called before attempting to draw the widget or querying its
    /// final width and height.
    fn compile(
        &mut self,
        available_extent: Extent2D<usize>,
        compile_ctx: &mut CompileCtx,
    ) -> CompileState {
        /// Look's up nearest freedesktop icons.
        fn lookup_freedesktop_icon(icon_name: &str, theme: &str, size: u16) -> Option<IconPath> {
            linicon::lookup_icon(icon_name)
                .from_theme(theme)
                .with_size(size)
                .next()
                .and_then(|icon| icon.ok())
        }

        if self.id.is_empty() {
            self.id = compile_ctx.generate_new_id(self.get_type());
        }

        let Some(associated_data) = compile_ctx.data_pool.get(&self.id) else {
            return CompileState::Failure;
        };

        if let Some(WidgetConfig::Image(image_config)) = &associated_data.config {
            self.configure(image_config.clone());
        }

        let Some(widget_data) = &associated_data.data else {
            return CompileState::Failure;
        };

        let image_configuration = self.to_config();
        self.content = match widget_data {
            WidgetData::ImageData(image_data) => {
                image::Image::from_image_data(image_data, &image_configuration, &available_extent)
            }
            WidgetData::ImagePath(image_path) => {
                image::Image::from_path(image_path, &image_configuration, &available_extent)
            }
            WidgetData::Icon { name, theme, sizes } => {
                let mut sizes = sizes.clone();
                sizes.sort();
                sizes
                    .into_iter()
                    .rev()
                    .find_map(|size| {
                        lookup_freedesktop_icon(name, theme, size)
                            .or_else(|| lookup_freedesktop_icon(name, DEFAULT_ICON_THEME, size))
                    })
                    .map(|icon_path| {
                        image::Image::from_path(
                            &icon_path.path,
                            &image_configuration,
                            &available_extent,
                        )
                    })
                    .unwrap_or(image::Image::Unknown)
            }
            _ => return CompileState::Failure,
        };

        let margin = self.margin.unwrap_or_default();
        self.extent.width = self
            .content
            .width()
            .map(|width| width + margin.horizontal())
            .unwrap_or(0);
        self.extent.height = self
            .content
            .height()
            .map(|height| height + margin.vertical())
            .unwrap_or(0);

        if self.extent.width > available_extent.width
            || self.extent.height > available_extent.height
        {
            warn!(
                "The image doesn't fit to available space.\
                \nThe image size: width={}, height={}.\
                \nAvailable space: width={}, height={}.",
                self.extent.width,
                self.extent.height,
                available_extent.width,
                available_extent.height
            );
            return CompileState::Failure;
        }

        if self.content.is_exists() {
            CompileState::Success
        } else {
            CompileState::Failure
        }
    }
}

impl Draw for Image {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        if !self.content.is_exists() {
            return;
        }

        let offset = Offset::from(&self.margin.unwrap_or_default()) + *offset;
        self.content.draw_with_offset(&offset, drawer)
    }
}

impl DispatchEvent for Image {
    fn dispatch_event(&self, _event: Event) -> Action {
        Action::None
    }
}
