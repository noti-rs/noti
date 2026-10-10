use config::{display::DisplayConfig, size::Size, Config};
use dbus::{actions::ClosingReason, notification::Notification};
use indexmap::{
    indexmap,
    map::{Iter, Values, ValuesMut},
    IndexMap,
};
use log::{debug, trace, warn};
use shared::text;
use std::{cmp::Ordering, collections::VecDeque, hash::Hash, path::PathBuf, time};
use widgets::{
    self,
    animations::AnimationKind,
    context::{
        Context, CreateState, GetState, SetState, StateLifetimeManagement, WidgetTreeCreation,
    },
    forest::NodeId,
    make_widget,
    stage::measure::Constraints,
    state::MutableState,
    types::{Alignment, Border, Direction, Point, Position, Spacing},
    widget::{
        animated_visibility::{AnimatedVisibility, AnimationDefinition},
        image::ImageProvider,
        Button, ConstrainedBox, FlexBox, Image, Text,
    },
};

use crate::managers::window_manager::audio_playback::{
    decode_from_sound_name, decode_from_sound_path, AudioPlayback,
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
            .for_each(|banner| banner.update_by_config(config, context));
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
        context: &mut Context,
    ) -> Vec<(Notification, ClosingReason)> {
        self.banners
            .drain_filter(|(_, banner)| banner.is_closed(context) && banner.is_finished(context))
            .into_iter()
            .map(|(_, banner)| {
                let closing_reason = context
                    .get(banner.banner_state.close_status)
                    .expect("Close status must exist in a banner state!")
                    .to_reason()
                    .expect("A banner pust be properly closed with a closing reason!");
                banner.banner_state.clear_states(context);

                (banner.notification, closing_reason)
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
        audio_playback: &mut AudioPlayback,
    ) where
        I: Iterator<Item = Notification>,
    {
        for notification in notifications {
            self.banners.insert(
                notification.id,
                Banner::new(notification, config, context, audio_playback),
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
    banner_state: BannerState,
}

struct BannerState {
    timeout: u128,
    shown_at: MutableState<time::Instant>,
    banner_phase: MutableState<BannerPhase>,

    visibility_state: MutableState<bool>,
    hovered: MutableState<bool>,
    close_status: MutableState<CloseStatus>,

    app_name_state: MutableState<text::Text>,
    summary_state: MutableState<text::Text>,
    body_state: MutableState<text::Text>,

    thumbnail_state: MutableState<ImageProvider>,
    app_icon_state: MutableState<ImageProvider>,
    window_close_icon_state: MutableState<ImageProvider>,
}

enum BannerPhase {
    NotShown,
    Shown,
    Disappearing,
    Closed,
}

impl Banner {
    pub(super) fn new(
        notification: Notification,
        config: &Config,
        context: &mut Context,
        audio_playback: &mut AudioPlayback,
    ) -> Self {
        let display = config.display_by_app(&notification.app_name);

        if !notification.hints.suppress_sound.unwrap_or(false) && !audio_playback.is_busy() {
            if let Some(file_reader) = display
                .sound
                .path
                .to_option()
                .and_then(decode_from_sound_path)
                .or_else(|| {
                    notification
                        .hints
                        .sound_file
                        .as_deref()
                        .and_then(decode_from_sound_path)
                })
                .or_else(|| {
                    notification
                        .hints
                        .sound_name
                        .as_deref()
                        .and_then(decode_from_sound_name)
                })
            {
                audio_playback.play_audio(file_reader, config.general());
            } else {
                audio_playback.play_default_audio(config.general());
            }
        }

        let banner_state = BannerState {
            timeout: display.timeout.by_urgency(&notification.hints.urgency) as u128,
            shown_at: context.create_state_mut(time::Instant::now()),
            banner_phase: context.create_state_mut(BannerPhase::NotShown),

            visibility_state: context.create_state_mut(true),
            hovered: context.create_state_mut(false),
            close_status: context.create_state_mut(CloseStatus::NotClosed),

            app_name_state: context.create_state_mut(text::Text {
                body: notification.app_name.to_string(),
                entities: vec![],
            }),
            summary_state: context.create_state_mut(notification.summary.clone()),
            body_state: context.create_state_mut(notification.body.clone()),

            thumbnail_state: context
                .create_state_mut(resolve_notification_thumbnail(&notification, display)),
            app_icon_state: context.create_state_mut(resolve_app_icon(
                &notification,
                display,
                None,
            )),

            window_close_icon_state: context.create_state_mut(resolve_window_close_icon(display)),
        };
        debug!("Banner (id={}): Created", notification.id);

        Self {
            notification,
            banner_state,
        }
    }

    pub(super) fn build_widget_tree(&self, context: &mut Context, config: &Config) -> NodeId {
        fn get_constraints(size: Size) -> Constraints<usize> {
            match size {
                Size::Fixed(val) => Constraints::new_tight(val as usize),
                Size::Dynamic { min, max } => Constraints {
                    min: min as usize,
                    max: max as usize,
                },
            }
        }

        let display = config.display_by_app(&self.notification.app_name);
        let theme = config.theme_by_app(&self.notification.app_name);
        let colors = theme.by_urgency(&self.notification.hints.urgency);

        let width_constraints = get_constraints(display.width);
        let height_constraints = get_constraints(display.height);

        let BannerState {
            shown_at,
            banner_phase,

            visibility_state,
            hovered,
            close_status,

            app_name_state,
            summary_state,
            body_state,

            thumbnail_state,
            app_icon_state,
            window_close_icon_state,
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
                    let close_status = context.get(close_status);

                    if close_status.is_some_and(|status| !status.is_closed()) {
                        context.set(visibility_state, true);
                        context.set(hovered, true);
                    }
                },
                on_leave: move |mut context, _| {
                    context.set(hovered, false);
                    context.set(shown_at, time::Instant::now());
                }
            ) {
                make_widget! {
                    context <== ConstrainedBox(
                        min_width: width_constraints.min,
                        max_width: width_constraints.max,

                        min_height: height_constraints.min,
                        max_height: height_constraints.max,
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
                                if display.top_bar.enable {
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
                                                    if display.top_bar.show_icon {
                                                        make_widget! {
                                                            context <== Image(
                                                                state: app_icon_state
                                                            )
                                                        }
                                                    } else {
                                                        None
                                                    },
                                                    make_widget! {
                                                        context <== Text(
                                                            state: app_name_state,
                                                            font: widgets::widget::Font {
                                                                name: display.app_name.font.name.clone(),
                                                                size: display.app_name.font_size as usize,
                                                                style: display.app_name.style.into(),
                                                            },
                                                            wrap: display.app_name.wrap,
                                                            margin: display.app_name.margin.into(),
                                                            alignment: display.app_name.alignment.into(),
                                                            line_spacing: display.app_name.line_spacing as usize,
                                                            color: colors.foreground.clone().into(),
                                                        )
                                                    },
                                                }
                                            },
                                            make_widget! {
                                                context <== Button(
                                                    on_click: move |mut context, _| {
                                                        context.set(visibility_state, false);
                                                        context.set(banner_phase, BannerPhase::Disappearing);
                                                        context.set(close_status, CloseStatus::Closed(ClosingReason::DismissedByUser));
                                                    }
                                                ) {
                                                    make_widget! {
                                                        context <== Image(
                                                            state: window_close_icon_state,
                                                        )
                                                    }
                                                }
                                            },
                                        }
                                    }
                                } else {
                                    None
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
        // INFO: declarative macro explicitly wraps into `Some(NodeId)` and we know it. So just
        // unwrap and go.
        .unwrap()
    }

    pub(super) fn close(&mut self, context: &mut Context, closing_reason: ClosingReason) {
        context.set(
            self.banner_state.close_status,
            CloseStatus::Closed(closing_reason),
        );
    }

    pub(super) fn is_closed(&self, context: &Context) -> bool {
        context
            .get(self.banner_state.close_status)
            .map(|close_status| close_status.is_closed())
            .unwrap_or(false)
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
        let display = config.display_by_app(&self.notification.app_name);

        self.banner_state.update_texts(&self.notification, context);
        self.banner_state
            .update_images(&self.notification, display, context);

        self.reset_timeout(context);
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.notification.id
        );
    }

    pub(super) fn update_by_config(&mut self, config: &Config, context: &mut Context) {
        let display = config.display_by_app(&self.notification.app_name);

        self.banner_state.timeout =
            display.timeout.by_urgency(&self.notification.hints.urgency) as u128;

        self.banner_state
            .update_images(&self.notification, display, context);
    }

    pub(super) fn update(&mut self, context: &mut Context) {
        match context
            .get(self.banner_state.banner_phase)
            .expect("Banner State must be created!")
        {
            BannerPhase::NotShown | BannerPhase::Disappearing => (),
            BannerPhase::Shown => {
                if context
                    .get(self.banner_state.hovered)
                    .copied()
                    .unwrap_or_default()
                {
                    return;
                }

                let shown_at = context
                    .get(self.banner_state.shown_at)
                    .expect("Time snapshot must be created!");

                if self.banner_state.timeout != 0
                    && shown_at.elapsed().as_millis() >= self.banner_state.timeout
                {
                    context.set(self.banner_state.visibility_state, false);
                    context.set(self.banner_state.banner_phase, BannerPhase::Disappearing);
                }
            }
            BannerPhase::Closed => {
                context.set(
                    self.banner_state.close_status,
                    CloseStatus::Closed(ClosingReason::Expired),
                );
            }
        }
    }
}

impl BannerState {
    fn clear_states(&self, context: &mut Context) {
        let &Self {
            timeout: _timeout,
            shown_at,
            banner_phase,

            visibility_state,
            hovered,
            close_status,

            app_name_state,
            summary_state,
            body_state,

            thumbnail_state,
            app_icon_state,
            window_close_icon_state,
        } = self;

        context.release(shown_at);
        context.release(banner_phase);

        context.release(visibility_state);
        context.release(hovered);
        context.release(close_status);

        context.release(app_name_state);
        context.release(summary_state);
        context.release(body_state);

        context.release(thumbnail_state);
        context.release(app_icon_state);
        context.release(window_close_icon_state);
    }

    fn update_texts(&self, notification: &Notification, context: &mut Context) {
        context.set(
            self.app_name_state,
            text::Text {
                body: notification.app_name.clone(),
                entities: vec![],
            },
        );

        context.set(self.summary_state, notification.summary.clone());
        context.set(self.body_state, notification.body.clone());
    }

    fn update_images(
        &self,
        notification: &Notification,
        display: &DisplayConfig,
        context: &mut Context,
    ) {
        context.set(
            self.thumbnail_state,
            resolve_notification_thumbnail(notification, display),
        );

        context.set(
            self.app_icon_state,
            resolve_app_icon(notification, display, None),
        );

        context.set(
            self.window_close_icon_state,
            resolve_window_close_icon(display),
        );
    }
}

fn resolve_notification_thumbnail(
    notification: &Notification,
    display: &DisplayConfig,
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
            notification.hints.image_path.as_deref().map(|path| {
                if path.starts_with("file://") {
                    ImageProvider::ImagePath(parse_uri_path(path).unwrap_or_default())
                } else {
                    ImageProvider::Icon {
                        name: path.to_owned(),
                        theme: display.icons.theme.clone(),
                        sizes: display.icons.size.clone(),
                    }
                }
            })
        })
        .or_else(|| {
            if display.top_bar.enable {
                return None;
            }

            if notification.app_icon.is_empty() {
                return None;
            }

            Some(resolve_app_icon(
                notification,
                display,
                display.icons.size.clone(),
            ))
        })
        .unwrap_or(ImageProvider::Unknown)
}

fn resolve_app_icon<S: Into<Option<Vec<u16>>>>(
    notification: &Notification,
    display: &DisplayConfig,
    sizes: S,
) -> ImageProvider {
    if notification.app_icon.starts_with("file://") {
        ImageProvider::ImagePath(dbg!(
            parse_uri_path(&notification.app_icon).unwrap_or_default()
        ))
    } else {
        ImageProvider::Icon {
            name: if !notification.app_icon.is_empty() {
                notification.app_icon.clone()
            } else {
                notification.hints.desktop_entry.clone().unwrap_or_default()
            },
            theme: display.icons.theme.clone(),
            sizes: sizes.into().unwrap_or(vec![22, 18, 16, 14, 12]),
        }
    }
}

fn resolve_window_close_icon(display: &DisplayConfig) -> ImageProvider {
    ImageProvider::Icon {
        name: "window-close".to_string(),
        theme: display.icons.theme.clone(),
        sizes: vec![22, 18, 16, 14, 12],
    }
}

fn parse_uri_path(path: &str) -> Option<PathBuf> {
    url::Url::parse(path).ok().and_then(|path| {
        debug_assert!(path.scheme() == "file");

        path.to_file_path().ok()
    })
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

    fn to_reason(&self) -> Option<ClosingReason> {
        match self {
            CloseStatus::NotClosed => None,
            CloseStatus::Closed(reason) => Some(reason.clone()),
        }
    }
}

fn correct_animation(
    config: &Config,
    mut animation_definition: AnimationDefinition,
) -> AnimationDefinition {
    if let AnimationKind::Translate(translate) = &mut animation_definition.kind {
        const ADDITION: f32 = 50.0;

        let max_banner_width = config
            .displays()
            .map(|display| match display.width {
                Size::Fixed(val) => val,
                Size::Dynamic { max, .. } => max,
            })
            .max()
            .unwrap_or_default() as f32;

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
