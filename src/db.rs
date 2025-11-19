//! Data store actor.

use std::path::PathBuf;

use anyhow::Result;
use reqwest::Url;
use rusqlite::params;
use tokio_rusqlite::Connection;

use crate::pocket::PocketItem;

/// Data store backed by SQLite.
pub struct Db {
    conn: Connection,
}

impl Db {
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path).await?;

        // I guess we're doing our migrations in line now with rusqlite?
        conn.call(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS items (
               url TEXT PRIMARY KEY,
               title TEXT NOT NULL,
               time_added INTEGER NOT NULL,
               tags TEXT,
               status TEXT NOT NULL,
               time_last_crawl INTEGER,
               html TEXT,
               markdown TEXT
            );",
            )
        })
        .await?;

        Ok(Self { conn })
    }

    pub async fn save_item(&self, item: PocketItem) -> Result<()> {
        let _ = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO items (url, title, time_added, tags, status)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(url) DO UPDATE SET
                        title=excluded.title,
                        tags=excluded.tags,
                        status=excluded.status",
                    params![
                        item.url.to_string(),
                        item.title,
                        item.time_added,
                        item.tags.to_string(),
                        item.status.to_string(),
                    ],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn get_uncrawled_items(&self, limit: Option<usize>) -> Result<Vec<UncrawledItem>> {
        let items: Vec<String> = self
            .conn
            .call(move |conn| {
                let sql = match limit {
                    Some(n) => format!("SELECT url FROM items WHERE html IS NULL LIMIT {n}"),
                    None => "SELECT url FROM items WHERE html IS NULL".to_string(),
                };

                let mut stmt = conn.prepare(&sql)?;

                stmt.query_map([], |row| row.get(0))?.collect()
            })
            .await?;

        let items = items
            .iter()
            .filter_map(|s| Url::parse(s).ok())
            .map(|url| UncrawledItem { url })
            .collect();

        Ok(items)
    }
}

#[derive(Debug)]
pub struct UncrawledItem {
    pub url: Url,
}
