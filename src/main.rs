//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

mod render;
mod page;
mod metadata;
mod auth;
mod error_message;

use std::path::{Component, Path, PathBuf};
use axum::{debug_handler, routing::get, Form, Router};
use axum::extract;
use axum::response::Redirect;
use tower_http::{
    services::ServeDir,
    trace::TraceLayer,
};
use serde::Deserialize;
use crate::auth::Access;
use crate::error_message::ErrorMessage;
use crate::metadata::Metadata;
use crate::page::{RawPage, RenderedPage};
use crate::render::Renderer;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct PagePathset {
    pub content: PathBuf
}

#[derive(Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum Mode {
    Read,
    Edit,
    Create,
}

#[derive(Deserialize)]
struct PageQuery {
    pub mode: Option<Mode>,
}

#[derive(Deserialize)]
struct EditForm {
    pub title: String,
    pub view_access: Access,
    pub edit_access: Access,
    pub cmark: String,
}

impl Default for PageQuery {
    fn default() -> Self { PageQuery { mode: Option::from(Mode::Read) }}
}

#[tokio::main]
async fn main() {
    // build our application with a route
    let router = Router::new()
        .route("/{*path}", get(get_handler).post(post_content_handler))
        .nest_service("/assets", ServeDir::new("assets"));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

#[debug_handler]
async fn get_handler(extract::Path(path): extract::Path<String>, mode: extract::Query<PageQuery>) -> RenderedPage {
    let renderer = Renderer::new("templates/**/*").unwrap();
    let pathset = match get_paths(&path) {
        None => return RenderedPage::not_found(renderer.render_error(&ErrorMessage::not_found(&path))),
        Some(paths) => paths
    };

    let template = match mode.mode.unwrap_or(Mode::Read) {
        Mode::Read => "page.html",
        Mode::Edit | Mode::Create => "page_edit.html",
    };

    match RawPage::read_from_path(&pathset.content).await {
        Ok(raw) => match renderer.render_page(&raw, template) {
            Ok(html) => RenderedPage::ok(raw.metadata, html),
            Err(err) => RenderedPage::internal_error(renderer.render_error(&err.into()))
        },
        Err(err) => {
            let err = ErrorMessage::from(err);
            RenderedPage::error(&err, renderer.render_error(&err))
        }
    }
}

#[debug_handler]
async fn post_content_handler(extract::Path(path): extract::Path<String>, form: Form<EditForm>) -> Result<Redirect, RenderedPage> {
    let renderer = Renderer::new("templates/**/*").unwrap();
    let pathset = match get_paths(&path) {
        None => return Err(RenderedPage::not_found(renderer.render_error(&ErrorMessage::not_found(path.as_str())))),
        Some(paths) => paths
    };

    let metadata = Metadata {
        title: form.title.clone(),
        view_access: form.view_access.clone(),
        edit_access: form.edit_access.clone(),
    };

    let raw_page = RawPage {
        metadata,
        markdown: form.cmark.clone(),
    };

    match raw_page.write_to_path(&pathset.content).await {
        Ok(_) => Ok(Redirect::to(&path)),
        Err(err) => {
            let err = ErrorMessage::from(err);
            Err(RenderedPage::error(&err, renderer.render_error(&err)))
        },
    }
}

fn get_paths(path: &str) -> Option<PagePathset> {
    let relative = match validate_path(&path) {
        None => return None,
        Some(relative) => relative,
    };

    let base = Path::new("pages").join(&relative);
    let content = base.with_extension("md");

    Some(PagePathset { content })
}

// Simplified version of tower's build_and_validate_path
// https://github.com/tower-rs/tower-http/blob/075479b852f348c8b74245f478b9012090acf5fc/tower-http/src/services/fs/serve_dir/mod.rs#L453
fn validate_path(requested_path: &str) -> Option<PathBuf> {
    let path = requested_path.trim_start_matches('/');
    let path = Path::new(&*path);

    for component in path.components() {
        match component {
            Component::Normal(comp) => {
                // protect against paths like `/foo/c:/bar/baz`
                if Path::new(&comp)
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
                {
                    return None;
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir => {
                return None;
            }
        }
    }

    Some(path.to_path_buf())
}
