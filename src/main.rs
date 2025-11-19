use std::{fs::File, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};

use reading_addiction::{
    db::Db,
    pocket::PocketReader,
    worker::{WorkItem, spawn_worker},
};
use reqwest::Client;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

const DB_NAME: &str = "addiction.db";

/// Interact with the reading addiction project.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Path to the database [default: addiction.db]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// parses a Pocket CSV export
    Pocket {
        /// file path for the Pocket export CSV
        path: PathBuf,
    },
    /// starts crawl for all items that don't have html yet
    Crawl {
        /// how many uncrawled items to process [default: all]
        #[arg(short)]
        n: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up our database.
    let db_path = cli.db.unwrap_or(PathBuf::from(DB_NAME.to_string()));
    let db = Db::new(db_path).await?;

    // Create channel for distributing work items.
    let (work_q, r) = async_channel::bounded(64);

    // Create an HTTP client that can be shared (internal connection pool).
    let client = Client::new();

    // Spawn a pool of worker tasks for crawling and cleaning.
    let mut workers = JoinSet::new();
    for _ in 0..16 {
        let r_i = r.clone();
        let c_i = client.clone();
        workers.spawn(async move { spawn_worker(c_i, r_i).await });
    }

    // Do what was asked.
    match cli.command {
        Some(Commands::Pocket { path }) => {
            let f = File::open(path)?;
            let pr = PocketReader::new(f);
            let items = pr.read()?;
            println!("found {} Pocket items", items.len());

            for item in items {
                let url = item.url.to_string();
                if db.save_item(item).await.is_err() {
                    return Err(anyhow!("failed to insert item for {url}..."));
                }
            }
        }
        Some(Commands::Crawl { n }) => {
            let candidates = db.get_uncrawled_items(n).await?;
            println!("Found {} candidates for crawling", candidates.len());

            // Results channel for work output
            let (results_tx, mut results_rx) = mpsc::channel(64);

            let worker_tx = results_tx.clone();

            // Spawn a Seeder task so we can start consuming results while
            // we're still pushing work on the queue.
            tokio::spawn(async move {
                for c in candidates {
                    let _ = work_q
                        .send(WorkItem {
                            url: c.url,
                            circle_back: worker_tx.clone(),
                        })
                        .await;
                }
            });

            println!("hello");

            // Prevent that we keep one sender open!
            drop(results_tx);

            while let Some(worker_output) = results_rx.recv().await {
                match worker_output {
                    Ok(article) => {
                        // Update our database with the extracted content
                        // TODO: db.store_crawl
                        println!("{}", article.markdown);
                    }
                    Err(err) => eprintln!("Error: {err}"),
                }
            }
        }
        None => {}
    }

    // Wait for our full worker pool to finish cleaning up.
    let _report_cards = workers.join_all().await;

    Ok(())
}
