use dbus::notification::Notification;
use rusqlite::Row;

pub trait Entity {
    const TABLE_NAME: &str;

    fn table_name() -> String;
    fn create_table() -> String;
    fn get_id(&self) -> i64;
    fn to_params(&self) -> Vec<Box<dyn rusqlite::ToSql>>;
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self>
    where
        Self: Sized;
}

impl Entity for Notification {
    const TABLE_NAME: &str = "notifications";

    fn table_name() -> String {
        Self::TABLE_NAME.to_string()
    }

    fn create_table() -> String {
        format!(
            "CREATE TABLE IF NOT EXISTS {} (
            id               INTEGER PRIMARY KEY,
            replaces_id      INTEGER NOT NULL,
            app_name         TEXT NOT NULL,
            app_icon         TEXT NOT NULL,
            summary          TEXT NOT NULL,
            body             TEXT NOT NULL,
            expire_timeout   TEXT NOT NULL,
            hints            TEXT NOT NULL,
            actions          TEXT NOT NULL,
            is_read          BOOLEAN NOT NULL DEFAULT 0,
            created_at       INTEGER NOT NULL
        )",
            Self::TABLE_NAME
        )
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Notification::try_from(row)?)
    }

    fn to_params(&self) -> Vec<Box<dyn rusqlite::ToSql>> {
        let actions_json = serde_json::to_string(&self.actions).unwrap_or_default();
        let hints_json = serde_json::to_string(&self.hints).unwrap_or_default();
        let body_json = serde_json::to_string(&self.body).unwrap_or_default();
        let expire_timeout_json = serde_json::to_string(&self.expire_timeout).unwrap_or_default();

        vec![
            Box::new(self.id.clone()),
            Box::new(self.app_name.clone()),
            Box::new(self.app_icon.clone()),
            Box::new(self.summary.clone()),
            Box::new(body_json),
            Box::new(expire_timeout_json),
            Box::new(hints_json),
            Box::new(actions_json),
            Box::new(self.is_read),
            Box::new(self.created_at),
        ]
    }

    fn get_id(&self) -> i64 {
        self.id as i64
    }
}
