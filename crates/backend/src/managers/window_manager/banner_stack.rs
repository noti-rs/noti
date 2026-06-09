use config::{display::DisplayConfig, Config};
use dbus::{actions::ClosingReason, notification::Notification};
use indexmap::{
    indexmap,
    map::{Iter, Values, ValuesMut},
    IndexMap,
};
use log::{debug, trace, warn};
use shared::text;
use std::{cmp::Ordering, collections::VecDeque, hash::Hash, time};
use widgets::{
    self,
    animations::AnimationKind,
    context::{Context, CreateState, DebugOptions, GetState, SetState, SetStyleClass, Tick},
    events::Event,
    make_style, make_widget,
    stage::{draw::Drawer, measure::Constraints},
    state::MutableState,
    types::{Alignment, Border, Direction, Extent, Offset, Point, Position},
    widget::{
        animated_visibility::{AnimatedVisibility, AnimatedVisibilityStyle, AnimationDefinition},
        image::ImageProvider,
        ContainerStyle, FlexContainer, Image, Text, TextStyle,
    },
    UiRoot,
};

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
            .values()
            .map(|banner| banner.width())
            .max()
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
            .for_each(|banner| banner.update_config(config));
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
            self.banners[&id].update_data(notification, config);
        }

        self.sort_by_config(config);
    }

    /// Extends current container with new banners that will be created from notification. Note
    /// that existing banner with the same notification id will be replaced.
    pub(super) fn extend_from<I>(
        &mut self,
        notifications: I,
        font_collection: skia_safe::textlayout::FontCollection,
        config: &Config,
    ) where
        I: Iterator<Item = Notification>,
    {
        for notification in notifications {
            self.banners.insert(
                notification.id,
                Banner::new(notification, font_collection.clone(), config),
            );
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
    ui_root: UiRoot,
    close_status: CloseStatus,

    banner_state: BannerState,
}

struct BannerState {
    timeout: u128,
    shown_at: MutableState<time::Instant>,
    banner_phase: MutableState<BannerPhase>,

    summary_state: MutableState<text::Text>,
    body_state: MutableState<text::Text>,
    image_state: MutableState<ImageProvider>,
    visible_state: MutableState<bool>,
}

enum BannerPhase {
    NotShown,
    Shown,
    Closing,
    Closed,
}

impl Banner {
    const NOTIFICATION_ANIMATED_VISIBILITY: &str = "notification_animated_visibility";
    const NOTIFICATION_FRAME: &str = "notification_frame";
    const NOTIFICATION_SUMMARY: &str = "notification_summary";
    const NOTIFICATION_BODY: &str = "notification_body";
    const NOTIFICATION_IMAGE: &str = "notification_image";

    pub(super) fn new(
        notification: Notification,
        font_collection: skia_safe::textlayout::FontCollection,
        config: &Config,
    ) -> Self {
        let extent = Extent::new(
            config.general().width as f32,
            config.general().height as f32,
        );

        let timeout = config
            .display_by_app(&notification.app_name)
            .timeout
            .by_urgency(&notification.hints.urgency) as u128;

        let display = config.display_by_app(&notification.app_name);

        let mut context = Context::new(font_collection);
        context.update_debug_options(to_debug_options(config));

        let summary_state = context.create_state_mut(notification.summary.clone());
        let summary_widget = make_widget! {
            Text {
                class: Banner::NOTIFICATION_SUMMARY,
                state: summary_state,
            }
        };

        let body_state = context.create_state_mut(notification.body.clone());
        let body_widget = make_widget! {
            Text {
                class: Banner::NOTIFICATION_BODY,
                state: body_state
            }
        };

        let image_state = context.create_state_mut(make_image_provider(&notification, display));
        let image_widget = make_widget! {
            Image {
                class: Banner::NOTIFICATION_IMAGE,
                state: image_state,
            }
        };

        let visible_state = context.create_state_mut(true);
        let shown_at = context.create_state_mut(time::Instant::now());
        let banner_phase = context.create_state_mut(BannerPhase::NotShown);

        let layout = make_widget! {
            AnimatedVisibility {
                state: visible_state,
                class: Banner::NOTIFICATION_ANIMATED_VISIBILITY,

                primary_animation: correct_animation(config, display.animation.primary.clone().into()),
                primary_spatial_change: display.animation.primary_spatial_change.clone().into(),

                secondary_animation: correct_animation(config, display.animation.secondary.clone().into()),
                secondary_spatial_change: display.animation.secondary_spatial_change.clone().into(),

                on_visible: move |mut context, _| {
                    context.set(banner_phase, BannerPhase::Shown);
                    context.set(shown_at, time::Instant::now());
                },
                on_hidden: move |mut context, _| {
                    context.set(banner_phase, BannerPhase::Closed);
                },
                on_hover: move |mut context, _| {
                    context.set(visible_state, true);
                },

                child: make_widget! {
                    FlexContainer {
                        class: Banner::NOTIFICATION_FRAME,
                        direction: Direction::Horizontal,
                        alignment: Alignment::new(Position::Start, Position::Center),
                        children: vec![
                            image_widget.into(),
                            make_widget!{
                                FlexContainer {
                                    direction: Direction::Vertical,
                                    alignment: Alignment::new(Position::Center, Position::Center),
                                    children: vec![summary_widget.into(), body_widget.into()],
                                }
                            }.into()
                        ]
                    }
                }
            }
        }
        .into();

        set_styles(&mut context, &notification, config);

        let mut ui_root = UiRoot::new(layout, context);
        ui_root.layout(Constraints::new_tight(extent).into());

        let banner_state = BannerState {
            timeout,
            shown_at,
            banner_phase,
            summary_state,
            body_state,
            image_state,
            visible_state,
        };
        debug!("Banner (id={}): Created", notification.id);

        Self {
            notification,
            ui_root,
            close_status: CloseStatus::NotClosed,
            banner_state,
        }
    }

    pub(super) fn close(&mut self, closing_reason: ClosingReason) {
        self.close_status.close_with(closing_reason);
    }

    pub(super) fn is_closed(&self) -> bool {
        self.close_status.is_closed()
    }

    fn is_finished(&self) -> bool {
        matches!(
            self.ui_root.get(self.banner_state.banner_phase).unwrap(),
            BannerPhase::Closed
        )
    }

    pub(super) fn reset_timeout(&mut self) {
        self.ui_root
            .set(self.banner_state.shown_at, time::Instant::now());

        trace!("Banner (id={}): Timeout reset", self.notification.id);
    }

    pub(super) fn update_data(&mut self, notification: Notification, config: &Config) {
        self.notification = notification;

        self.ui_root.set(
            self.banner_state.summary_state,
            self.notification.summary.clone(),
        );
        self.ui_root
            .set(self.banner_state.body_state, self.notification.body.clone());
        self.ui_root.set(
            self.banner_state.image_state,
            make_image_provider(
                &self.notification,
                config.display_by_app(&self.notification.app_name),
            ),
        );

        self.reset_timeout();
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.notification.id
        );
    }

    pub(super) fn update_config(&mut self, config: &Config) {
        let extent = Extent::new(
            config.general().width as f32,
            config.general().height as f32,
        );

        self.ui_root.update_debug_options(to_debug_options(config));

        let display_config = config.display_by_app(&self.notification.app_name);
        self.banner_state.timeout = display_config
            .timeout
            .by_urgency(&self.notification.hints.urgency)
            as u128;

        self.ui_root.set(
            self.banner_state.image_state,
            make_image_provider(&self.notification, display_config),
        );

        set_styles(&mut self.ui_root, &self.notification, config);
        self.ui_root.layout(Constraints::new_tight(extent).into());
    }

    // TODO: use it for resize
    pub(super) fn width(&self) -> usize {
        self.ui_root.width() as usize
    }

    pub(super) fn height(&self) -> usize {
        self.ui_root.height() as usize
    }

    /// Draws the notification banner frame into provided surface with offset.
    pub(super) fn draw(&self, offset: &Offset<f32>, sk_surface: &mut skia_safe::Surface) {
        debug!("Banner (id={}): Beginning of draw", self.notification.id);

        let mut drawer = Drawer::use_surface(sk_surface.clone());
        self.ui_root.draw(offset, &mut drawer);

        debug!("Banner (id={}): Complete draw", self.notification.id);
    }

    pub(super) fn dispatch_event(&mut self, event: Event) {
        self.ui_root.dispatch_event(event);
    }
}

fn set_styles<C: SetStyleClass>(context: &mut C, notification: &Notification, config: &Config) {
    macro_rules! make_text_config {
        (for $kind:ident use $compile_ctx:ident, $display_config:ident, $colors:ident, $id:ident) => {
            $compile_ctx.set_style_class(
                Banner::$id,
                widgets::types::WidgetStyle::Text(make_style! {
                    TextStyle {
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
                    }
                }),
            );
        };
    }

    let display_config = config.display_by_app(&notification.app_name);
    let theme = config.theme_by_app(&notification.app_name);
    let colors = theme.by_urgency(&notification.hints.urgency);

    context.set_style_class(
        Banner::NOTIFICATION_ANIMATED_VISIBILITY,
        widgets::types::WidgetStyle::AnimatedVisibility(make_style! {
            AnimatedVisibilityStyle {
                primary_animation: correct_animation(config, display_config.animation.primary.clone().into()),
                primary_spatial_change: display_config.animation.primary_spatial_change.clone().into(),

                secondary_animation: correct_animation(config, display_config.animation.secondary.clone().into()),
                secondary_spatial_change: display_config.animation.secondary_spatial_change.clone().into(),
            }
        })
    );

    context.set_style_class(
        Banner::NOTIFICATION_FRAME,
        widgets::types::WidgetStyle::Container(make_style! {
            ContainerStyle {
                background_color: colors.background.clone().into(),
                border: Border {
                    size: display_config.border.size as usize,
                    radius: display_config.border.radius as usize,
                    color: colors.border.clone().into(),
                },
                spacing: display_config.padding.into(),
                alignment: Alignment::new(Position::Start, Position::Center),
            }
        }),
    );

    context.set_style_class(
        Banner::NOTIFICATION_IMAGE,
        widgets::types::WidgetStyle::Image(display_config.image.clone().into()),
    );

    make_text_config!(for summary use context, display_config, colors, NOTIFICATION_SUMMARY);
    make_text_config!(for body use context, display_config, colors, NOTIFICATION_BODY);
}

fn make_image_provider(
    notification: &Notification,
    display_config: &DisplayConfig,
) -> ImageProvider {
    notification
        .hints
        .image_data
        .as_ref()
        .cloned()
        .map(|image_data| {
            ImageProvider::ImageInfo(widgets::widget::ImageInfo {
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
                .map(ImageProvider::ImagePath)
        })
        .or_else(|| {
            if notification.app_icon.is_empty() {
                return None;
            }

            Some(ImageProvider::Icon {
                name: notification.app_icon.clone(),
                theme: display_config.icons.theme.clone(),
                sizes: display_config.icons.size.clone(),
            })
        })
        .unwrap_or(ImageProvider::Unknown)
}

impl Tick for Banner {
    fn tick(&mut self, delta_ns: u128) {
        self.ui_root.tick(delta_ns);

        self.ui_root.invalidate();
        self.ui_root.layout(None);

        match self
            .ui_root
            .get(self.banner_state.banner_phase)
            .expect("Banner State must be created!")
        {
            BannerPhase::NotShown | BannerPhase::Closing => (),
            BannerPhase::Shown => {
                let shown_at = self
                    .ui_root
                    .get(self.banner_state.shown_at)
                    .expect("Time snapshot must be created!");

                if self.banner_state.timeout != 0
                    && shown_at.elapsed().as_millis() >= self.banner_state.timeout
                    && *self.ui_root.get(self.banner_state.visible_state).unwrap()
                {
                    self.ui_root.set(self.banner_state.visible_state, false);
                    self.ui_root
                        .set(self.banner_state.banner_phase, BannerPhase::Closing);
                }
            }
            BannerPhase::Closed => {
                self.close_status.close_with(ClosingReason::Expired);
            }
        }
    }
}

impl<'a> From<&'a Banner> for &'a Notification {
    fn from(value: &'a Banner) -> Self {
        &value.notification
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

fn to_debug_options(config: &Config) -> DebugOptions {
    DebugOptions {
        show_layout_bounds: config.general().debug.show_layout_bounds,
    }
}

fn correct_animation(
    config: &Config,
    mut animation_definition: AnimationDefinition,
) -> AnimationDefinition {
    if let AnimationKind::Translate(translate) = &mut animation_definition.kind {
        const ADDITION: f32 = 50.0;
        let max_banner_width = config.general().width as f32;
        let horizontal_margin = config.general().offset.0 as f32;

        let mut start = Point { x: 0.0, y: 0.0 };
        let end = start;

        if config.general().anchor.is_left() {
            start.x = -(max_banner_width + horizontal_margin + ADDITION);
        } else if config.general().anchor.is_right() {
            start.x = max_banner_width + horizontal_margin + ADDITION;
        } else {
            warn!("Slide animation may work incorrectly at middle of screen!")
        }

        translate.update_path(start, end);
    }

    animation_definition
}
