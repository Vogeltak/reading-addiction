//! Web crawler and parser.

use anyhow::{Result, anyhow};
use async_channel::Receiver;
use dom_smoothie::{Config, Readability, TextMode};
use reqwest::{Client, StatusCode, Url};
use tokio::sync::mpsc;

pub type WorkerInbox = Receiver<WorkItem>;
pub type WorkerOutput = Result<CrawledArticle>;

pub struct WorkItem {
    pub url: Url,
    pub circle_back: mpsc::Sender<WorkerOutput>,
}

#[derive(Debug)]
pub struct CrawledArticle {
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
                .map_err(|e| anyhow!("failed to parse {e:?}"))?;

            Ok(CrawledArticle {
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
