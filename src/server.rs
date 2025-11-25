//! Web server for the reading addiction service.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use pulldown_cmark::{Options, Parser, html::push_html};
use serde::Deserialize;

use crate::db::Db;

pub type AppState = Arc<Db>;

pub fn router(db: Db) -> Router {
    let state: AppState = Arc::new(db);

    Router::new()
        .route("/", get(index))
        .route("/article", get(article))
        .with_state(state)
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
                            a href=(format!("/article?url={}", urlencoding::encode(&item.url))) {
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

#[derive(Deserialize)]
struct ArticleQuery {
    url: String,
}

async fn article(
    State(db): State<AppState>,
    Query(query): Query<ArticleQuery>,
) -> impl IntoResponse {
    let article = match db.get_article_by_url(query.url.clone()).await {
        Ok(Some(article)) => article,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Article not found".to_string()).into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            )
                .into_response();
        }
    };

    let html_content = match &article.markdown {
        Some(md) => {
            let parser = Parser::new_ext(md, Options::all());
            let mut html_output = String::new();
            push_html(&mut html_output, parser);
            html_output
        }
        None => "<p>Article content not available.</p>".to_string(),
    };

    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (&article.title) }
                style {
                    "body { font-family: serif; max-width: 800px; margin: 2rem auto; padding: 0 1rem; font-size: 18px; line-height: 1.6; background: #faf9f5; }
                     h1 { margin-bottom: 0.5rem; }
                     .meta { color: #666; font-size: 0.9rem; margin-bottom: 2rem; }
                     .meta a { color: #666; }
                     .back { margin-bottom: 1rem; }
                     img { max-width: 100%; height: auto; }
                     pre { overflow-x: auto; background: #f0ede5; padding: 1rem; }
                     code { background: #f0ede5; padding: 0.1rem 0.3rem; font-size: 16px; }
                     pre code { background: none; padding: 0; }
                     blockquote { border-left: 3px solid #ccc; margin-left: 0; padding-left: 1rem; color: #555; }"
                }
            }
            body {
                p class="back" { a href="/" { "← Back to list" } }
                h1 { (&article.title) }
                p class="meta" {
                    a href=(&article.url) target="_blank" { "View original" }
                }
                article {
                    (PreEscaped(html_content))
                }
            }
        }
    }.into_response()
}
