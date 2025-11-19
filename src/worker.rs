//! Web crawler and parser.

use anyhow::{Result, anyhow};
use async_channel::Receiver;
use dom_smoothie::{Config, Readability, TextMode};
use reqwest::{Client, Url};
use tokio::sync::mpsc;

pub type WorkerInbox = Receiver<WorkItem>;
pub type WorkerOutput = Result<ExtractedArticle>;

pub struct WorkItem {
    pub url: Url,
    pub circle_back: mpsc::Sender<WorkerOutput>,
}

#[derive(Debug)]
pub struct ExtractedArticle {
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
        println!("received work item for {}", work.url);

        // Fetch the website's content.
        let Ok(res) = client.get(work.url.clone()).send().await else {
            let _ = work
                .circle_back
                .send(Err(anyhow!("failed to fetch {}", work.url)));
            continue;
        };

        // Decode response as html.
        let Ok(html) = res.text().await else {
            let _ = work
                .circle_back
                .send(Err(anyhow!("failed to decode response from {}", work.url)));
            continue;
        };

        // Do Readability magic.
        let Ok(article) = Readability::new(html, Some(work.url.as_str()), Some(cfg.clone()))
            .unwrap()
            .parse()
        else {
            let _ = work
                .circle_back
                .send(Err(anyhow!("failed to parse {}", work.url)));
            continue;
        };

        // Send back HTML and extracted markdown content.
        let _ = work.circle_back.send(Ok(ExtractedArticle {
            url: work.url.clone(),
            html: article.content.to_string(),
            markdown: article.text_content.to_string(),
        }));
    }
}
