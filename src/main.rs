use std::{fs::File, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};

use reading_addiction::{
    db::{DbActor, DbMessage},
    pocket::PocketReader,
};
use tokio::sync::{mpsc, oneshot};

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
        n: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.unwrap_or(PathBuf::from(DB_NAME.to_string()));

    let (db_actor, rx) = mpsc::channel(32);

    // Spawn our database actor in a generic OS thread so we don't block
    // our crawler tasks when writing a big transaction to SQLite.
    std::thread::spawn(move || {
        DbActor::new(rx, db_path)
            .expect("DB actor is dying because we failed to set up SQLite")
            .run();
    });

    match &cli.command {
        Some(Commands::Pocket { path }) => {
            let f = File::open(path)?;
            let pr = PocketReader::new(f);
            let items = pr.read()?;
            println!("found {} Pocket items", items.len());

            for item in items {
                let (resp_tx, resp_rx) = oneshot::channel();
                let url = item.url.to_string();
                db_actor
                    .send(DbMessage::SaveItem {
                        item,
                        resp: resp_tx,
                    })
                    .await?;
                if resp_rx.await.is_err() {
                    return Err(anyhow!("failed to insert item for {url}..."));
                }
            }
        }
        Some(Commands::Crawl { n }) => {}
        None => {}
    }

    Ok(())
}
