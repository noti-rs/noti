use std::marker::PhantomData;

use anyhow::Context;
use entity::Entity;
use log::debug;
use rusqlite::Connection;

mod entity;

pub struct Database<T: Entity> {
    connection: rusqlite::Connection,
    _phantom: PhantomData<T>,
}

impl<T: Entity> Database<T> {
    const DB_FILE: &str = "noti.db";

    pub(crate) fn new() -> anyhow::Result<Self> {
        let db_path = Self::get_database_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        debug!("Opening database at: {:?}", db_path);
        let connection = Connection::open(&db_path)?;

        let db = Self {
            connection,
            _phantom: PhantomData,
        };

        db.init()?;

        debug!("Database: Succesfully created");
        Ok(db)
    }

    fn get_database_path() -> anyhow::Result<std::path::PathBuf> {
        shared::paths::xdg_data_dir(Self::DB_FILE)
            .or_else(|| shared::paths::home_data_dir(Self::DB_FILE))
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory path"))
    }

    fn init(&self) -> anyhow::Result<()> {
        debug!("Database: Initializing table");
        self.connection
            .execute(&T::create_table(), [])
            .context("Failed to create table")?;

        Ok(())
    }

    pub fn insert(&self, entity: &T) -> anyhow::Result<i64> {
        let params = entity.to_params();
        let param_placeholders = (1..=params.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "INSERT INTO {} VALUES (NULL, {})",
            T::table_name(),
            param_placeholders
        );

        self.connection
            .execute(&sql, rusqlite::params_from_iter(params))?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn find_all(&self) -> anyhow::Result<Vec<T>> {
        let mut stmt = self
            .connection
            .prepare(&format!("SELECT * FROM {}", T::table_name()))?;

        let entities = stmt
            .query_map([], |row| T::from_row(row))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(entities)
    }

    pub fn find_by_id(&self, id: i64) -> anyhow::Result<Option<T>> {
        let mut stmt = self
            .connection
            .prepare(&format!("SELECT * FROM {} WHERE id = ?1", T::table_name()))?;

        let entity = stmt
            .query_map([id], |row| T::from_row(row))?
            .next()
            .transpose()?;

        Ok(entity)
    }

    pub fn update(&self, entity: &T) -> anyhow::Result<()> {
        let params = entity.to_params();
        let set_clauses = (1..=params.len())
            .map(|i| format!("column{} = ?{}", i, i))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "UPDATE {} SET {} WHERE id = ?{}",
            T::table_name(),
            set_clauses,
            params.len() + 1
        );

        let mut all_params = entity.to_params();
        all_params.push(Box::new(entity.get_id()));

        self.connection
            .execute(&sql, rusqlite::params_from_iter(all_params))?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> anyhow::Result<()> {
        let sql = format!("DELETE FROM {} WHERE id = ?1", T::table_name());

        self.connection.execute(&sql, [id])?;
        Ok(())
    }
}
