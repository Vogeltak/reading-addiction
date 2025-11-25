//! Web server for the reading addiction service.

use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use maud::{DOCTYPE, Markup, html};

use crate::db::Db;

pub type AppState = Arc<Db>;

pub fn router(db: Db) -> Router {
    let state: AppState = Arc::new(db);

    Router::new().route("/", get(index)).with_state(state)
}

async fn index(State(db): State<AppState>) -> Markup {
    let items = db.get_unread_items().await.unwrap_or_default();

    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Reading List" }
                style {
                    "body { font-family: serif; max-width: 1200px; margin: 2rem auto; padding: 0 1rem; font-size: 18px; background: #faf9f5; }
                     h1 { padding-bottom: 0.5rem; }
                     ul { list-style: none; padding: 0; }
                     li { padding: 0.3rem 0; }
                     a:hover { background: #e9e6da; }
                     .count { color: #666; font-size: 0.9rem; }
                     @media (min-width: 768px) {
                       ul { columns: 2; column-gap: 2rem; }
                       li { break-inside: avoid; }
                     }"
                }
            }
            body {
                h1 { "Unread Articles" }
                p class="count" { (items.len()) " articles" }
                ul {
                    @for item in &items {
                        li {
                            a href=(item.url) target="_blank" {
                                @if item.title.is_empty() {
                                    (item.url)
                                } @else {
                                    (item.title)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
