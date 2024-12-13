use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeZone, Utc};
use dbus::notification::ScheduledNotification;
use log::{debug, warn};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub struct Scheduler {
    queue: BinaryHeap<Reverse<ScheduledNotification>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            queue: BinaryHeap::new(),
        }
    }

    pub fn add(&mut self, notification: ScheduledNotification) {
        match Self::parse_time(&notification.time) {
            Ok(parsed_time) => {
                debug!(
                    "Successfully parsed time '{}' for scheduling: {}",
                    &notification.time, &parsed_time
                );

                let scheduled_notification = ScheduledNotification {
                    time: parsed_time.to_rfc3339(),
                    data: notification.data,
                    id: notification.id,
                };
                self.queue.push(Reverse(scheduled_notification));
            }
            Err(e) => {
                warn!(
                    "Failed to parse time '{}' for notification with id '{}': {}",
                    &notification.time, &notification.data.id, e
                );
            }
        }
    }

    pub fn pop_due_notifications(&mut self) -> Vec<ScheduledNotification> {
        let now = Utc::now();
        let mut due_notifications = Vec::new();

        while let Some(Reverse(top)) = self.queue.peek() {
            match top.time.parse::<DateTime<Utc>>() {
                Ok(time) if time <= now => {
                    due_notifications.push(self.queue.pop().unwrap().0);
                }
                _ => break,
            }
        }

        due_notifications
    }

    fn parse_time(time_str: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
        let now = Utc::now();

        if let Ok(duration) = humantime::parse_duration(time_str) {
            return Ok(now + chrono::Duration::from_std(duration).unwrap());
        }

        const DATETIME_FORMATS: &[&str] = &[
            "%Y.%m.%d %H:%M", // 2025.01.01 00:00
            "%Y-%m-%d %H:%M", // 2025-01-01 00:00
            //
            "%d.%m.%Y %H:%M", // 01.01.2025 00:00
            "%d-%m-%Y %H:%M", // 01-01-2025 00:00
            //
            "%d.%m.%Y %I:%M %p", // 01.01.2025 12:00 AM
            "%d-%m-%Y %I:%M %p", // 01-01-2025 12:00 AM
            //
            "%Y.%m.%d %I:%M %p", // 2025.01.01 12:00 AM
            "%Y-%m-%d %I:%M %p", // 2025-01-01 12:00 AM
        ];

        const TIME_FORMATS: &[&str] = &[
            "%H:%M",    // 18:45
            "%I:%M %p", // 06:45 PM
        ];

        if let Some(datetime) = DATETIME_FORMATS.iter().find_map(|&format| {
            NaiveDateTime::parse_from_str(time_str, format)
                .ok()
                .map(|parsed| Self::from_local_to_utc(&parsed))
        }) {
            return Ok(datetime);
        }

        if let Some(datetime) = TIME_FORMATS.iter().find_map(|&format| {
            NaiveTime::parse_from_str(time_str, format)
                .ok()
                .map(|parsed| {
                    let today = Local::now().date_naive();
                    Self::from_local_to_utc(&today.and_time(parsed))
                })
        }) {
            return Ok(datetime);
        }

        time_str.parse::<DateTime<Utc>>()
    }

    fn from_local_to_utc(datetime: &NaiveDateTime) -> DateTime<Utc> {
        Local
            .from_local_datetime(datetime)
            .single()
            .expect("Local time conversion failed: ambiguous or invalid local time")
            .with_timezone(&Utc)
    }
}
