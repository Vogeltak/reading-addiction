//! Web crawler and parser.
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use async_channel::Receiver;
use dom_smoothie::{Config, Readability, TextMode};
use reqwest::{Client, StatusCode, Url};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::db::Db;

pub static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " bot"
);

pub struct Crawler {
    db: Db,
    client: reqwest::Client,
}

impl Crawler {
    pub fn new(db: Db) -> Result<Self> {
        // Create an HTTP client that can be shared (internal connection pool).
        let client = Client::builder().user_agent(USER_AGENT).build()?;

        Ok(Self { db, client })
    }

    pub async fn crawl_batch(&self, limit: Option<usize>) -> Result<()> {
        // Create channel for distributing work items.
        let (work_q, r) = async_channel::bounded(64);

        // Spawn a pool of worker tasks for crawling and cleaning.
        let mut workers = JoinSet::new();
        for _ in 0..16 {
            let r_i = r.clone();
            let c_i = self.client.clone();
            workers.spawn(async move { spawn_worker(c_i, r_i).await });
        }

        let candidates = self.db.get_uncrawled_items(limit).await?;
        println!("Found {} candidates for crawling", candidates.len());

        // Results channel for work output
        let (results_tx, mut results_rx) = mpsc::channel(64);

        // Spawn a Seeder task so we can start consuming results while
        // we're still pushing work on the queue.
        tokio::spawn(async move {
            for c in candidates {
                let _ = work_q
                    .send(WorkItem {
                        url: c.url,
                        circle_back: results_tx.clone(),
                    })
                    .await;
            }
        });

        while let Some(worker_output) = results_rx.recv().await {
            match worker_output {
                Ok(article) => {
                    // Update our database with the extracted content
                    println!(
                        "{} - {} {} bytes of text, ~{} tokens",
                        article.status,
                        article.url,
                        article.markdown.len(),
                        article.markdown.len() / 4
                    );
                    self.db.save_crawl(article).await?;
                }
                Err(err) => eprintln!("Worker error: {err}"),
            }
        }

        // Wait for our full worker pool to finish cleaning up.
        let _report_cards = workers.join_all().await;

        Ok(())
    }
}

pub type WorkerInbox = Receiver<WorkItem>;
pub type WorkerOutput = Result<CrawledArticle>;

pub struct WorkItem {
    pub url: Url,
    pub circle_back: mpsc::Sender<WorkerOutput>,
}

#[derive(Debug)]
pub struct CrawledArticle {
    pub timestamp: u64,
    pub status: StatusCode,
    pub url: Url,
    pub html: String,
    pub markdown: String,
}

pub async fn spawn_worker(client: Client, inbox: WorkerInbox) {
    // Readability config
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };

    while let Ok(work) = inbox.recv().await {
        // Fetch the website's content.
        let Ok(res) = client.get(work.url.clone()).send().await else {
            let _ = work
                .circle_back
                .send(Err(anyhow!("failed to fetch {}", work.url)))
                .await;
            continue;
        };

        let status_code = res.status();

        // Decode response as html.
        let Ok(html) = res.text().await else {
            let _ = work
                .circle_back
                .send(Err(anyhow!("failed to decode response from {}", work.url)))
                .await;
            continue;
        };

        // Do Readability magic. Needs to be blocking because [`Tendril`]s are !Send.
        let url2 = work.url.clone();
        let cfg2 = cfg.clone();
        let extraction_result = tokio::task::spawn_blocking(move || {
            let article = Readability::new(html, Some(url2.as_str()), Some(cfg2))
                .unwrap()
                .parse()
                .map_err(|e| anyhow!("failed to parse {}: {e:?}", url2))?;

            Ok(CrawledArticle {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time > unix epoch")
                    .as_secs(),
                status: status_code,
                url: url2.clone(),
                html: article.content.to_string(),
                markdown: article.text_content.to_string(),
            })
        })
        .await;

        // Send back HTML and extracted markdown content.
        match extraction_result {
            Ok(Ok(article)) => {
                let _ = work.circle_back.send(Ok(article)).await;
            }
            Ok(Err(e)) => {
                let _ = work.circle_back.send(Err(e)).await;
            }
            Err(_) => {
                // Blocking thread panicked
                let _ = work
                    .circle_back
                    .send(Err(anyhow!("dom_smoothie parser panicked on {}", work.url)))
                    .await;
            }
        }
    }
}
