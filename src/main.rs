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
use std::sync::Arc;
use axum::{debug_handler, routing::get, Form, Router};
use axum::extract;
use axum::extract::State;
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

#[derive(Clone)]
struct AppState {
    pub renderer: Arc<Renderer>,
    pub pages_dir: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PagePathset {
    pub url: String,
    pub md: PathBuf,
}

#[derive(Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum Mode {
    Read,
    Edit,
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

#[derive(Deserialize)]
struct CreateForm {
    pub path: String,
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
    // TODO: Read these from a config file
    let pages_dir = "pages/";
    let assets_dir = "assets/";
    let bind_address = "127.0.0.1:8080";
    let state = AppState {
        renderer: Renderer::new("templates/**/*").unwrap().into(),
        pages_dir: PathBuf::from(pages_dir),
    };

    // build our application with a route
    let router = Router::new()
        .route("/special:create", get(get_create_handler).post(post_create_handler))
        .route("/{*path}", get(get_page_handler).post(post_edit_handler))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .with_state(state);

    // run it
    let listener = tokio::net::TcpListener::bind(bind_address)
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

#[debug_handler]
async fn get_page_handler(State(state): State<AppState>, extract::Path(path): extract::Path<String>, query: extract::Query<PageQuery>) -> RenderedPage {
    let pathset = match get_paths(&state.pages_dir, &path) {
        None => return render_error(state, ErrorMessage::not_found(&path)),
        Some(paths) => paths
    };

    let mode = query.mode.unwrap_or(Mode::Read);

    let template = match mode {
        Mode::Read => "page.tera",
        Mode::Edit => "page_edit.tera",
    };

    let raw = match RawPage::read_from_path(&pathset.md).await {
        Ok(raw) => raw,
        Err(err) => {
            let err = ErrorMessage::from(err);
            return RenderedPage::error(&err, state.renderer.render_error(&err))
        }
    };

    render_page(state, raw, template)
}

#[debug_handler]
async fn post_edit_handler(State(state): State<AppState>, extract::Path(path): extract::Path<String>, form: Form<EditForm>) -> Result<Redirect, RenderedPage> {
    let pathset = match get_paths(&state.pages_dir, &path) {
        None => return Err(render_error(state, ErrorMessage::not_found(&path))),
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

    match raw_page.write_to_path(&pathset.md).await {
        Ok(_) => Ok(Redirect::to(&pathset.url)),
        Err(err) => {
            let err = ErrorMessage::from(err);
            Err(render_error(state, ErrorMessage::from(err)))
        },
    }
}

#[debug_handler]
async fn get_create_handler(State(state): State<AppState>) -> RenderedPage {
    let template = "page_create.tera";
    let raw = RawPage::default();
    render_page(state, raw, template)
}

#[debug_handler]
async fn post_create_handler(State(state): State<AppState>, form: Form<CreateForm>) -> Result<Redirect, RenderedPage> {
    let path = &form.path;
    let pathset = match get_paths(&state.pages_dir, path) {
        None => return Err(render_error(state, ErrorMessage::bad_request())),
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

    match raw_page.write_to_path(&pathset.md).await {
        Ok(_) => Ok(Redirect::to(&pathset.url)),
        Err(err) => Err(render_error(state, ErrorMessage::from(err)))
    }
}

fn render_page(state: AppState, raw: RawPage, template: &str) -> RenderedPage {
    match state.renderer.render_page(&raw, template) {
        Ok(html) => RenderedPage::ok(raw.metadata, html),
        Err(err) => RenderedPage::internal_error(state.renderer.render_error(&err.into()))
    }
}

fn render_error(state: AppState, error: ErrorMessage) -> RenderedPage {
    RenderedPage::error(&error, state.renderer.render_error(&error))
}

fn get_paths(pages_root: &Path, path: &str) -> Option<PagePathset> {
    let (relative, url) = match validate_path(&path) {
        None => return None,
        Some(relative) => relative,
    };

    let base = pages_root.join(&relative);
    let markdown = base.with_extension("md");
    let url = { let mut str = String::from("/"); str.push_str(url); str };

    Some(PagePathset { url: url, md: markdown })
}

// Simplified version of tower's build_and_validate_path
// https://github.com/tower-rs/tower-http/blob/075479b852f348c8b74245f478b9012090acf5fc/tower-http/src/services/fs/serve_dir/mod.rs#L453
fn validate_path(requested_path: &str) -> Option<(PathBuf, &str)> {
    let path_str = requested_path.trim_start_matches('/');
    let path = Path::new(&path_str);
    if path_str.starts_with("special:") {
        return None;
    }

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

    Some((path.to_path_buf(), path_str))
}
