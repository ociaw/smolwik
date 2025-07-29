//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

mod render;
mod article;
mod metadata;
mod auth;
mod error_message;
mod login;
mod config;

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use axum::{debug_handler, routing::get, Form, Router};
use axum::extract;
use axum::extract::{FromRef, State};
use axum::response::{Html, Redirect};
use axum_extra::extract::cookie::Key;
use axum_extra::extract::SignedCookieJar;
use tower_http::{
    services::ServeDir,
    trace::TraceLayer,
};
use serde::Deserialize;
use crate::auth::*;
use crate::config::*;
use crate::error_message::ErrorMessage;
use crate::metadata::Metadata;
use crate::article::{RawArticle, RenderedArticle};
use crate::render::Renderer;

#[derive(Clone)]
struct AppState {
    pub renderer: Arc<Renderer>,
    pub config: Arc<Config>,
}

impl FromRef<AppState> for Key {
    fn from_ref(input: &AppState) -> Self {
        Key::from(&input.config.secret_key)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ArticlePaths {
    pub url: String,
    pub md: PathBuf,
}

#[derive(Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum Mode {
    Read,
    Edit,
}

#[derive(Deserialize)]
struct ArticleQuery {
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

impl Default for ArticleQuery {
    fn default() -> Self { ArticleQuery { mode: Option::from(Mode::Read) }}
}

#[tokio::main]
async fn main() {
    let config = Config::from_file("config.toml").await.expect("config.toml must be a readable, valid config.");
    let config = Arc::new(config);
    let state = AppState {
        renderer: Renderer::new(&config.templates).unwrap().into(),
        config: config.clone()
    };

    // build our application with a route
    let router = Router::new()
        .route("/special:login", get(login::get).post(login::post))
        .route("/special:create", get(create_get_handler).post(create_post_handler))
        .route("/{*path}", get(article_get_handler).post(article_post_handler))
        .route("/", get(root_article_get_handler).post(root_article_post_handler))
        .nest_service("/assets", ServeDir::new(&config.assets))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    // run it
    let listener = tokio::net::TcpListener::bind(&config.address)
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

#[debug_handler]
async fn article_get_handler(
    State(state): State<AppState>,
    extract::Path(path): extract::Path<String>,
    query: extract::Query<ArticleQuery>,
    jar: SignedCookieJar,
) -> Result<RenderedArticle, RenderedArticle> {
    let pathset = match get_paths(&state.config, &path) {
        None => return Err(render_error(state, ErrorMessage::not_found(&path))),
        Some(paths) => paths
    };

    let raw = match RawArticle::read_from_path(&pathset.md).await {
        Ok(raw) => raw,
        Err(err) => return Err(render_error(state, err.into()))
    };

    let mode = query.mode.unwrap_or(Mode::Read);
    let required = match mode {
        Mode::Read => &raw.metadata.view_access,
        Mode::Edit => &raw.metadata.edit_access,
    };

    match User::from(jar).check_authorization(required) {
        Authorization::Unauthorized => return Err(render_error(state, ErrorMessage::forbidden())),
        Authorization::AuthenticationRequired => return Err(render_error(state, ErrorMessage::unauthenticated())),
        _ => ()
    }

    let mode = query.mode.unwrap_or(Mode::Read);
    let template = match mode {
        Mode::Read => "article.tera",
        Mode::Edit => "article_edit.tera",
    };
    render_article(state, raw, template)
}

#[debug_handler]
async fn article_post_handler(
    State(state): State<AppState>,
    extract::Path(path): extract::Path<String>,
    jar: SignedCookieJar,
    form: Form<EditForm>
) -> Result<Redirect, RenderedArticle> {
    let pathset = match get_paths(&state.config, &path) {
        None => return Err(render_error(state, ErrorMessage::not_found(&path))),
        Some(paths) => paths
    };

    let raw = match RawArticle::read_from_path(&pathset.md).await {
        Ok(raw) => raw,
        Err(err) => return Err(render_error(state, err.into()))
    };

    match User::from(jar).check_authorization(&raw.metadata.edit_access) {
        Authorization::Unauthorized => return Err(render_error(state, ErrorMessage::forbidden())),
        Authorization::AuthenticationRequired => return Err(render_error(state, ErrorMessage::unauthenticated())),
        _ => ()
    }

    let metadata = Metadata {
        title: form.title.clone(),
        view_access: form.view_access.clone(),
        edit_access: form.edit_access.clone(),
    };

    let raw_article = RawArticle {
        metadata,
        markdown: form.cmark.clone(),
    };

    match raw_article.write_to_path(&pathset.md).await {
        Ok(_) => Ok(Redirect::to(&pathset.url)),
        Err(err) => {
            let err = ErrorMessage::from(err);
            Err(render_error(state, ErrorMessage::from(err)))
        },
    }
}

async fn root_article_get_handler(
    State(state): State<AppState>,
    query: extract::Query<ArticleQuery>,
    jar: SignedCookieJar,
) -> Result<RenderedArticle, RenderedArticle> {
    article_get_handler(State(state), extract::Path(String::new()), query, jar).await
}

async fn root_article_post_handler(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    form: Form<EditForm>
) -> Result<Redirect, RenderedArticle> {
    article_post_handler(State(state), extract::Path(String::new()), jar, form).await
}

#[debug_handler]
async fn create_get_handler(
    State(state): State<AppState>,
    jar: SignedCookieJar,
) -> Result<RenderedArticle, RenderedArticle> {
    match User::from(jar).check_authorization(&state.config.create_access) {
        Authorization::Unauthorized => return Err(render_error(state, ErrorMessage::forbidden())),
        Authorization::AuthenticationRequired => return Err(render_error(state, ErrorMessage::unauthenticated())),
        _ => ()
    }

    let template = "article_create.tera";
    let raw = RawArticle::default();
    render_article(state, raw, template)
}

#[debug_handler]
async fn create_post_handler(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    form: Form<CreateForm>,
) -> Result<Redirect, RenderedArticle> {
    let path = &form.path;
    let pathset = match get_paths(&state.config, path) {
        None => return Err(render_error(state, ErrorMessage::bad_request())),
        Some(paths) => paths
    };

    match User::from(jar).check_authorization(&state.config.create_access) {
        Authorization::Unauthorized => return Err(render_error(state, ErrorMessage::forbidden())),
        Authorization::AuthenticationRequired => return Err(render_error(state, ErrorMessage::unauthenticated())),
        _ => ()
    }

    let metadata = Metadata {
        title: form.title.clone(),
        view_access: form.view_access.clone(),
        edit_access: form.edit_access.clone(),
    };

    let raw_article = RawArticle {
        metadata,
        markdown: form.cmark.clone(),
    };

    match raw_article.write_to_path(&pathset.md).await {
        Ok(_) => Ok(Redirect::to(&pathset.url)),
        Err(err) => Err(render_error(state, ErrorMessage::from(err)))
    }
}

fn render_article(state: AppState, raw: RawArticle, template: &str) -> Result<RenderedArticle, RenderedArticle> {
    match state.renderer.render_article(&raw, template) {
        Ok(html) => Ok(RenderedArticle::ok(html)),
        Err(err) => Err(RenderedArticle::internal_error(state.renderer.render_error(&err.into())))
    }
}

fn render_template(state: AppState, template: &str, title: &str) -> Result<Html<String>, Html<String>> {
    match state.renderer.render_template(&state, template, title) {
        Ok(html) => Ok(Html(html)),
        Err(err) => Err(Html(state.renderer.render_error(&err.into())))
    }
}

fn render_error(state: AppState, error: ErrorMessage) -> RenderedArticle {
    RenderedArticle::error(&error, state.renderer.render_error(&error))
}

fn get_paths(config: &Config, path: &str) -> Option<ArticlePaths> {
    let mut relative = match validate_path(&path) {
        None => return None,
        Some(relative) => relative,
    };

    // If the path points to a directory, use the index of the directory instead
    let file_stem = {
        let mut file_stem = config.articles.join(&relative);
        if relative.to_str().unwrap().ends_with("/") || file_stem.is_dir() {
            file_stem.push("index");
            relative.push("index");
        }
        file_stem
    };

    let markdown = file_stem.with_extension("md");
    let url = format!("/{}", relative.to_str().unwrap());

    Some(ArticlePaths { url, md: markdown })
}

// Simplified version of tower's build_and_validate_path
// https://github.com/tower-rs/tower-http/blob/075479b852f348c8b74245f478b9012090acf5fc/tower-http/src/services/fs/serve_dir/mod.rs#L453
fn validate_path(path: &str) -> Option<PathBuf> {
    let path = path.trim_start_matches('/');
    if path.starts_with("special:") {
        return None;
    }

    let path = PathBuf::from(&path);
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

    Some(path)
}
