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
    context::{Context, CreateState, GetState, SetState, WidgetTreeCreation},
    forest::NodeId,
    make_widget,
    state::MutableState,
    types::{Alignment, Border, Direction, Point, Position, Spacing},
    widget::{
        animated_visibility::{AnimatedVisibility, AnimationDefinition},
        image::ImageProvider,
        Button, ConstrainedBox, FlexBox, Image, Text,
    },
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

    /// Updates the banner stack and banners by newly updated user configuration.
    pub(super) fn configure(&mut self, config: &Config, context: &mut Context) {
        self.sort_by_config(config);
        self.banners_mut()
            .for_each(|banner| banner.update_config(config, context));
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
    pub(super) fn remove_closed(
        &mut self,
        context: &Context,
    ) -> Vec<(Notification, ClosingReason)> {
        self.banners
            .drain_filter(|(_, banner)| banner.is_closed() && banner.is_finished(context))
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
        context: &mut Context,
    ) {
        let notifications_to_replace =
            notifications.drain_filter(|notification| self.banners.get(&notification.id).is_some());

        for notification in notifications_to_replace {
            let id = notification.id;
            self.banners[&id].update_data(notification, config, context);
        }

        self.sort_by_config(config);
    }

    /// Extends current container with new banners that will be created from notification. Note
    /// that existing banner with the same notification id will be replaced.
    pub(super) fn extend_from<I>(
        &mut self,
        notifications: I,
        config: &Config,
        context: &mut Context,
    ) where
        I: Iterator<Item = Notification>,
    {
        for notification in notifications {
            self.banners
                .insert(notification.id, Banner::new(notification, config, context));
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
    close_status: CloseStatus,

    banner_state: BannerState,
}

struct BannerState {
    timeout: u128,
    shown_at: MutableState<time::Instant>,
    banner_phase: MutableState<BannerPhase>,

    summary_state: MutableState<text::Text>,
    body_state: MutableState<text::Text>,
    thumbnail_state: MutableState<ImageProvider>,
    visibility_state: MutableState<bool>,
}

enum BannerPhase {
    NotShown,
    Shown,
    Closing,
    Closed,
}

impl Banner {
    pub(super) fn new(notification: Notification, config: &Config, context: &mut Context) -> Self {
        let display = config.display_by_app(&notification.app_name);

        let banner_state = BannerState {
            timeout: display.timeout.by_urgency(&notification.hints.urgency) as u128,
            shown_at: context.create_state_mut(time::Instant::now()),
            banner_phase: context.create_state_mut(BannerPhase::NotShown),
            summary_state: context.create_state_mut(notification.summary.clone()),
            body_state: context.create_state_mut(notification.body.clone()),
            thumbnail_state: context
                .create_state_mut(get_notification_thumbnail(&notification, display)),
            visibility_state: context.create_state_mut(true),
        };
        debug!("Banner (id={}): Created", notification.id);

        Self {
            notification,
            close_status: CloseStatus::NotClosed,
            banner_state,
        }
    }

    pub(super) fn build_widget_tree(&self, context: &mut Context, config: &Config) -> NodeId {
        let width = config.general().width;
        let height = config.general().height;

        let display = config.display_by_app(&self.notification.app_name);
        let theme = config.theme_by_app(&self.notification.app_name);
        let colors = theme.by_urgency(&self.notification.hints.urgency);

        let BannerState {
            visibility_state,
            banner_phase,
            shown_at,
            summary_state,
            body_state,
            thumbnail_state,
            ..
        } = self.banner_state;

        make_widget! {
            context <== AnimatedVisibility(
                key: format!("banner-{}", self.notification.id).into(),

                visibility_state: visibility_state,

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
                    context.set(visibility_state, true);
                },
            ) {
                make_widget! {
                    context <== ConstrainedBox(
                        min_width: width as usize,
                        max_width: width as usize,

                        min_height: width as usize,
                        max_height: height as usize,
                    ) {
                        make_widget! {
                            context <== FlexBox(
                                direction: Direction::Vertical,
                                alignment: Alignment::new(Position::Start, Position::Center),
                                background_color: colors.background.clone().into(),
                                border: Border {
                                    size: display.border.size as usize,
                                    radius: display.border.radius as usize,
                                    color: colors.border.clone().into(),
                                },
                            ) {
                                make_widget! {
                                    context <== FlexBox(
                                        direction: Direction::Horizontal,
                                        spacing: 10,
                                        padding: Spacing { left: 15, right: 5, ..Default::default() },
                                        alignment: Alignment::new(Position::SpaceBetween, Position::Center),
                                        expand: true,
                                    ) {
                                        make_widget! {
                                            context <== FlexBox(
                                                direction: Direction::Horizontal,
                                                spacing: 10,
                                            ) {
                                                make_widget! {
                                                    context <== Image(
                                                        state: context.create_state(ImageProvider::Icon {
                                                            name: self.notification.app_icon.clone(),
                                                            theme: display.icons.theme.clone(),
                                                            sizes: vec![22, 12]
                                                        }),
                                                    )
                                                },
                                                make_widget! {
                                                    context <== Text(
                                                        state: context.create_state(text::Text {
                                                            body: self.notification.app_name.to_string(), entities: vec![]
                                                        }),
                                                        color: colors.foreground.clone().into(),
                                                    )
                                                },
                                            }
                                        },
                                        make_widget! {
                                            context <== Button(
                                                on_click: move |mut context, _| {
                                                    context.set(visibility_state, false);
                                                    context.set(banner_phase, BannerPhase::Closed);
                                                }
                                            ) {
                                                make_widget! {
                                                    context <== Image(
                                                        state: context.create_state(ImageProvider::Icon {
                                                            name: "window-close".to_string(),
                                                            theme: display.icons.theme.clone(),
                                                            sizes: vec![22, 12]
                                                        })
                                                    )
                                                }
                                            }
                                        },
                                    }
                                },
                                make_widget! {
                                    context <== FlexBox(
                                        direction: Direction::Horizontal,
                                        alignment: Alignment::new(Position::Start, Position::Center),
                                        padding: display.padding.into(),
                                    ) {
                                        make_widget! {
                                            context <== Image (
                                                state: thumbnail_state,
                                                rounding: display.image.rounding,
                                                margin: display.image.margin.into(),
                                                resizing_method: display.image.resizing_method.into(),
                                                mipmap_mode: display.image.mipmap_mode.into(),
                                                fit_mode: display.image.fit_mode.into(),
                                            )
                                        },
                                        make_widget!{
                                            context <== FlexBox(
                                                direction: Direction::Vertical,
                                                alignment: Alignment::new(Position::Center, Position::Center),
                                            ) {
                                                make_widget! {
                                                    context <== Text (
                                                        state: summary_state,
                                                        font: widgets::widget::Font {
                                                            name: display.summary.font.name.clone(),
                                                            size: display.summary.font_size as usize,
                                                            style: display.summary.style.into(),
                                                        },
                                                        wrap: display.summary.wrap,
                                                        margin: display.summary.margin.into(),
                                                        alignment: display.summary.alignment.into(),
                                                        line_spacing: display.summary.line_spacing as usize,
                                                        color: colors.foreground.clone().into(),
                                                    )
                                                },
                                                make_widget! {
                                                    context <== Text (
                                                        state: body_state,
                                                        font: widgets::widget::Font {
                                                            name: display.body.font.name.clone(),
                                                            size: display.body.font_size as usize,
                                                            style: display.body.style.into(),
                                                        },
                                                        wrap: display.body.wrap,
                                                        margin: display.body.margin.into(),
                                                        alignment: display.body.alignment.into(),
                                                        line_spacing: display.body.line_spacing as usize,
                                                        color: colors.foreground.clone().into(),
                                                    )
                                                }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                },
            }
        }
    }

    pub(super) fn close(&mut self, closing_reason: ClosingReason) {
        self.close_status.close_with(closing_reason);
    }

    pub(super) fn is_closed(&self) -> bool {
        self.close_status.is_closed()
    }

    fn is_finished(&self, context: &Context) -> bool {
        matches!(
            context.get(self.banner_state.banner_phase).unwrap(),
            BannerPhase::Closed
        )
    }

    pub(super) fn reset_timeout(&mut self, context: &mut Context) {
        context.set(self.banner_state.shown_at, time::Instant::now());

        trace!("Banner (id={}): Timeout reset", self.notification.id);
    }

    pub(super) fn update_data(
        &mut self,
        notification: Notification,
        config: &Config,
        context: &mut Context,
    ) {
        self.notification = notification;

        context.set(
            self.banner_state.summary_state,
            self.notification.summary.clone(),
        );
        context.set(self.banner_state.body_state, self.notification.body.clone());
        context.set(
            self.banner_state.thumbnail_state,
            get_notification_thumbnail(
                &self.notification,
                config.display_by_app(&self.notification.app_name),
            ),
        );

        self.reset_timeout(context);
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.notification.id
        );
    }

    pub(super) fn update_config(&mut self, config: &Config, context: &mut Context) {
        let display_config = config.display_by_app(&self.notification.app_name);

        self.banner_state.timeout = display_config
            .timeout
            .by_urgency(&self.notification.hints.urgency)
            as u128;

        context.set(
            self.banner_state.thumbnail_state,
            get_notification_thumbnail(&self.notification, display_config),
        );
    }

    pub(super) fn update(&mut self, context: &mut Context) {
        match context
            .get(self.banner_state.banner_phase)
            .expect("Banner State must be created!")
        {
            BannerPhase::NotShown | BannerPhase::Closing => (),
            BannerPhase::Shown => {
                let shown_at = context
                    .get(self.banner_state.shown_at)
                    .expect("Time snapshot must be created!");

                if self.banner_state.timeout != 0
                    && shown_at.elapsed().as_millis() >= self.banner_state.timeout
                    && *context.get(self.banner_state.visibility_state).unwrap()
                {
                    context.set(self.banner_state.visibility_state, false);
                    context.set(self.banner_state.banner_phase, BannerPhase::Closing);
                }
            }
            BannerPhase::Closed => {
                self.close_status.close_with(ClosingReason::Expired);
            }
        }
    }
}

fn get_notification_thumbnail(
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
        // .or_else(|| {
        //     if notification.app_icon.is_empty() {
        //         return None;
        //     }
        //
        //     Some(ImageProvider::Icon {
        //         name: notification.app_icon.clone(),
        //         theme: display_config.icons.theme.clone(),
        //         sizes: display_config.icons.size.clone(),
        //     })
        // })
        .unwrap_or(ImageProvider::Unknown)
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
