use std::{collections::HashMap, fs::File, iter::zip, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};

use ndarray::{Array1, Array2, Axis};
use reading_addiction::{
    USER_AGENT,
    db::Db,
    pocket::PocketReader,
    server,
    worker::{WorkItem, spawn_worker},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use text_splitter::MarkdownSplitter;
use tokio::{sync::mpsc, task::JoinSet};

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
    /// get latest crawl results as a histogram
    Histogram,
    /// embed articles
    Embed {
        /// how many articles to embed [default: all]
        #[arg(short)]
        n: Option<usize>,
    },
    /// get URLs and their doc embedding vector
    Cluster,
    /// start the web server
    Serve {
        /// port to listen on [default: 3000]
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up our database.
    let db_path = cli.db.unwrap_or(PathBuf::from(DB_NAME.to_string()));
    let db = Db::new(db_path).await?;

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
            // Create channel for distributing work items.
            let (work_q, r) = async_channel::bounded(64);

            // Create an HTTP client that can be shared (internal connection pool).
            let client = Client::builder().user_agent(USER_AGENT).build()?;

            // Spawn a pool of worker tasks for crawling and cleaning.
            let mut workers = JoinSet::new();
            for _ in 0..16 {
                let r_i = r.clone();
                let c_i = client.clone();
                workers.spawn(async move { spawn_worker(c_i, r_i).await });
            }

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

            // Prevent that we keep one sender open!
            drop(results_tx);

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
                        db.save_crawl(article).await?;
                    }
                    Err(err) => eprintln!("Worker error: {err}"),
                }
            }

            // Wait for our full worker pool to finish cleaning up.
            let _report_cards = workers.join_all().await;
        }
        Some(Commands::Histogram) => {
            let hist: HashMap<u16, usize> = db
                .get_crawl_status_hist()
                .await?
                .into_iter()
                .map(|(k, v)| (k.unwrap_or(0), v))
                .collect();

            println!("{}", serde_json::to_string(&hist)?);
        }
        Some(Commands::Embed { n }) => {
            let candidates = db.get_unembedded_items(n).await?;
            println!("Found {} candidates for embedding", candidates.len());

            let api_key = std::env::var("OPENROUTER_API_KEY")?;

            // Create an HTTP client that can be shared (internal connection pool).
            let client = Client::new();

            // Create our semantic chunker for markdown with a high max because
            // we're using our embeddings for clustering and not for retrieval.
            // That's why we can be less precise.
            let splitter = MarkdownSplitter::new(5000);

            // Ugh, okay, don't have the mental capacity right now to do this with concurrent actors.
            // So let's just do it in serial.
            for c in candidates {
                let chunks: Vec<&str> = splitter.chunks(&c.markdown).collect();

                let req = EmbeddingRequest {
                    model: "qwen/qwen3-embedding-8b".to_string(),
                    input: chunks.clone(),
                };

                let res = client
                    .post("https://openrouter.ai/api/v1/embeddings")
                    .header("Authorization", format!("Bearer {}", &api_key))
                    .header("Content-Type", "application/json")
                    .json(&req)
                    .send()
                    .await?;

                println!("{} (OpenRouter) - {}", res.status(), c.url);

                let embedding: EmbeddingResponse =
                    res.json().await.context("failed to parse response")?;

                let mut data = embedding.data;
                data.sort_by_key(|d| d.index);

                for (chunk_text, chunk_data) in zip(chunks, data.clone()) {
                    db.save_chunk_and_embedding(
                        c.url.clone(),
                        chunk_text.to_string(),
                        &chunk_data.embedding,
                    )
                    .await?;
                }

                // Finally, do mean pooling to determine the document embedding.
                let embeddings = data.into_iter().map(|ed| ed.embedding).collect::<Vec<_>>();
                let doc_vector = mean_pooling_ndarray(&embeddings)?.to_vec();

                db.save_doc_vector(c.url, &doc_vector).await?;
            }
        }
        Some(Commands::Cluster) => {
            let items = db.get_urls_with_doc_vector().await?;
            println!("{}", serde_json::to_string(&items)?);
        }
        Some(Commands::Serve { port }) => {
            let app = server::router(db);
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            println!("Listening on http://{}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
        None => {}
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest<'a> {
    model: String,
    input: Vec<&'a str>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

fn mean_pooling_ndarray(embeddings: &[Vec<f32>]) -> Result<Array1<f32>> {
    if embeddings.is_empty() {
        return Err(anyhow!("No embeddings provided"));
    }

    let rows = embeddings.len();
    let cols = embeddings[0].len();

    // Flatten the Vec<Vec<f32>> into a single Vec to create an Array2
    let flat_data: Vec<f32> = embeddings.iter().flatten().cloned().collect();

    // Create a 2D Matrix (Rows = Chunks, Cols = Dimensions)
    let matrix = Array2::from_shape_vec((rows, cols), flat_data)?;

    // Calculate mean along Axis 0 (collapsing rows down to one)
    // This returns an Array1<f32>
    let mean_vector = matrix
        .mean_axis(Axis(0))
        .ok_or(anyhow!("Calculation failed"))?;

    Ok(mean_vector)
}
