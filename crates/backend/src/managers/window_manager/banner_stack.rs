use config::{
    display::{Border, DisplayConfig},
    Config,
};
use dbus::notification::{self, Notification};
use indexmap::{
    indexmap,
    map::{Iter, Values, ValuesMut},
    IndexMap,
};
use log::{debug, error, trace};
use render::{
    drawer::Drawer,
    types::RectSize,
    widget::{
        self, Alignment, Draw, FlexContainerBuilder, Position, WImage, WText, WTextKind, Widget,
        WidgetConfiguration,
    },
    PangoContext,
};
use shared::cached_data::CachedData;
use std::{cmp::Ordering, collections::VecDeque, hash::Hash, path::PathBuf, time};

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
            .for_each(|banner| banner.is_drawn = false);
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
    /// Removes banner by notification id.
    pub(super) fn remove(&mut self, key: u32) -> Option<Notification> {
        self.banners
            .shift_remove(&key)
            .map(Banner::into_notification)
    }

    /// Removes banners by notification indices.
    pub(super) fn remove_by_keys(&mut self, keys: &[u32]) -> Vec<Notification> {
        keys.iter()
            .filter_map(|id| self.banners.shift_remove(id))
            .map(Banner::into_notification)
            .collect()
    }

    /// Removes expired banners by timeout.
    pub(super) fn remove_expired(&mut self, config: &Config) -> Vec<Notification> {
        self.banners
            .drain_filter(|(_, banner)| banner.is_expired(config))
            .into_iter()
            .map(|(_, banner)| banner.into_notification())
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

pub(super) struct Banner {
    notification: Notification,
    layout: Option<Widget>,
    created_at: time::Instant,

    is_drawn: bool,
}

impl Banner {
    pub(super) fn init(notification: Notification) -> Self {
        debug!("Banner (id={}): Created", notification.id);

        Self {
            notification,
            layout: None,
            created_at: time::Instant::now(),

            is_drawn: false,
        }
    }

    pub(super) fn is_drawn(&self) -> bool {
        self.is_drawn
    }

    pub(super) fn notification(&self) -> &Notification {
        &self.notification
    }

    pub(super) fn into_notification(self) -> Notification {
        debug!("Banner (id={}): Destroyed", self.notification.id);
        self.notification
    }

    pub(super) fn is_expired(&self, config: &Config) -> bool {
        match &self.notification.expire_timeout {
            notification::Timeout::Millis(millis) => {
                self.created_at.elapsed().as_millis() > *millis as u128
            }
            notification::Timeout::Configurable => {
                let timeout = config
                    .display_by_app(&self.notification.app_name)
                    .timeout
                    .by_urgency(&self.notification.hints.urgency);
                timeout != 0 && self.created_at.elapsed().as_millis() > timeout as u128
            }
            notification::Timeout::Never => false,
        }
    }

    pub(super) fn reset_timeout(&mut self) {
        self.created_at = time::Instant::now();

        trace!("Banner (id={}): Timeout reset", self.notification.id);
    }

    pub(super) fn update_data(&mut self, notification: Notification) {
        self.notification = notification;
        self.created_at = time::Instant::now();
        self.is_drawn = false;
        debug!(
            "Banner (id={}): Updated notification data and timeout",
            self.notification.id
        );
    }

    // TODO: use it for resize
    pub(super) fn width(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.width())
            .unwrap_or_default()
    }

    pub(super) fn height(&self) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.height())
            .unwrap_or_default()
    }

    pub(super) fn draw(
        &mut self,
        pango_context: &PangoContext,
        config: &Config,
        cached_layouts: &CachedData<PathBuf, CachedLayout>,
    ) -> DrawState<Vec<u8>> {
        debug!("Banner (id={}): Beginning of draw", self.notification.id);

        let rect_size = RectSize::new(
            config.general().width as usize,
            config.general().height as usize,
        );

        let display = config.display_by_app(&self.notification.app_name);
        let mut drawer = match Drawer::create(rect_size) {
            Ok(drawer) => drawer,
            Err(err) => {
                error!("Failed to create drawer for Banner(id={}), avoided to draw banner. Error: {err}", self.notification.id);
                return DrawState::Failure;
            }
        };

        let mut layout = match &display.layout {
            config::display::Layout::Default => Self::default_layout(display),
            config::display::Layout::FromPath { path_buf } => cached_layouts
                .get(path_buf)
                .and_then(CachedLayout::layout)
                .cloned()
                .unwrap_or_else(|| Self::default_layout(display)),
        };

        layout.compile(
            rect_size,
            &WidgetConfiguration {
                display_config: display,
                theme: config.theme_by_app(&self.notification.app_name),
                notification: &self.notification,
                pango_context,
                override_properties: display.layout.is_default(),
            },
        );

        if let Err(err) = layout.draw(pango_context, &mut drawer) {
            error!(
                "Failed to draw banner Banner(id={}). Error: {err}",
                self.notification.id
            );
            return DrawState::Failure;
        }

        self.layout = Some(layout);
        let framebuffer: Vec<u8> = match drawer.try_into() {
            Ok(val) => val,
            Err(err) => {
                error!(
                    "Failed to get data after drawing Banner(id={}). Error: {err}",
                    self.notification.id
                );
                return DrawState::Failure;
            }
        };

        self.is_drawn = true;
        debug!("Banner (id={}): Complete draw", self.notification.id);
        DrawState::Success(framebuffer)
    }

    fn default_layout(display_config: &DisplayConfig) -> Widget {
        FlexContainerBuilder::default()
            .spacing(display_config.padding.clone())
            .border(display_config.border.clone())
            .direction(widget::Direction::Horizontal)
            .alignment(Alignment::new(Position::Start, Position::Center))
            .children(vec![
                WImage::new().into(),
                FlexContainerBuilder::default()
                    .spacing(Default::default())
                    .border(Border::default())
                    .direction(widget::Direction::Vertical)
                    .alignment(Alignment::new(Position::Center, Position::Center))
                    .transparent_background(true)
                    .children(vec![
                        WText::new(WTextKind::Summary).into(),
                        WText::new(WTextKind::Body).into(),
                    ])
                    .build()
                    .unwrap()
                    .into(),
            ])
            .build()
            .unwrap()
            .into()
    }
}

impl From<Notification> for Banner {
    fn from(value: Notification) -> Self {
        Self::init(value)
    }
}

impl<'a> From<&'a Banner> for &'a Notification {
    fn from(value: &'a Banner) -> Self {
        &value.notification
    }
}

pub(super) enum DrawState<T> {
    Success(T),
    Failure,
}
