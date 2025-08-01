#![feature(path_add_extension)]

mod render;
mod article;
mod metadata;
mod auth;
mod error_message;
mod config;
mod routes;
mod extractors;

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use axum::{debug_handler, routing::get, Router};
use axum::response::{Html, Redirect};
use axum_core::response::{IntoResponse, Response};
use tower_http::{services::ServeDir, trace::TraceLayer};
use serde::Deserialize;
use tera::Context;
use crate::config::*;
pub use crate::error_message::ErrorMessage;
pub use crate::metadata::Metadata;
use crate::article::RawArticle;
use crate::auth::{Access, User};
use crate::render::Renderer;

#[derive(Clone)]
struct AppState {
    pub renderer: Arc<Renderer>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() {
    let config = match Config::from_file("config.toml").await {
        Ok(c) => Arc::new(c),
        Err(err) => {
            eprintln!("Couldn't open `config.toml`: {}", err.to_string());
            return
        }
    };

    let state = AppState {
        renderer: Renderer::new((*config).clone()).unwrap().into(),
        config: config.clone()
    };

    let article_routes = routes::articles::router(state.clone());
    let auth_routes = routes::auth::router(state.clone());
    let admin_routes = routes::admin::router(state.clone());

    tracing_subscriber::fmt::init();

    // build our application with a route
    let router = Router::new()
        .nest_service("/assets", ServeDir::new(&config.assets))
        .with_state(state.clone())
        .merge(article_routes)
        .merge(auth_routes)
        .merge(admin_routes)
        .layer(TraceLayer::new_for_http());

    // run it
    let listener = tokio::net::TcpListener::bind(&config.address)
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}


fn render_error(state: &AppState, user: &User, error: ErrorMessage) -> Response {
    let mut response = Html(state.renderer.render_error(&user, &error)).into_response();
    *response.status_mut() = error.status_code;
    response
}

fn render_template(state: &AppState, user: &User, template: &str, title: &str) -> Result<Response, Response> {
    state.renderer.render_template(&user, template, title).map_or_else(
        |err| Err(render_error(state, user, err.into())),
        |s| Ok(Html(s).into_response())
    )
}

fn render_template_with_context(state: &AppState, user: &User, template: &str, title: &str, context: Context) -> Result<Response, Response> {
    state.renderer.render_template_with_context(&user, template, title, context).map_or_else(
        |err| Err(render_error(state, user, err.into())),
        |s| Ok(Html(s).into_response())
    )
}

fn check_access(user: &User, state: &AppState, access: &Access) -> Result<(), Response> {
    use crate::auth::Authorization;

    match user.check_authorization(access) {
        Authorization::Unauthorized => Err(render_error(state, user, ErrorMessage::forbidden())),
        Authorization::AuthenticationRequired => Err(render_error(state, user, ErrorMessage::unauthenticated())),
        _ => Ok(())
    }
}
