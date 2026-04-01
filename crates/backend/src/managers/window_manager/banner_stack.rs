use config::{
    display::{AnimationDefinition, AnimationStyle, DisplayConfig},
    Config,
};
use dbus::{
    actions::ClosingReason,
    notification::{self, Notification},
};
use indexmap::{
    indexmap,
    map::{Iter, Values, ValuesMut},
    IndexMap,
};
use log::{debug, trace, warn};
use shared::cached_data::CachedData;
use skia_safe::textlayout::FontCollection;
use std::{
    cmp::Ordering,
    collections::VecDeque,
    hash::Hash,
    path::PathBuf,
    time::{self, Duration},
};
use widgets::{
    self,
    animation::{Animated, AnimatedWidget},
    drawer::Drawer,
    types::{constraints::Constraints, Alignment, Border, Extent2D, Offset, Position},
    widget::{FlexContainerBuilder, ImageBuilder, TextBuilder},
    Compile, CompileCtx, Draw, Widget, WidgetInfo,
};

use super::CachedLayout;

/// The container of banners which allows manage them easily.
pub(super) struct BannerStack<K>
where
    K: Hash + Eq,
{
    banners: IndexMap<K, Banner>,
}

impl<K> BannerStack<K>
where
    K: Hash + Eq,
{
    pub(super) fn new() -> Self {
        Self {
            banners: indexmap! {},
        }
    }

    pub(super) fn len(&self) -> usize {
        self.banners.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.banners.is_empty()
    }

    pub(super) fn get_mut(&mut self, key: &K) -> Option<&mut Banner> {
        self.banners.get_mut(key)
    }

    pub(super) fn width(&self) -> usize {
        self.banners
            .first()
            .map(|(_, banner)| banner.width())
            .unwrap_or(0)
    }

    /// Sum of banner heights with gaps between them.
    pub(super) fn total_height_with_gap(&self, gap: usize) -> usize {
        self.total_height() + self.len().saturating_sub(1) * gap
    }

    /// Sum of banner heights.
    pub(super) fn total_height(&self) -> usize {
        self.banners.values().map(|banner| banner.height()).sum()
    }

    /// Updates the banner stack and banners by newly updated user configuration.
    pub(super) fn configure(&mut self, config: &Config) {
        self.sort_by_config(config);
        self.banners_mut()
            .for_each(|banner| banner.is_compiled = false);
    }

    fn sort_by_config(&mut self, config: &Config) {
        self.banners
            .sort_by_values(config.general().sorting.get_cmp());
    }

    /// Iterator over references of banners.
    pub(super) fn banners<'a>(&'a self) -> Values<'a, K, Banner> {
        self.banners.values()
    }

    /// Iterator over mutable references of banners.
    pub(super) fn banners_mut<'a>(&'a mut self) -> ValuesMut<'a, K, Banner> {
        self.banners.values_mut()
    }
}

impl BannerStack<u32> {
    /// Removes already closed banners.
    pub(super) fn remove_closed(&mut self) -> Vec<(Notification, ClosingReason)> {
        self.banners
            .drain_filter(|(_, banner)| banner.is_closed() && banner.is_finished())
            .into_iter()
            .map(|(_, banner)| {
                (banner.notification, unsafe {
                    banner.close_status.into_reason().unwrap_unchecked()
                })
            })
            .collect()
    }

    /// Takes from input [VecDeque] and replaces existing notifications.
    pub(super) fn replace_by_keys(
        &mut self,
        notifications: &mut VecDeque<Notification>,
        config: &Config,
    ) {
        let notifications_to_replace =
            notifications.drain_filter(|notification| self.banners.get(&notification.id).is_some());

        for notification in notifications_to_replace {
            let id = notification.id;
            self.banners[&id].update_data(notification);
        }

        self.sort_by_config(config);
    }

    /// Extends current container with new banners that will be created from notification. Note
    /// that existing banner with the same notification id will be replaced.
    pub(super) fn extend_from<I>(&mut self, notifications: I, config: &Config)
    where
        I: Iterator<Item = Notification>,
    {
        for notification in notifications {
            self.banners.insert(notification.id, notification.into());
        }
        self.sort_by_config(config);
    }
}

impl<K> Default for BannerStack<K>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K> std::ops::Index<&K> for BannerStack<K>
where
    K: Hash + Eq,
{
    type Output = Banner;
    fn index(&self, index: &K) -> &Self::Output {
        &self.banners[index]
    }
}

impl<K> std::ops::IndexMut<&K> for BannerStack<K>
where
    K: Hash + Eq,
{
    fn index_mut(&mut self, index: &K) -> &mut Self::Output {
        &mut self.banners[index]
    }
}

trait SortByValues<K, V> {
    fn sort_by_values(&mut self, cmp: for<'a> fn(&'a V, &'a V) -> Ordering);
}

impl<K, V> SortByValues<K, V> for IndexMap<K, V> {
    fn sort_by_values(&mut self, cmp: for<'a> fn(&'a V, &'a V) -> Ordering) {
        self.sort_by(|_, lhs, _, rhs| cmp(lhs, rhs));
    }
}

trait DrainFilter<F, T> {
    fn drain_filter(&mut self, filter: F) -> Vec<T>;
}

impl<F, T> DrainFilter<F, T> for VecDeque<T>
where
    F: Fn(&T) -> bool,
{
    fn drain_filter(&mut self, filter: F) -> Vec<T> {
        let mut removed = Vec::new();
        let mut i = 0;
        while i < self.len() {
            if filter(&self[i]) {
                removed.push(self.remove(i).unwrap());
            } else {
                i += 1;
            }
        }
        removed
    }
}

impl<F, K, V> DrainFilter<F, (K, V)> for IndexMap<K, V>
where
    F: Fn((&K, &V)) -> bool,
{
    fn drain_filter(&mut self, filter: F) -> Vec<(K, V)> {
        let mut removed = Vec::new();
        let mut i = 0;
        while i < self.len() {
            if filter(self.get_index(i).unwrap()) {
                removed.push(self.shift_remove_index(i).unwrap());
            } else {
                i += 1;
            }
        }
        removed
    }
}

impl<'a, K> IntoIterator for &'a BannerStack<K>
where
    K: Hash + Eq,
{
    type Item = (&'a K, &'a Banner);
    type IntoIter = Iter<'a, K, Banner>;

    fn into_iter(self) -> Self::IntoIter {
        self.banners.iter()
    }
}

/// Represents a notification banner.
pub(super) struct Banner {
    notification: Notification,
    base_layout: Option<Widget>,
    stage: Option<BannerStage>,
    close_status: CloseStatus,

    /// A widget layout only needs to be compiled once, until the data or layout changes.
    is_compiled: bool,
}

impl Banner {
    const NOTIFICATION_FRAME: &str = "notification_frame";
    const NOTIFICATION_SUMMARY: &str = "notification_summary";
    const NOTIFICATION_BODY: &str = "notification_body";
    const NOTIFICATION_IMAGE: &str = "notification_image";

    pub(super) fn new(notification: Notification) -> Self {
        debug!("Banner (id={}): Created", notification.id);

        Self {
            notification,
            base_layout: None,
            stage: None,
            close_status: CloseStatus::NotClosed,

            is_compiled: false,
        }
    }

    pub(super) fn notification(&self) -> &Notification {
        &self.notification
    }

    pub(super) fn close(&mut self, closing_reason: ClosingReason) {
        self.close_status.close_with(closing_reason);
    }

    pub(super) fn is_closed(&self) -> bool {
        self.close_status.is_closed()
    }

    pub(super) fn is_interactable(&self) -> bool {
        !self.is_closed() && self.stage.as_ref().is_some_and(BannerStage::is_showing)
    }

    pub(super) fn reset_timeout(&mut self) {
        if let Some(stage) = self.stage.as_mut() {
            stage.reset_timeout();
        }

        trace!("Banner (id={}): Timeout reset", self.notification.id);
    }

    pub(super) fn update_data(&mut self, notification: Notification) {
        self.notification = notification;
        self.is_compiled = false;
        self.reset_timeout();
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.notification.id
        );
    }

    // TODO: use it for resize
    pub(super) fn width(&self) -> usize {
        self.stage
            .as_ref()
            .map(|stage| stage.width())
            .unwrap_or_default()
    }

    pub(super) fn height(&self) -> usize {
        self.stage
            .as_ref()
            .map(|stage| stage.height())
            .unwrap_or_default()
    }

    pub(super) fn try_next_stage(&mut self, config: &Config) -> bool {
        let is_unfinished_or_last_or_unskipable = |stage: &BannerStage| {
            !(stage.is_current_finished(&self.notification, config)
                || (self.is_closed() && stage.is_skipable()))
                || stage.is_totally_finished()
        };

        if self
            .stage
            .as_ref()
            .is_none_or(is_unfinished_or_last_or_unskipable)
        {
            return false;
        }

        let Some(stage) = self.stage.take() else {
            return false;
        };

        self.stage = stage.next(&self.notification, config);

        true
    }

    /// Compiles the widget layout of notification banner.
    ///
    /// Compiling the widget layout is important to ensure correct positioning of the UI elements,
    /// proper text alignment, and overall layout consistency. Without compilation, the notification
    /// banner may render incorrectly or not appear at all.
    pub(super) fn compile(
        &mut self,
        config: &Config,
        font_collection: skia_safe::textlayout::FontCollection,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) {
        if self.is_compiled {
            return;
        }

        let extent = Extent2D::new(
            config.general().width as usize,
            config.general().height as usize,
        );

        let display = config.display_by_app(&self.notification.app_name);

        let mut layout = match &display.layout {
            config::display::Layout::Default => default_layout(),
            config::display::Layout::FromPath { path_buf } => cached_layouts
                .get(path_buf)
                .and_then(CachedLayout::layout)
                .cloned()
                .unwrap_or_else(default_layout),
        };

        layout.compile(
            Constraints::from_extent_soft(extent.into()),
            &mut make_compile_context(&self.notification, config, font_collection),
        );

        self.is_compiled = true;
        self.base_layout = Some(layout.clone());

        match self.stage.as_mut() {
            Some(stage) => stage.replace_widget(layout),
            None => self.stage = Some(BannerStage::start(layout, &self.notification, config)),
        }
    }

    /// Draws the notification banner frame into provided surface with offset.
    pub(super) fn draw(
        &self,
        offset: &Offset<usize>,
        sk_surface: &mut skia_safe::Surface,
    ) -> DrawState {
        debug!("Banner (id={}): Beginning of draw", self.notification.id);

        let mut drawer = Drawer::use_surface(sk_surface.clone());
        let Some(stage) = &self.stage else {
            return DrawState::Failure;
        };

        stage.draw_with_offset(offset, &mut drawer);

        debug!("Banner (id={}): Complete draw", self.notification.id);
        DrawState::Success
    }
}

/// Returns the default widget layout if a custom layout has not been defined.
fn default_layout() -> Widget {
    FlexContainerBuilder::default()
        .id(Banner::NOTIFICATION_FRAME)
        .direction(widgets::types::Direction::Horizontal)
        .alignment(Alignment::new(Position::Start, Position::Center))
        .children(vec![
            ImageBuilder::default()
                .id(Banner::NOTIFICATION_IMAGE)
                .build()
                .unwrap()
                .into(),
            FlexContainerBuilder::default()
                .spacing(Default::default())
                .border(Border::default())
                .direction(widgets::types::direction::Direction::Vertical)
                .alignment(Alignment::new(Position::Center, Position::Center))
                .children(vec![
                    TextBuilder::default()
                        .id(Banner::NOTIFICATION_SUMMARY)
                        .build()
                        .unwrap()
                        .into(),
                    TextBuilder::default()
                        .id(Banner::NOTIFICATION_BODY)
                        .build()
                        .unwrap()
                        .into(),
                ])
                .build()
                .unwrap()
                .into(),
        ])
        .build()
        .unwrap()
        .into()
}

fn make_compile_context(
    notification: &Notification,
    config: &Config,
    font_collection: FontCollection,
) -> CompileCtx {
    macro_rules! make_text_config {
        (for $kind:ident use $compile_ctx:ident, $display_config:ident, $colors:ident, $id:ident) => {
            $compile_ctx.assign_config(
                Banner::$id,
                widgets::types::WidgetConfig::Text(widgets::widget::TextConfiguration {
                    font: widgets::widget::Font {
                        name: $display_config.$kind.font.name.clone(),
                        size: $display_config.$kind.font_size as usize,
                        style: $display_config.$kind.style.clone().into(),
                    },
                    wrap: $display_config.$kind.wrap,
                    margin: $display_config.$kind.margin.into(),
                    alignment: $display_config.$kind.alignment.clone().into(),
                    line_spacing: $display_config.$kind.line_spacing as usize,
                    color: $colors.foreground.clone().into(),
                }),
            );
        };
    }

    let mut compile_ctx = CompileCtx::new(font_collection);

    let display_config = config.display_by_app(&notification.app_name);
    let theme = config.theme_by_app(&notification.app_name);
    let colors = theme.by_urgency(&notification.hints.urgency);

    compile_ctx.assign_config(
        Banner::NOTIFICATION_FRAME,
        widgets::types::WidgetConfig::Container(widgets::widget::ContainerConfiguration {
            background_color: colors.background.clone().into(),
            border: Border {
                size: display_config.border.size as usize,
                radius: display_config.border.radius as usize,
                color: colors.border.clone().into(),
            },
            spacing: display_config.padding.into(),
            alignment: Alignment::new(Position::Start, Position::Center),
        }),
    );

    compile_ctx.assign_config(
        Banner::NOTIFICATION_IMAGE,
        widgets::types::WidgetConfig::Image(display_config.image.clone().into()),
    );

    if let Some(image_data) = try_get_image_data(notification, display_config) {
        compile_ctx.assign_data(Banner::NOTIFICATION_IMAGE, image_data);
    }

    make_text_config!(for summary use compile_ctx, display_config, colors, NOTIFICATION_SUMMARY);
    compile_ctx.assign_data(
        Banner::NOTIFICATION_SUMMARY,
        widgets::types::WidgetData::Text(notification.summary.clone()),
    );

    make_text_config!(for body use compile_ctx, display_config, colors, NOTIFICATION_BODY);
    compile_ctx.assign_data(
        Banner::NOTIFICATION_BODY,
        widgets::types::WidgetData::Text(notification.body.clone()),
    );

    compile_ctx
}

fn try_get_image_data(
    notification: &Notification,
    display_config: &DisplayConfig,
) -> Option<widgets::types::WidgetData> {
    notification
        .hints
        .image_data
        .as_ref()
        .cloned()
        .map(|image_data| {
            widgets::types::WidgetData::ImageData(widgets::image::ImageData {
                width: image_data.width,
                height: image_data.height,
                has_alpha: image_data.has_alpha,
                image_file_descriptor: image_data.image_file_descriptor.clone(),
            })
        })
        .or_else(|| {
            notification
                .hints
                .image_path
                .as_deref()
                .map(std::path::PathBuf::from)
                .map(widgets::types::WidgetData::ImagePath)
        })
        .or_else(|| {
            if notification.app_icon.is_empty() {
                return None;
            }

            Some(widgets::types::WidgetData::Icon {
                name: notification.app_icon.clone(),
                theme: display_config.icons.theme.clone(),
                sizes: display_config.icons.size.clone(),
            })
        })
}

impl Draw for Banner {
    // TODO: add `Result` type for these methods to handle possible errors
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        let Some(layout) = &self.base_layout else {
            return;
        };

        layout.draw_with_offset(offset, drawer);
    }
}

impl Animated for Banner {
    fn is_finished(&self) -> bool {
        self.stage
            .as_ref()
            .is_none_or(|stage| stage.is_totally_finished())
    }

    fn update(&mut self, delta_time_ns: u64) {
        if let Some(stage) = self.stage.as_mut() {
            stage.update(delta_time_ns);

            if stage.is_totally_finished() {
                self.close_status.close_with(ClosingReason::Expired);
            }
        }
    }
}

impl From<Notification> for Banner {
    fn from(value: Notification) -> Self {
        Self::new(value)
    }
}

impl<'a> From<&'a Banner> for &'a Notification {
    fn from(value: &'a Banner) -> Self {
        &value.notification
    }
}

pub(super) enum DrawState {
    Success,
    Failure,
}

// TODO: Add missing 'Allocation' and 'Free' variants
enum BannerStage {
    Allocation(Spacer<Increasing>),
    Appearing(AnimatedWidget),
    Showing {
        widget: Widget,
        created_at: time::Instant,
        timeout: notification::Timeout,
    },
    Disappearing(AnimatedWidget),
    Free(Spacer<Decreasing>),
}

impl BannerStage {
    fn start(widget: Widget, notification: &Notification, config: &Config) -> Self {
        // TODO: some animations may require additional information about position or something
        // else. For instance, the slide animation which requires to have start and end positions.
        let max_height = widget.height();
        BannerStage::Allocation(Spacer::<Increasing>::new(
            widget,
            0,
            max_height,
            config
                .display_by_app(&notification.app_name)
                .animation
                .allocation
                .duration
                .clone()
                .into(),
        ))
    }

    fn next(self, notification: &Notification, config: &Config) -> Option<Self> {
        match self {
            BannerStage::Allocation(Spacer { widget, .. }) => {
                Some(BannerStage::Appearing(make_animated_widget(
                    widget,
                    config,
                    &config
                        .display_by_app(&notification.app_name)
                        .animation
                        .enter,
                )))
            }
            BannerStage::Appearing(animated_widget) => Some(BannerStage::Showing {
                widget: animated_widget.into_widget(),
                created_at: time::Instant::now(),
                timeout: notification.expire_timeout.clone(),
            }),
            BannerStage::Showing { widget, .. } => {
                Some(BannerStage::Disappearing(make_animated_widget(
                    widget,
                    config,
                    &config.display_by_app(&notification.app_name).animation.exit,
                )))
            }
            BannerStage::Disappearing(animated_widget) => {
                let max_height = animated_widget.as_widget().height();
                Some(BannerStage::Free(Spacer::<Decreasing>::new(
                    animated_widget.into_widget(),
                    0,
                    max_height,
                    config
                        .display_by_app(&notification.app_name)
                        .animation
                        .free
                        .duration
                        .clone()
                        .into(),
                )))
            }
            BannerStage::Free(_) => None,
        }
    }

    fn replace_widget(&mut self, new_widget: Widget) {
        match self {
            BannerStage::Allocation(spacer) => spacer.widget = new_widget,
            BannerStage::Appearing(animated_widget) => animated_widget.replace_widget(new_widget),
            BannerStage::Showing { widget, .. } => *widget = new_widget,
            BannerStage::Disappearing(animated_widget) => {
                animated_widget.replace_widget(new_widget)
            }
            BannerStage::Free(spacer) => spacer.widget = new_widget,
        }
    }

    fn update(&mut self, delta_time_ns: u64) {
        match self {
            BannerStage::Allocation(spacer) => spacer.update(delta_time_ns),
            BannerStage::Appearing(animated_widget) => animated_widget.update(delta_time_ns),
            BannerStage::Showing { .. } => (),
            BannerStage::Disappearing(animated_widget) => animated_widget.update(delta_time_ns),
            BannerStage::Free(spacer) => spacer.update(delta_time_ns),
        }
    }

    fn reset_timeout(&mut self) {
        if let BannerStage::Showing {
            ref mut created_at, ..
        } = self
        {
            *created_at = time::Instant::now();
        }
    }

    fn is_showing(&self) -> bool {
        matches!(self, BannerStage::Showing { .. })
    }

    fn is_skipable(&self) -> bool {
        matches!(self, BannerStage::Showing { .. })
    }

    fn is_current_finished(&self, notification: &Notification, config: &Config) -> bool {
        match self {
            BannerStage::Allocation(spacer) => spacer.is_finished(),
            BannerStage::Appearing(animated_widget) => animated_widget.is_finished(),
            BannerStage::Showing {
                created_at,
                timeout,
                ..
            } => match timeout {
                notification::Timeout::Millis(millis) => {
                    created_at.elapsed().as_millis() > *millis as u128
                }
                notification::Timeout::Configurable => {
                    let timeout = config
                        .display_by_app(&notification.app_name)
                        .timeout
                        .by_urgency(&notification.hints.urgency);
                    timeout != 0 && created_at.elapsed().as_millis() > timeout as u128
                }
                notification::Timeout::Never => false,
            },
            BannerStage::Disappearing(animated_widget) => animated_widget.is_finished(),
            BannerStage::Free(spacer) => spacer.is_finished(),
        }
    }

    fn is_totally_finished(&self) -> bool {
        if let BannerStage::Free(spacer) = self {
            return spacer.is_finished();
        }

        false
    }

    fn width(&self) -> usize {
        match self {
            BannerStage::Allocation(spacer) => spacer.widget.width(),
            BannerStage::Appearing(animated_widget) => animated_widget.as_widget().width(),
            BannerStage::Showing { widget, .. } => widget.width(),
            BannerStage::Disappearing(animated_widget) => animated_widget.as_widget().width(),
            BannerStage::Free(spacer) => spacer.widget.width(),
        }
    }

    fn height(&self) -> usize {
        match self {
            BannerStage::Allocation(spacer) => spacer.current_height,
            BannerStage::Appearing(animated_widget) => animated_widget.as_widget().height(),
            BannerStage::Showing { widget, .. } => widget.height(),
            BannerStage::Disappearing(animated_widget) => animated_widget.as_widget().height(),
            BannerStage::Free(spacer) => spacer.current_height,
        }
    }
}

impl Draw for BannerStage {
    fn draw_with_offset(&self, offset: &Offset<usize>, drawer: &mut Drawer) {
        match self {
            BannerStage::Appearing(animated_widget) => {
                animated_widget.draw_with_offset(offset, drawer)
            }
            BannerStage::Showing { widget, .. } => widget.draw_with_offset(offset, drawer),
            BannerStage::Disappearing(animated_widget) => {
                animated_widget.draw_with_offset(offset, drawer)
            }
            BannerStage::Allocation(_) | BannerStage::Free(_) => (),
        }
    }
}

#[derive(Default)]
enum CloseStatus {
    #[default]
    NotClosed,
    Closed(ClosingReason),
}

impl CloseStatus {
    fn is_closed(&self) -> bool {
        matches!(self, CloseStatus::Closed(_))
    }

    fn close_with(&mut self, closing_reason: ClosingReason) {
        match self {
            CloseStatus::NotClosed | CloseStatus::Closed(_) => {
                *self = CloseStatus::Closed(closing_reason)
            }
        }
    }

    fn into_reason(self) -> Option<ClosingReason> {
        match self {
            CloseStatus::NotClosed => None,
            CloseStatus::Closed(reason) => Some(reason),
        }
    }
}

fn make_animated_widget(
    widget: Widget,
    config: &Config,
    animation_definition: &AnimationDefinition,
) -> AnimatedWidget {
    let duration: Duration = animation_definition.duration.clone().into();
    let easing_type = animation_definition.easing.clone();

    // TODO: better to use actual window size instead of fixed value when the dynamic adaptation by
    // width (by filled content).
    let (start_x, end_x) = if config.general().anchor.is_left() {
        (-500.0, 0.0)
    } else if config.general().anchor.is_right() {
        (500.0, 0.0)
    } else {
        warn!("Selected slide in/out animation for middle window which won't look normally!");

        (500.0, 0.0)
    };

    match animation_definition.style {
        AnimationStyle::FadeIn => widget.fade_in(duration, easing_type).into(),
        AnimationStyle::FadeOut => widget.fade_out(duration, easing_type).into(),
        AnimationStyle::PopIn => widget.pop_in(duration, easing_type).into(),
        AnimationStyle::PopOut => widget.pop_out(duration, easing_type).into(),
        AnimationStyle::SlideIn => widget
            .slide_horizontally(start_x, end_x, duration, easing_type)
            .into(),
        AnimationStyle::SlideOut => widget
            .slide_horizontally(end_x, start_x, duration, easing_type)
            .into(),
    }
}

trait ChangeDirection {
    fn get_current_value(progress: f32, min: usize, max: usize) -> usize;
}

struct Increasing;
struct Decreasing;

impl ChangeDirection for Increasing {
    fn get_current_value(progress: f32, min: usize, max: usize) -> usize {
        (progress * (max - min) as f32).round() as usize
    }
}

impl ChangeDirection for Decreasing {
    fn get_current_value(progress: f32, min: usize, max: usize) -> usize {
        ((1.0 - progress) * (max - min) as f32).round() as usize
    }
}

struct Spacer<Direction: ChangeDirection> {
    widget: Widget,

    min_height: usize,
    max_height: usize,
    current_height: usize,

    elapsed_ns: u64,
    duration: Duration,

    _marker: std::marker::PhantomData<Direction>,
}

impl Spacer<Increasing> {
    fn new(widget: Widget, min_height: usize, max_height: usize, duration: Duration) -> Self {
        Self {
            widget,
            min_height,
            max_height,
            current_height: min_height,
            elapsed_ns: 0,
            duration,
            _marker: std::marker::PhantomData,
        }
    }
}

impl Spacer<Decreasing> {
    fn new(widget: Widget, min_height: usize, max_height: usize, duration: Duration) -> Self {
        Self {
            widget,
            min_height,
            max_height,
            current_height: max_height,
            elapsed_ns: 0,
            duration,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<Direction: ChangeDirection> Spacer<Direction> {
    fn update(&mut self, delta_time_ns: u64) {
        if self.is_finished() {
            return;
        }

        self.elapsed_ns += delta_time_ns;
        self.current_height = Direction::get_current_value(
            self.elapsed_ns as f32 / self.duration.as_nanos() as f32,
            self.min_height,
            self.max_height,
        )
    }

    fn is_finished(&self) -> bool {
        self.elapsed_ns as u128 > self.duration.as_nanos()
    }
}
