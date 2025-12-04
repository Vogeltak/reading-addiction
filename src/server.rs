//! Web server for the reading addiction service.

use std::sync::Arc;

use askama::Template;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use pulldown_cmark::{Options, Parser, html::push_html};
use reqwest::Url;

use crate::db::{Db, ListItem};

struct HtmlTemplate<T>(T);

impl<T: Template> IntoResponse for HtmlTemplate<T> {
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        }
    }
}

pub type AppState = Arc<Db>;

pub fn router(db: Db) -> Router {
    let state: AppState = Arc::new(db);

    Router::new()
        .route("/", get(index))
        .route("/archived", get(archived))
        .route("/read/{id}", get(article))
        .with_state(state)
}

#[derive(Template)]
#[template(path = "list.html")]
struct ListTemplate {
    title: String,
    heading: String,
    items: Vec<ListItem>,
}

#[derive(Template)]
#[template(path = "article.html")]
struct ArticleTemplate {
    article: crate::db::Article,
    origin: Option<String>,
    time_added: Option<DateTime<Utc>>,
    time_crawled: Option<DateTime<Utc>>,
    tag_list: Vec<String>,
    html_content: String,
}

async fn index(State(db): State<AppState>) -> impl IntoResponse {
    let items = db.get_unread_items().await.unwrap_or_default();

    HtmlTemplate(ListTemplate {
        title: "Reading List".to_string(),
        heading: "Unread articles".to_string(),
        items,
    })
}

async fn archived(State(db): State<AppState>) -> impl IntoResponse {
    let items = db.get_archived_items().await.unwrap_or_default();

    HtmlTemplate(ListTemplate {
        title: "Archive".to_string(),
        heading: "Archived articles".to_string(),
        items,
    })
}

async fn article(State(db): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let article = match db.get_article_by_pub_id(id).await {
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

    let origin = Url::parse(&article.url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()));

    let time_added = DateTime::<Utc>::from_timestamp(article.time_added, 0);

    let time_crawled = article
        .time_last_crawl
        .and_then(|t| DateTime::<Utc>::from_timestamp(t, 0));

    let tag_list: Vec<String> = article
        .tags
        .as_ref()
        .map(|tags| {
            tags.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    HtmlTemplate(ArticleTemplate {
        article,
        origin,
        time_added,
        time_crawled,
        tag_list,
        html_content,
    })
    .into_response()
}
