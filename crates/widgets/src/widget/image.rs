use std::{
    io::{Read, Seek, Write},
    path::PathBuf,
};

use linicon::IconPath;
use log::{debug, error, warn};
use shared::{error::ConversionError, file_descriptor::FileDescriptor, value::TryFromValue};

use crate::{
    context::{
        LoadConstraints, LoadExtent, ManageDirtyFlags, ManageIntrinsic, SaveConstraints, SaveExtent,
    },
    drawer::Drawer,
    events::{DispatchContext, DispatchEvent, Event},
    make_configuration,
    state::State,
    types::{
        data::{Configure, WidgetStyle},
        dirty_flags::DirtyFlags,
        extent::Extent,
        identifiers::{WidgetClass, WidgetId, WidgetKey},
        measure::{self, Constraints, Measure, MeasureContext, SizingMode},
        offset::Offset,
        spacing::Spacing,
    },
    widget::{
        Draw, DrawContext, Init, InitContext, Invalidate, InvalidateContext, Layout, LayoutContext,
        WidgetBase,
    },
};

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
    #[builder(private, default)]
    #[gbuilder(hidden, default)]
    id: WidgetId,

    #[builder(setter(strip_option, into), default)]
    #[gbuilder(default)]
    key: Option<WidgetKey>,

    #[builder(default, setter(into))]
    #[gbuilder(default)]
    class: WidgetClass,

    /// The source data for the image being rendered.
    ///
    /// Unlike other widgets, this field is strictly populated via
    /// `WidgetData::Image` during the compilation phase. It holds the
    /// processed pixel data or file path information required to
    /// draw the image to the screen.
    #[builder(private, default = None)]
    #[gbuilder(hidden, default(None))]
    value: Option<ImageData>,

    #[builder(setter(strip_option, into), default)]
    #[gbuilder(hidden, default(None))]
    state: Option<State<ImageProvider>>,

    #[builder(setter(strip_option), default)]
    width: Option<usize>,

    #[builder(setter(strip_option), default)]
    height: Option<usize>,

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

    #[builder(setter(strip_option), default)]
    fit_mode: Option<FitMode>,

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
    pub struct ImageStyle {
        pub max_size: u16,
        pub rounding: u16,
        pub margin: Spacing,
        pub resizing_method: ResizingMethod,
        pub mipmap_mode: MipmapMode,
        pub fit_mode: FitMode,
    } <<= Image
}

impl Image {
    fn load_image(&mut self, provider: &ImageProvider) {
        /// Look's up nearest freedesktop icons.
        fn lookup_freedesktop_icon(icon_name: &str, theme: &str, size: u16) -> Option<IconPath> {
            linicon::lookup_icon(icon_name)
                .from_theme(theme)
                .with_size(size)
                .next()
                .and_then(|icon| icon.ok())
        }

        match provider {
            ImageProvider::ImageInfo(image_data) => {
                self.value = ImageData::from_image_data(image_data)
            }
            ImageProvider::ImagePath(image_path) => self.value = ImageData::from_path(image_path),
            ImageProvider::Icon { name, theme, sizes } => {
                let mut sizes = sizes.clone();
                sizes.sort();
                self.value = sizes
                    .into_iter()
                    .rev()
                    .find_map(|size| {
                        lookup_freedesktop_icon(name, theme, size)
                            .or_else(|| lookup_freedesktop_icon(name, DEFAULT_ICON_THEME, size))
                    })
                    .and_then(|icon_path| ImageData::from_path(&icon_path.path))
            }
            ImageProvider::Unknown => self.value = None,
        };
    }
}

impl WidgetBase for Image {
    fn get_id(&self) -> WidgetId {
        self.id
    }

    fn set_id(&mut self, id: WidgetId) {
        self.id = id;
    }

    fn get_key(&self) -> Option<&WidgetKey> {
        self.key.as_ref()
    }

    fn get_class(&self) -> WidgetClass {
        self.class.clone()
    }

    fn get_type(&self) -> &'static str {
        "image"
    }

    fn sizing_mode(&self) -> SizingMode {
        SizingMode::Dynamic
    }
}

impl<C> Invalidate<C> for Image
where
    C: InvalidateContext,
{
    fn invalidate(&mut self, context: &mut C) -> DirtyFlags {
        let mut dirty_flags = context.get_dirty_flags(self.id);

        if dirty_flags.contains(DirtyFlags::NEEDS_REBUILD) {
            if let Some(image_provider) = self.state.and_then(|state| context.get(state)) {
                self.load_image(image_provider);

                dirty_flags |= DirtyFlags::NEEDS_MEASURE;
            }

            dirty_flags -= DirtyFlags::NEEDS_REBUILD;
        }

        context.set_dirty_flags(self.id, dirty_flags);
        dirty_flags
    }
}

impl<C> Init<C> for Image
where
    C: InitContext,
{
    fn on_init(&mut self, context: &mut C) {
        if let Some(WidgetStyle::Image(image_config)) = context.get_style(&self.class) {
            self.configure(image_config.clone());
        }

        if let Some(state) = self.state {
            context.subscribe(self.id, state);

            if let Some(image_provider) = context.get(state) {
                self.load_image(image_provider);
            }
        }
    }
}

impl Measure<f32, WidgetId> for Image {
    /// Calculates the "natural" size of the widget without external influence.
    ///
    /// Min Intrinsic: Represents the absolute floor for the widget's size.
    ///
    ///  - Logic: Sum of horizontal_spacing and vertical_spacing.
    ///  - Intention: Prevents layout collapse. If an image is loading or missing, the widget still occupies the space defined by its margins/padding to avoid layout shifts.
    ///
    /// Max Intrinsic: Represents the "ideal" unconstrained size.
    ///
    /// - Logic: content.extent + spacing.
    /// - Intention: Tells the parent container (like a Flex) the native resolution of the asset plus its required offsets.
    fn get_intrinsic<C>(&self, context: &mut C) -> measure::Intrinsic<f32>
    where
        C: ManageIntrinsic<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let dirty_flags = context.get_dirty_flags(self.id);
        let cached_intrinsic = context.load(self.id);

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE) && cached_intrinsic.is_some() {
            return cached_intrinsic.unwrap_or_default();
        }

        let inner_spacing = self.margin.unwrap_or_default();
        let spacing_size = Extent::from(inner_spacing);

        let Some(content) = &self.value else {
            return measure::Intrinsic::new(spacing_size, spacing_size);
        };

        let mut extent = Extent::new(0., 0.);
        if let Some(width) = self.width {
            extent.width = width as f32;

            if self.height.is_none() {
                extent.height = extent.width / content.aspect_ratio;
            }
        }

        if let Some(height) = self.height {
            extent.height = height as f32;

            if self.width.is_none() {
                extent.width = extent.height * content.aspect_ratio;
            }
        }

        if self.width.is_none() && self.height.is_none() {
            extent = content.extent;
        }

        let max = extent + spacing_size;
        let intrinsic = measure::Intrinsic::new(spacing_size, max);

        context.save(self.id, intrinsic);
        intrinsic
    }

    ///Determines the final size of the widget based on the parent's Constraints.
    ///
    /// 1. Constraint Adjustment
    ///
    /// Before any calculation, the Constraints must be reduced by the spacing (margins/padding).
    ///
    /// `inner_max = constraints.max - spacing`
    ///
    /// If inner_max dimensions are negative, return spacing (the minimum allowed size).
    ///
    /// 2. Determining Content Size
    ///
    /// The widget resolves its size based on the following priority:
    ///
    /// Case A: Explicit Dimensions (Hard/Partial Fixation)
    /// If width or height (or both) are provided as Option<usize>, they override the content's native extent.
    ///
    /// Both set: The size is fixed to these values.
    /// Only one set: The missing dimension is calculated using content.aspect_ratio.
    ///
    /// Case B: Dynamic Scaling (Soft Fixation)
    /// If no explicit dimensions are set, the widget attempts to fit the image as large as possible within
    /// inner_max while preserving aspect_ratio.
    ///
    /// Assume width = inner_max.width.
    ///
    /// Calculate height = width / aspect_ratio.
    /// If height > inner_max.height:
    /// Set height = inner_max.height.
    /// Set width = height * aspect_ratio.
    ///
    /// 3. Contract Enforcement (Clamping)
    ///
    /// The resulting size from the steps above must be validated against the original inner_constraints.
    ///
    /// Logic: final_content_size = size.clamp(inner_min, inner_max).
    ///
    /// Intention: Ensures the widget never returns a value smaller than constraints.min or larger than constraints.max,
    /// even if the aspect ratio calculation suggests otherwise. This handles "tight" constraints where a parent forces
    /// a specific size.
    fn measure<C>(&self, context: &mut C, constraints: Constraints<Extent<f32>>) -> Extent<f32>
    where
        C: MeasureContext<f32, WidgetId> + ManageDirtyFlags<WidgetId>,
    {
        let mut dirty_flags = context.get_dirty_flags(self.id);
        let constraints_changed =
            Some(constraints) != <C as LoadConstraints<f32, WidgetId>>::load(context, self.id);
        let cached_extent = <C as LoadExtent<f32, WidgetId>>::load(context, self.id);

        if !dirty_flags.contains(DirtyFlags::NEEDS_MEASURE)
            && !constraints_changed
            && cached_extent.is_some()
        {
            return cached_extent.unwrap_or_default();
        }

        let inner_spacing = self.margin.unwrap_or_default();
        let spacing_size = Extent::from(inner_spacing);
        let inner_constraints = constraints.max.shrink_to_with(&inner_spacing);

        if inner_constraints.width <= 0. || inner_constraints.height <= 0. {
            return spacing_size.clamp_with(constraints.min, constraints.max);
        }

        let Some(content) = &self.value else {
            return spacing_size.clamp_with(constraints.min, constraints.max);
        };

        let mut fixed_extent = Extent::new(0., 0.);
        if let Some(width) = self.width {
            fixed_extent.width = width as f32;

            if self.height.is_none() {
                fixed_extent.height = fixed_extent.width / content.aspect_ratio;
            }
        }

        if let Some(height) = self.height {
            fixed_extent.height = height as f32;

            if self.width.is_none() {
                fixed_extent.width = fixed_extent.height * content.aspect_ratio;
            }
        }

        if self.width.is_none() && self.height.is_none() {
            let proportional_height = inner_constraints.width / content.aspect_ratio;

            if proportional_height > inner_constraints.height {
                let proportional_width = inner_constraints.height * content.aspect_ratio;
                fixed_extent = Extent::new(proportional_width, inner_constraints.height);
            } else {
                fixed_extent = Extent::new(inner_constraints.width, proportional_height);
            }
        }

        let used_extent =
            (fixed_extent + spacing_size).clamp_with(constraints.min, constraints.max);

        <C as SaveConstraints<f32, WidgetId>>::save(context, self.id, constraints);
        <C as SaveExtent<f32, WidgetId>>::save(context, self.id, used_extent);

        dirty_flags -= DirtyFlags::NEEDS_MEASURE;
        context.set_dirty_flags(self.id, dirty_flags);

        used_extent
    }
}

impl<C> Layout<C, f32> for Image
where
    C: LayoutContext<f32>,
{
    fn layout(&mut self, context: &C) {
        if context.load(self.id).is_none() && self.value.is_some() {
            warn!(
                "Image widget with id {} didn't measured! The widget may be incorrectly drawn.",
                *self.id
            );
        }
    }
}

impl<C> Draw<C, f32> for Image
where
    C: DrawContext<f32>,
{
    fn draw_on(&self, context: &C, offset: &Offset<f32>, drawer: &mut Drawer) {
        if self.value.is_none() {
            return;
        }

        let Some(provided_extent) = <C as LoadExtent<f32, WidgetId>>::load(context, self.id) else {
            warn!(
                "Image widget with id {} didn't measured! Refused to draw.",
                *self.id
            );
            return;
        };

        let inner_spacing = self.margin.unwrap_or_default();
        let inner_extent = provided_extent.shrink_to_with(&inner_spacing);

        if inner_extent.width <= 0.0 || inner_extent.height <= 0.0 {
            return;
        }

        let Some(ImageData {
            image_file_descriptor,
            extent: image_extent,
            ..
        }) = &self.value
        else {
            return;
        };

        let offset = *offset + self.margin.unwrap_or_default().into();

        let mut file = image_file_descriptor.get_file();
        file.seek(std::io::SeekFrom::Start(0))
            .expect("The temp file should be seekable");

        let mut buffer = vec![];
        if let Err(err) = file.read_to_end(&mut buffer) {
            ImageData::print_readable_fs_error(err, None);
        }

        let data = skia_safe::Data::new_copy(&buffer);
        let image_info = skia_safe::ImageInfo::new(
            (image_extent.width as i32, image_extent.height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        let image =
            skia_safe::images::raster_from_data(&image_info, data, image_extent.width as usize * 4)
                .expect("Image must be valid");

        let scale_x = inner_extent.width / image_extent.width;
        let scale_y = inner_extent.height / image_extent.height;

        let final_extent = match self.fit_mode.as_ref().cloned().unwrap_or_default() {
            FitMode::Contain => {
                let scale = scale_x.min(scale_y);
                image_extent * scale
            }
            FitMode::Cover => {
                let scale = scale_x.max(scale_y);
                image_extent * scale
            }
            FitMode::Fill => inner_extent,
            FitMode::ScaleDown => {
                let scale = scale_x.min(scale_y).min(1.0);
                image_extent * scale
            }
        };

        let actual_offset = offset
            + Offset::new(
                (inner_extent.width - final_extent.width) / 2.0,
                (inner_extent.height - final_extent.height) / 2.0,
            )
            + inner_spacing.into();

        let src_rect =
            skia_safe::Rect::from_xywh(0., 0., image.width() as f32, image.height() as f32);

        let dst_rect = skia_safe::Rect::from_xywh(
            actual_offset.x,
            actual_offset.y,
            final_extent.width,
            final_extent.height,
        );

        let corner_radius = (final_extent.width.min(final_extent.height) / 2.0)
            .min(self.rounding.unwrap_or_default() as f32);
        let rrect = skia_safe::RRect::new_rect_xy(dst_rect, corner_radius, corner_radius);

        let canvas = drawer.surface.canvas();

        canvas.save();
        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);

        let sampling = skia_safe::SamplingOptions::new(
            self.resizing_method
                .as_ref()
                .cloned()
                .unwrap_or_default()
                .to_skia_value(),
            self.mipmap_mode
                .as_ref()
                .cloned()
                .unwrap_or_default()
                .to_skia_value(),
        );

        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);

        canvas.draw_image_rect_with_sampling_options(
            image,
            Some((&src_rect, skia_safe::canvas::SrcRectConstraint::Fast)),
            dst_rect,
            sampling,
            &paint,
        );

        canvas.restore();
    }
}

impl<C> DispatchEvent<C, f32> for Image
where
    C: DispatchContext<f32>,
{
    fn dispatch_event(&mut self, _context: &mut C, _event: Event) {}
}

#[derive(Debug, Clone)]
pub enum ImageProvider {
    /// Raw byte data for an image.
    ImageInfo(ImageInfo),
    /// A filesystem path to an image file.
    ImagePath(PathBuf),
    /// Instructions for looking up a system icon.
    Icon {
        name: String,
        theme: String,
        sizes: Vec<u16>,
    },
    Unknown,
}

/// Represents a GPU-ready image used by [`Image`] widget during rendering.
///
/// The `ImageData` type acts as a container for image data and its rendering
/// configuration. Rather than immediately rasterizing or resizing images,
/// it stores their file descriptor and size information, leaving the
/// final drawing and scaling work to the GPU.
///
/// This allows efficient rendering and flexible resizing while avoiding
/// unnecessary CPU work.  
///
/// The [`Image::Unknown`] variant indicates that the image could
/// not be loaded or was invalid (e.g. corrupt raw data or unsupported format),
/// allowing the caller to gracefully skip rendering instead of crashing.
#[derive(Debug, Clone)]
pub struct ImageData {
    image_file_descriptor: FileDescriptor,
    extent: Extent<f32>,

    /// Width / Height ratio
    aspect_ratio: f32,
}

impl ImageData {
    pub fn from_image_data(image_info: &ImageInfo) -> Option<Self> {
        let origin_width = image_info.width as u32;
        let origin_height = image_info.height as u32;

        let extent = Extent::new(image_info.width as f32, image_info.height as f32);
        let aspect_ratio = extent.width / extent.height;

        if image_info.has_alpha {
            return Some(ImageData {
                image_file_descriptor: image_info.image_file_descriptor.clone(),
                extent,
                aspect_ratio,
            });
        }

        let rgb_image = {
            let mut image_buffer = vec![];
            let mut file = image_info.image_file_descriptor.get_file();
            file.seek(std::io::SeekFrom::Start(0))
                .expect("The file must be able to seek");
            file.read_to_end(&mut image_buffer)
                .expect("The file must be able to read");
            image_buffer
        };

        let mut rgba_image = vec![255; origin_width as usize * origin_height as usize * 4];
        for (i, chunk) in rgb_image.chunks_exact(3).enumerate() {
            let offset = i * 4;
            rgba_image[offset] = chunk[0];
            rgba_image[offset + 1] = chunk[1];
            rgba_image[offset + 2] = chunk[2];
        }

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        if let Err(err) = file.write_all(&rgba_image) {
            Self::print_readable_fs_error(err, None);
            return None;
        }

        debug!("Image: Created from 'image_data'");

        Some(ImageData {
            image_file_descriptor: file.into(),
            extent,
            aspect_ratio,
        })
    }

    /// Creates an [`Image`] from an image file at the given path.
    ///
    /// This method supports any file format handled by `image-rs`
    /// (PNG, JPEG, WEBP, etc.) and will load, decode, and store the
    /// result as a temporary file descriptor for GPU-backed rendering.
    ///
    /// Like [`Self::from_image_data`], it respects the
    /// size restrictions and scaling rules provided in [`ImageConfiguration`].
    ///
    /// Returns [`Image::Unknown`] if the file is missing, unreadable,
    /// or cannot be decoded.
    pub fn from_path(image_path: &std::path::Path) -> Option<ImageData> {
        let data = match std::fs::read(image_path) {
            Ok(data) => data,
            Err(err) => {
                Self::print_readable_fs_error(err, image_path);
                return None;
            }
        };

        let format = match image::guess_format(&data) {
            Ok(format) => format,
            Err(err) => {
                warn!(
                    "Cannot guess the format of image at {image_path}. \
                    Error: {err}. Maybe it's SVG, trying to parse.",
                    image_path = image_path.display()
                );
                return Self::from_svg(image_path);
            }
        };

        let image = match image::load_from_memory_with_format(&data, format) {
            Ok(image) => image,
            Err(err) => {
                error!(
                    "Cannot load the image at {image_path}. Error: {err}",
                    image_path = image_path.display()
                );
                return None;
            }
        };

        let extent = Extent::new(image.width() as f32, image.height() as f32);
        let aspect_ratio = extent.width / extent.height;

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        if let Err(err) = file.write_all(&image.to_rgba8().into_vec()) {
            Self::print_readable_fs_error(err, image_path);
        }

        Some(ImageData {
            image_file_descriptor: file.into(),
            extent,
            aspect_ratio,
        })
    }

    /// Creates an [`Image`] from an SVG file.
    ///
    /// Unlike [`Self::from_path`], this method rasterizes
    /// the SVG immediately into a bitmap image before storing it
    /// as a file descriptor.  
    ///
    /// This ensures the resulting image is GPU-friendly while preserving
    /// correct vector scaling and aspect ratio within the given
    /// [`Extent2D`] constraints.
    pub fn from_svg(image_path: &std::path::Path) -> Option<Self> {
        if !image_path.is_file() {
            return None;
        }

        let data = match std::fs::read(image_path) {
            Ok(data) => data,
            Err(err) => {
                Self::print_readable_fs_error(err, image_path);
                return None;
            }
        };

        let tree = match resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()) {
            Ok(tree) => tree,
            Err(err) => {
                let image_path = image_path.display();
                match err {
                    resvg::usvg::Error::MalformedGZip => {
                        warn!("Malformed gzip format of SVG image in path: {image_path}")
                    }
                    resvg::usvg::Error::NotAnUtf8Str => {
                        warn!("The SVG image file contains non-UTF-8 string in path: {image_path}")
                    }
                    _ => warn!("Something wrong with SVG image in path: {image_path}"),
                }
                return None;
            }
        };

        let Some(mut pixmap) =
            resvg::tiny_skia::Pixmap::new(tree.size().width() as u32, tree.size().height() as u32)
        else {
            warn!("The SVG Image width or height is equal to zero!");
            return None;
        };

        resvg::render(
            &tree,
            resvg::usvg::Transform::identity(),
            // resvg::usvg::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

        debug!(
            "Image: Created image from svg by path {image_path}",
            image_path = image_path.display()
        );

        let mut file = tempfile::tempfile().expect("The temp file must be created");
        if let Err(err) = file.write_all(pixmap.data()) {
            Self::print_readable_fs_error(err, image_path);
            return None;
        }

        let extent = Extent::new(tree.size().width(), tree.size().height());
        let aspect_ratio = extent.width / extent.height;
        Some(ImageData {
            image_file_descriptor: file.into(),
            extent,
            aspect_ratio,
        })
    }

    /// Prints a human-readable description of a file-system error
    /// that occurred while working with the image.
    ///
    /// This is used when reading, writing, or opening the temporary
    /// file descriptor fails, making it easier to debug I/O issues.
    ///
    /// It is intended for logging and diagnostics, not for user-facing
    /// error messages.
    fn print_readable_fs_error<'a, I>(error: std::io::Error, image_path: I)
    where
        I: Into<Option<&'a std::path::Path>>,
    {
        let image_path = image_path
            .into()
            .and_then(std::path::Path::to_str)
            .unwrap_or("Hidden path");

        match error.kind() {
            std::io::ErrorKind::NotFound => {
                warn!("Not found SVG image in path: {image_path}")
            }
            std::io::ErrorKind::PermissionDenied => {
                warn!("Permission to read SVG image in path is denied: {image_path}")
            }
            _ => warn!("Something wrong happened during reading SVG image in path: {image_path}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: i32,
    pub height: i32,
    pub has_alpha: bool,

    /// File descriptor containing the image data in RGB byte order.
    ///
    /// To avoid storing large data in RAM, the image is stored in a temporary file.
    /// The file is automatically removed when no handles reference it.
    pub image_file_descriptor: FileDescriptor,
}

#[derive(Debug, Default, Clone)]
pub enum FitMode {
    /// Fit all, leave empty spaces
    #[default]
    Contain,

    /// Crop and show only top-left image or center
    Cover,

    /// Break aspect ratio and fill fully in rectangular space
    Fill,

    /// Like [`FitMode::Contain`], but only when bigger than available space
    ScaleDown,
}

impl TryFromValue for FitMode {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "contain" => FitMode::Contain,
            "cover" => FitMode::Cover,
            "fill" => FitMode::Fill,
            "scale-down" => FitMode::ScaleDown,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "contain, cover, fill or scale-down",
                actual: value,
            })?,
        })
    }
}

/// The algorithm used to calculate new pixels when an image is scaled.
///
/// This determines the visual quality of the image when it is made
/// larger or smaller than its original size.
#[derive(Debug, Default, Clone)]
pub enum ResizingMethod {
    /// Faster but "blocky" scaling. Picks the closest pixel color
    /// without blending. Best for pixel art.
    Nearest,
    /// Smoother scaling. Blends neighboring pixels together to
    /// prevent jagged edges. Standard for photos and icons.
    #[default]
    Linear,
}

impl TryFromValue for ResizingMethod {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "nearest" => ResizingMethod::Nearest,
            "linear" => ResizingMethod::Linear,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "nearest or linear",
                actual: value,
            })?,
        })
    }
}

/// The technique used for choosing between different pre-scaled
/// versions of an image.
///
/// Mipmapping helps prevent "shimmering" or visual noise when an
/// image is shrunk significantly. It tells Skia how to sample
/// from pre-calculated lower-resolution versions of the image.
#[derive(Debug, Default, Clone)]
pub enum MipmapMode {
    /// No pre-scaled versions are used; the image is scaled directly
    /// from the original.
    None,
    /// Picks the single best-fitting pre-scaled version.
    Nearest,
    /// Blends between the two best-fitting pre-scaled versions for
    /// the smoothest possible transition.
    #[default]
    Linear,
}

impl TryFromValue for MipmapMode {
    fn try_from_string(value: String) -> Result<Self, ConversionError> {
        Ok(match value.to_lowercase().as_str() {
            "none" => MipmapMode::None,
            "nearest" => MipmapMode::Nearest,
            "linear" => MipmapMode::Linear,
            _ => Err(shared::error::ConversionError::InvalidValue {
                expected: "none, nearest or linear",
                actual: value,
            })?,
        })
    }
}

/// A helper trait to convert application-specific configuration
/// values into their Skia equivalents.
///
/// This trait is implemented for types like filter mode or mipmap
/// mode that exist in user configuration but must be translated
/// into `skia_safe` enums before rendering.
///
/// By using this trait, `Image` and other drawing code remain
/// decoupled from configuration details, making the conversion
/// process uniform and easy to extend.
trait ToSkiaValue<T> {
    fn to_skia_value(&self) -> T;
}

impl ToSkiaValue<skia_safe::FilterMode> for ResizingMethod {
    fn to_skia_value(&self) -> skia_safe::FilterMode {
        match self {
            ResizingMethod::Nearest => skia_safe::FilterMode::Nearest,
            ResizingMethod::Linear => skia_safe::FilterMode::Linear,
        }
    }
}

impl ToSkiaValue<skia_safe::MipmapMode> for MipmapMode {
    fn to_skia_value(&self) -> skia_safe::MipmapMode {
        match self {
            MipmapMode::None => skia_safe::MipmapMode::None,
            MipmapMode::Nearest => skia_safe::MipmapMode::Nearest,
            MipmapMode::Linear => skia_safe::MipmapMode::Linear,
        }
    }
}
