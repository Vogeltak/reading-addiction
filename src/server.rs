//! Web server for the reading addiction service.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use pulldown_cmark::{Options, Parser, html::push_html};
use reqwest::Url;
use serde::Deserialize;

use crate::db::Db;

pub type AppState = Arc<Db>;

pub fn router(db: Db) -> Router {
    let state: AppState = Arc::new(db);

    Router::new()
        .route("/", get(index))
        .route("/archived", get(archived))
        .route("/article", get(article))
        .with_state(state)
}

fn list_page_styles() -> &'static str {
    "body { font-family: serif; max-width: 1200px; margin: 2rem auto; padding: 0 1rem; font-size: 18px; background: #faf9f5; }
     h1 { padding-bottom: 0.5rem; }
     ul { list-style: none; padding: 0; }
     li { padding: 0.3rem 0; }
     a:hover { background: #e9e6da; }
     .count { color: #666; font-size: 0.9rem; }
     .status { margin-right: 0.4rem; }
     .status-none { color: #cf222e; }
     .status-short { color: #c6613f; }
     .status-good { color: #67c23a; display: none; }
     nav { margin-bottom: 1rem; }
     nav a { margin-right: 1rem; }
     @media (min-width: 768px) {
       ul { columns: 2; column-gap: 2rem; }
       li { break-inside: avoid; }
     }"
}

use crate::db::ListItem;

fn render_item_list(items: &[ListItem]) -> Markup {
    html! {
        ul {
            @for item in items {
                @let status = item.content_status();
                li {
                    span class=(format!("status {}", status.css_class())) { (status.icon()) }
                    a href=(format!("/article?id={}", &item.pub_id)) {
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

async fn index(State(db): State<AppState>) -> Markup {
    let items = db.get_unread_items().await.unwrap_or_default();

    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Reading List" }
                style { (list_page_styles()) }
            }
            body {
                nav {
                    a href="/" { "Unread" }
                    a href="/archived" { "Archived" }
                }
                h1 { "Unread Articles" }
                p class="count" { (items.len()) " articles" }
                (render_item_list(&items))
            }
        }
    }
}

async fn archived(State(db): State<AppState>) -> Markup {
    let items = db.get_archived_items().await.unwrap_or_default();

    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Archived - Reading List" }
                style { (list_page_styles()) }
            }
            body {
                nav {
                    a href="/" { "Unread" }
                    a href="/archived" { "Archived" }
                }
                h1 { "Archived Articles" }
                p class="count" { (items.len()) " articles" }
                (render_item_list(&items))
            }
        }
    }
}

#[derive(Deserialize)]
struct ArticleQuery {
    id: String,
}

async fn article(
    State(db): State<AppState>,
    Query(query): Query<ArticleQuery>,
) -> impl IntoResponse {
    let article = match db.get_article_by_pub_id(query.id.clone()).await {
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
                    "body { font-family: serif; max-width: 80ch; margin: 2rem auto; padding: 0 1rem; font-size: 18px; line-height: 1.6; background: #faf9f5; }
                     h1 { font-size: 1.6rem; margin-bottom: 0.5rem; }
                     h2 { font-size: 1.4rem; }
                     hr { border: 1px dashed; }
                     .meta { background: #f0eee6; color: #666; font-size: 0.9rem; border-radius: 16px; padding: 1px 1rem; }
                     .meta a { color: #666; }
                     .origin { font-weight: bold; }
                     .tag { background-color: #e1dac2; padding: 2px 4px; color: #333; }
                     .back { margin-bottom: 1rem; }
                     img { max-width: 100%; height: auto; }
                     pre { overflow-x: auto; background: #f0ede5; padding: 1rem; border: 2px dashed black; }
                     code { background: #f0ede5; padding: 0.1rem 0.3rem; font-size: 16px; }
                     pre code { background: none; padding: 0; }
                     blockquote { border-left: 3px solid #ccc; margin-left: 0; padding-left: 1rem; color: #555; }"
                }
            }
            body {
                p class="back" { a href="/" { "← Back to list" } }
                div class="meta" {
                    p {
                        @let origin = Url::parse(&article.url).ok().and_then(|u| u.host_str().map(|s| s.to_string()));
                        @if let Some(host) = origin {
                            span class="origin" { (host) }
                            span class="separator" { " · " }
                        }
                        a class="original-link" href=(&article.url) target="_blank" { "View original" }
                        span class="separator" { " · " }
                        span class="status" { (&article.status) }
                    }
                    p {
                        @let added = DateTime::<Utc>::from_timestamp(article.time_added, 0);
                        @if let Some(dt) = added {
                            span class="time-added" {
                                span class="label" { "Saved on " }
                                span class="value" { (dt.format("%Y-%m-%d %H:%M")) }
                            }
                        }
                        @if let Some(crawl_time) = article.time_last_crawl {
                            @let crawled = DateTime::<Utc>::from_timestamp(crawl_time, 0);
                            @if let Some(dt) = crawled {
                                span class="separator" { " · " }
                                span class="time-crawled" {
                                    span class="label" { "Last crawled on " }
                                    span class="value" { (dt.format("%Y-%m-%d %H:%M")) }
                                }
                            }
                        }
                        @if let Some(status) = article.http_status_last_crawl {
                            span class="http-status" {
                                " ("
                                span class="value" { (status) }
                                ")"
                            }
                        }
                    }
                    @if let Some(tags) = &article.tags {
                        p {
                            @let tag_list: Vec<&str> = tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
                            @if !tag_list.is_empty() {
                                span class="tags" {
                                    @for (i, tag) in tag_list.iter().enumerate() {
                                        span class="tag" { (tag) }
                                        @if i < tag_list.len() - 1 {
                                            " "
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                h1 { (&article.title) }
                article {
                    (PreEscaped(html_content))
                }
            }
        }
    }.into_response()
}
