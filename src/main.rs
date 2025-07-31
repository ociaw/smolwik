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
mod config;
mod routes;
mod extractors;

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use axum::{debug_handler, routing::get, Router};
use axum::response::{Html, Redirect};
use axum_core::response::{IntoResponse, Response};
use tower_http::{
    services::ServeDir,
    trace::TraceLayer,
};
use serde::Deserialize;
use crate::config::*;
pub use crate::error_message::ErrorMessage;
pub use crate::metadata::Metadata;
use crate::article::RawArticle;
use crate::auth::User;
use crate::render::Renderer;

#[derive(Clone)]
struct AppState {
    pub renderer: Arc<Renderer>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() {
    let config = Config::from_file("config.toml").await.expect("config.toml must be a readable, valid config.");
    let config = Arc::new(config);
    let state = AppState {
        renderer: Renderer::new((*config).clone()).unwrap().into(),
        config: config.clone()
    };

    let article_routes = routes::articles::router(state.clone());
    let auth_routes = routes::auth::router(state.clone());

    // build our application with a route
    let router = Router::new()
        .nest_service("/assets", ServeDir::new(&config.assets))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone())
        .merge(article_routes)
        .merge(auth_routes);

    // run it
    let listener = tokio::net::TcpListener::bind(&config.address)
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}


fn render_error(state: AppState, user: &User, error: ErrorMessage) -> Response {
    let mut response = Html(state.renderer.render_error(&user, &error)).into_response();
    *response.status_mut() = error.status_code;
    response
}
