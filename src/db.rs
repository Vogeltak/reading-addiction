//! Data store actor.

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use reqwest::Url;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use tokio_rusqlite::Connection;

use crate::{pocket::PocketItem, worker::CrawledArticle};

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
                   http_status_last_crawl INTEGER,
                   html TEXT,
                   markdown TEXT,
                   doc_vector BLOB
                );
                CREATE TABLE IF NOT EXISTS chunks (
                    id INTEGER PRIMARY KEY,
                    url TEXT NOT NULL,
                    chunk TEXT NOT NULL,
                    vector BLOB NOT NULL
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

    pub async fn get_uncrawled_items(&self, limit: Option<usize>) -> Result<Vec<ItemHandle>> {
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
            .map(|url| ItemHandle { url })
            .collect();

        Ok(items)
    }

    pub async fn save_crawl(&self, crawl: CrawledArticle) -> Result<()> {
        let _ = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE items
                    SET time_last_crawl = ?, http_status_last_crawl = ?, html = ?, markdown = ?
                    WHERE url = ?",
                    params![
                        crawl.timestamp,
                        crawl.status.as_u16(),
                        crawl.html,
                        crawl.markdown,
                        crawl.url.to_string()
                    ],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn get_crawl_status_hist(&self) -> Result<HashMap<Option<u16>, usize>> {
        let status_codes: Vec<Option<u16>> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT http_status_last_crawl FROM items")?;
                stmt.query_map([], |row| row.get::<_, Option<u16>>(0))?
                    .collect()
            })
            .await?;

        let mut hist = HashMap::new();

        for code in status_codes {
            *hist.entry(code).or_insert(0) += 1;
        }

        Ok(hist)
    }

    pub async fn get_unembedded_items(&self, limit: Option<usize>) -> Result<Vec<ItemForChunking>> {
        let items: Vec<(String, String)> = self
            .conn
            .call(move |conn| {
                let sql = match limit {
                    Some(n) => format!("SELECT url, markdown FROM items WHERE doc_vector IS NULL AND markdown IS NOT NULL LIMIT {n}"),
                    None => "SELECT url, markdown FROM items WHERE doc_vector IS NULL AND markdown IS NOT NULL".to_string(),
                };

                let mut stmt = conn.prepare(&sql)?;

                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect()
            })
            .await?;

        let items = items
            .into_iter()
            .filter_map(|(u, markdown)| {
                Url::parse(&u)
                    .ok()
                    .map(|url| ItemForChunking { url, markdown })
            })
            .collect();

        Ok(items)
    }

    pub async fn save_chunk_and_embedding(
        &self,
        url: Url,
        chunk: String,
        vector: &[f32],
    ) -> Result<()> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();

        let _ = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO chunks (url, chunk, vector) VALUES (?1, ?2, ?3)",
                    params![url.to_string(), chunk, bytes],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn save_doc_vector(&self, url: Url, doc_vector: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = doc_vector.iter().flat_map(|f| f.to_le_bytes()).collect();

        let _ = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE items
                    SET doc_vector = ?
                    WHERE url = ?",
                    params![bytes, url.to_string()],
                )
            })
            .await?;

        Ok(())
    }

    pub async fn get_unread_items(&self) -> Result<Vec<ListItem>> {
        let items: Vec<(String, String, Option<usize>)> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT url, title, LENGTH(markdown) FROM items WHERE status = 'unread' ORDER BY time_added DESC",
                )?;
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect()
            })
            .await?;

        let items = items
            .into_iter()
            .map(|(url, title, markdown_len)| ListItem {
                url,
                title,
                markdown_len,
            })
            .collect();

        Ok(items)
    }

    pub async fn get_archived_items(&self) -> Result<Vec<ListItem>> {
        let items: Vec<(String, String, Option<usize>)> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT url, title, LENGTH(markdown) FROM items WHERE status = 'archive' ORDER BY time_added DESC",
                )?;
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect()
            })
            .await?;

        let items = items
            .into_iter()
            .map(|(url, title, markdown_len)| ListItem {
                url,
                title,
                markdown_len,
            })
            .collect();

        Ok(items)
    }

    pub async fn get_urls_with_doc_vector(&self) -> Result<Vec<UrlWithDocVector>> {
        let items: Vec<(String, Vec<u8>)> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT url, doc_vector FROM items WHERE markdown IS NOT NULL AND doc_vector IS NOT NULL")?;
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect()
            })
            .await?;

        let items = items
            .into_iter()
            .map(|(url, vector)| {
                let vector = vector
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect();

                UrlWithDocVector { url, vector }
            })
            .collect();

        Ok(items)
    }

    pub async fn get_article_by_url(&self, url: String) -> Result<Option<Article>> {
        let article: Option<(String, String, Option<String>)> = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT url, title, markdown FROM items WHERE url = ?")?;
                stmt.query_row([&url], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                    .optional()
            })
            .await?;

        Ok(article.map(|(url, title, markdown)| Article {
            url,
            title,
            markdown,
        }))
    }
}

#[derive(Debug)]
pub struct ItemHandle {
    pub url: Url,
}

#[derive(Debug)]
pub struct ItemForChunking {
    pub url: Url,
    pub markdown: String,
}

#[derive(Debug, Serialize)]
pub struct UrlWithDocVector {
    pub url: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub url: String,
    pub title: String,
    pub markdown_len: Option<usize>,
}

impl ListItem {
    pub fn content_status(&self) -> ContentStatus {
        match self.markdown_len {
            None => ContentStatus::None,
            Some(len) if len < 1000 => ContentStatus::Short,
            Some(_) => ContentStatus::Good,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContentStatus {
    None,
    Short,
    Good,
}

impl ContentStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            ContentStatus::None => "○",
            ContentStatus::Short => "◐",
            ContentStatus::Good => "●",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            ContentStatus::None => "status-none",
            ContentStatus::Short => "status-short",
            ContentStatus::Good => "status-good",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub markdown: Option<String>,
}
