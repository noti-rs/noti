use chrono::{DateTime, Utc};
use dbus::notification::ScheduledNotification;
use humantime::parse_duration;
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
        let time = match Self::parse_time(&notification.time) {
            Ok(parsed_time) => parsed_time,
            Err(e) => {
                eprintln!("Error parsing time '{}': {}", notification.time, e);
                return;
            }
        };

        let scheduled_notification = ScheduledNotification {
            time: time.to_rfc3339(),
            data: notification.data,
            id: notification.id,
        };

        self.queue.push(Reverse(scheduled_notification));
    }

    fn parse_time(time_str: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
        let now = Utc::now();

        if let Ok(duration) = parse_duration(time_str) {
            Ok(now + chrono::Duration::from_std(duration).unwrap())
        } else {
            time_str.parse::<DateTime<Utc>>()
        }
    }

    pub fn pop_due_notifications(&mut self) -> Vec<ScheduledNotification> {
        let now = Utc::now();
        let mut due_notifications = Vec::new();

        while let Some(Reverse(top)) = self.queue.peek() {
            if top.time.parse::<DateTime<Utc>>().unwrap() <= now {
                due_notifications.push(self.queue.pop().unwrap().0);
            } else {
                break;
            }
        }

        due_notifications
    }
}
