use dbus::notification::Notification;
use log::debug;

use crate::db::Database;

pub struct HistoryManager {
    db: Database<Notification>,
}

impl HistoryManager {
    pub fn init() -> anyhow::Result<Self> {
        let db = Database::<Notification>::new()?;
        let hm = Self { db };

        debug!("History Manager: Initialized");
        Ok(hm)
    }

    pub fn push(&self, notification: &Notification) -> anyhow::Result<()> {
        self.db.insert(notification)?;

        Ok(())
    }

    pub fn find_all(&self) -> anyhow::Result<Vec<Notification>> {
        self.db.find_all()
    }

    pub fn find(&self, id: i64) -> anyhow::Result<Option<Notification>> {
        self.db.find_by_id(id)
    }

    pub fn delete(&self, id: i64) -> anyhow::Result<()> {
        self.db.delete(id)
    }
}
