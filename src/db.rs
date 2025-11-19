//! Data store actor.

use std::path::PathBuf;

use anyhow::Result;
use reqwest::Url;
use rusqlite::Connection;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::pocket::{PocketItem, PocketStatus, Tag};

/// Data store backed by SQLite.
pub struct DbActor {
    recv: mpsc::Receiver<DbMessage>,
    conn: Connection,
}

impl DbActor {
    pub fn new(recv: mpsc::Receiver<DbMessage>, db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        // I guess we're doing our migrations in line now with rusqlite?
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
        )?;

        Ok(Self { recv, conn })
    }

    pub fn run(mut self) {
        while let Some(msg) = self.recv.blocking_recv() {
            match msg {
                DbMessage::SaveItem { item, resp } => {
                    let res = self.conn.execute(
                        "INSERT INTO items (url, title, time_added, tags, status) VALUES (?1, ?2, ?3, ?4, ?5)",
                        (item.url.to_string(), item.title, item.time_added, item.tags.to_string(), item.status.to_string())
                    ).inspect_err(|err| eprintln!("{err:?}"));
                    let _ = resp.send(res.map(|_| ()));
                }
                DbMessage::GetUncrawledItems { resp } => {
                    let mut stmt = self
                        .conn
                        .prepare("SELECT url FROM items WHERE html IS NULL")
                        .expect("sql call should succeed");
                    let items = stmt
                        .query_map([], |row| {
                            let url_text: String =
                                row.get(0).expect("returned row should have url column");
                            Ok(UncrawledItem {
                                url: Url::parse(&url_text)
                                    .expect("url from database should be parseable as Url"),
                            })
                        })
                        .unwrap()
                        .collect();
                    let _ = resp.send(items);
                }
            }
        }
    }
}

pub enum DbMessage {
    SaveItem {
        item: PocketItem,
        resp: oneshot::Sender<rusqlite::Result<()>>,
    },
    GetUncrawledItems {
        resp: oneshot::Sender<rusqlite::Result<Vec<UncrawledItem>>>,
    },
}

#[derive(Debug)]
pub struct UncrawledItem {
    pub url: Url,
}
