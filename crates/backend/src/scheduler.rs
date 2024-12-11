use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeZone, Utc};
use dbus::notification::ScheduledNotification;
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
                dbg!(&parsed_time);

                let scheduled_notification = ScheduledNotification {
                    time: parsed_time.to_rfc3339(),
                    data: notification.data,
                    id: notification.id,
                };
                self.queue.push(Reverse(scheduled_notification));
            }
            Err(e) => {
                eprintln!("Error parsing time '{}': {}", notification.time, e);
            }
        }
    }

    fn parse_time(time_str: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
        let now = Utc::now();

        if let Ok(duration) = humantime::parse_duration(time_str) {
            return Ok(now + chrono::Duration::from_std(duration).unwrap());
        }

        let datetime_formats = [
            "%d.%m.%Y %H:%M",    // 01.01.2025 00:00
            "%Y-%m-%d %H:%M",    // 01-01-2025 00:00
            "%d.%m.%Y %I:%M %p", // 01.01.2025 12:00 AM
            "%Y-%m-%d %I:%M %p", // 01-01-2025 12:00 AM
        ];

        for format in &datetime_formats {
            if let Ok(datetime) = NaiveDateTime::parse_from_str(time_str, format) {
                return Ok(Local
                    .from_local_datetime(&datetime)
                    .single()
                    .expect("Ambiguous local time")
                    .with_timezone(&Utc));
            }
        }

        let time_formats = [
            "%H:%M",    // 18:45
            "%I:%M %p", // 06:45 PM
        ];

        for format in &time_formats {
            if let Ok(time) = NaiveTime::parse_from_str(time_str, format) {
                let today = Local::now().date_naive();
                let datetime = today.and_time(time);
                return Ok(Local
                    .from_local_datetime(&datetime)
                    .single()
                    .expect("Ambiguous local time")
                    .with_timezone(&Utc));
            }
        }

        time_str.parse::<DateTime<Utc>>()
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
}
