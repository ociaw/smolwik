#![feature(path_add_extension)]

mod article;
mod auth;
mod config;
mod extractors;
mod filesystem;
mod metadata;
mod render;
mod responses;
mod routes;

use crate::article::RawArticle;
use crate::auth::{Access, User};
use crate::config::*;
pub use crate::metadata::Metadata;
use crate::render::Renderer;
pub use crate::responses::ErrorResponse;
use crate::responses::TemplatedResponse;
use axum::extract::State;
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{Html, Redirect};
use axum::{Router, debug_handler, routing::get};
use axum_core::body::Body;
use axum_core::response::{IntoResponse, Response};
use http::Request;
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tera::Context;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    pub renderer: Arc<Renderer>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() {
    let mut config = match Config::from_file("config.toml").await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Couldn't open `config.toml`: {}", err.to_string());
            return;
        }
    };

    // Ensure we have a valid secret key define.
    if config.secret_key.len() < 64 {
        let key_string = config.generate_secret_key();
        eprintln!(
            "WARN: Empty or weak secret_key found in configuration. Using temp value; to make permanent, update config.toml with {key_string}"
        );
    }
    let config = Arc::new(config);

    let mut account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Couldn't open `accounts.toml`: {}", err.to_string());
            return;
        }
    };
    if !account_config.validate_single_user_password() && config.auth_mode == auth::AuthenticationMode::Single {
        let password = account_config.generate_single_user_password();
        eprintln!("WARN: Missing or invalid single-user password specified. Updating password to {password}");

        if let Err(err) = account_config.write_to_file("accounts.toml").await {
            eprintln!("ERR: Failed to update accounts.toml. Authentication will not be possible.\n {err}")
        }
    }

    let state = AppState {
        renderer: Renderer::new((*config).clone()).unwrap().into(),
        config: config.clone(),
    };

    let article_routes = routes::articles::router(state.clone());
    let auth_routes = routes::auth::router(state.clone());
    let admin_routes = routes::admin::router(state.clone());
    let discovery_routes = routes::discovery::router(state.clone());

    tracing_subscriber::fmt::init();

    // build our application with a route
    let router = Router::new()
        .nest_service("/assets", ServeDir::new(&config.assets))
        .with_state(state.clone())
        .merge(article_routes)
        .merge(auth_routes)
        .merge(admin_routes)
        .merge(discovery_routes)
        .layer(from_fn_with_state(state, template_middleware))
        .layer(TraceLayer::new_for_http());

    // run it
    let listener = tokio::net::TcpListener::bind(&config.address).await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

async fn template_middleware(State(state): State<AppState>, user: User, request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let extensions = response.extensions_mut();
    if extensions.len() == 0 {
        // For routes that don't use extensions, just return the original response
        return response;
    }

    // Render errors that occurred in handler.
    if let Some(error) = extensions.remove::<ErrorResponse>() {
        return render_error(&state, &user, error);
    }

    // Build a new response from the extension data.
    let context = extensions.remove::<Context>();
    let template = extensions.remove::<&'static str>().expect("String (Template) must be set.");
    let title = extensions.remove::<String>().unwrap_or(String::new());

    if let Some(context) = context {
        return match state.renderer.render_template_with_context(&user, &template, &title, context) {
            Ok(html) => Html(html).into_response(),
            Err(err) => render_error(&state, &user, err.into()),
        };
    }

    match state.renderer.render_template(&user, &template, &title) {
        Ok(html) => Html(html).into_response(),
        Err(err) => render_error(&state, &user, err.into()),
    }
}

/// Creates a new template [Context] with the `title` key set to the specified value.
pub(crate) fn context(title: &str) -> Context {
    let mut context = Context::new();
    context.insert("title", title);
    context
}

fn render_error(state: &AppState, user: &User, error: ErrorResponse) -> Response {
    let mut response = Html(state.renderer.render_error(&user, &error)).into_response();
    *response.status_mut() = error.status_code;
    response
}

/// Checks if the specified user has the specified access. Returns an error response with an error
/// message if the access check fails.
fn check_access(user: &User, access: &Access) -> Result<(), ErrorResponse> {
    use crate::auth::Authorization;

    match user.check_authorization(access) {
        Authorization::Unauthorized => Err(ErrorResponse::forbidden()),
        Authorization::AuthenticationRequired => Err(ErrorResponse::unauthenticated()),
        _ => Ok(()),
    }
}
