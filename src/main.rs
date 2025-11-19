use std::{fs::File, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};

use reading_addiction::{db::Db, pocket::PocketReader, worker::spawn_worker};
use tokio::task::JoinSet;

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
    let (s, r) = async_channel::bounded(64);

    // Spawn a pool of worker tasks for crawling and cleaning.
    let mut workers = JoinSet::new();
    for _ in 0..16 {
        let r_i = r.clone();
        workers.spawn(async move { spawn_worker(r_i) });
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
        }
        None => {}
    }

    // Wait for our full worker pool to finish cleaning up.
    let _report_cards = workers.join_all().await;

    Ok(())
}
