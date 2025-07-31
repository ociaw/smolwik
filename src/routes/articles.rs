use axum::{debug_handler, routing::get, Router};
use axum::extract;
use axum::extract::State;
use axum::response::Redirect;
use serde::Deserialize;
use crate::auth::*;
use crate::article::{RawArticle, RenderedArticle};
use crate::*;
use crate::extractors::Form;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/special:create", get(create_get_handler).post(create_post_handler))
        .route("/{*path}", get(get_handler).post(post_handler))
        .route("/", get(root_get_handler).post(root_post_handler))
        .with_state(state)
}

#[derive(Deserialize)]
struct ArticleQuery {
    pub edit: Option<String>,
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct ArticlePaths {
    pub url: String,
    pub md: PathBuf,
}

#[debug_handler]
async fn get_handler(
    State(state): State<AppState>,
    extract::Path(path): extract::Path<String>,
    query: extract::Query<ArticleQuery>,
    user: User,
) -> Result<RenderedArticle, RenderedArticle> {
    let pathset = match get_paths(&state.config, &path) {
         None => return Err(render_error(&state, &user, ErrorMessage::not_found(&path))),
        Some(paths) => paths
    };

    let raw = match RawArticle::read_from_path(&pathset.md).await {
        Ok(raw) => raw,
        Err(err) => return Err(render_error(&state, &user, err.into()))
    };

    let required = match &query.edit {
        Some(_) => &raw.metadata.edit_access,
        None => &raw.metadata.view_access,
    };

    if let Err(err) = check_access(&user, &state, &required) {
        return Err(err);
    }

    let template = match &query.edit {
        Some(_) => "article_edit.tera",
        None => "article.tera",
    };

    render_article(state, &user, raw, template)
}

#[debug_handler]
async fn post_handler(
    State(state): State<AppState>,
    extract::Path(path): extract::Path<String>,
    user: User,
    form: Form<EditForm>
) -> Result<Redirect, RenderedArticle> {
    let pathset = match get_paths(&state.config, &path) {
        None => return Err(render_error(&state, &user, ErrorMessage::not_found(&path))),
        Some(paths) => paths
    };

    let raw = match RawArticle::read_from_path(&pathset.md).await {
        Ok(raw) => raw,
        Err(err) => return Err(render_error(&state, &user, err.into()))
    };

    if let Err(err) = check_access(&user, &state, &raw.metadata.edit_access) {
        return Err(err);
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
            Err(render_error(&state, &user, ErrorMessage::from(err)))
        },
    }
}

async fn root_get_handler(
    State(state): State<AppState>,
    query: extract::Query<ArticleQuery>,
    user: User,
) -> Result<RenderedArticle, RenderedArticle> {
    get_handler(State(state), extract::Path(String::new()), query, user).await
}

async fn root_post_handler(
    State(state): State<AppState>,
    user: User,
    form: Form<EditForm>
) -> Result<Redirect, RenderedArticle> {
    post_handler(State(state), extract::Path(String::new()), user, form).await
}


#[debug_handler]
async fn create_get_handler(
    State(state): State<AppState>,
    user: User,
) -> Result<RenderedArticle, RenderedArticle> {
    if let Err(err) = check_access(&user, &state, &state.config.create_access) {
        return Err(err);
    }

    let template = "article_create.tera";
    let raw = RawArticle::default();
    render_article(state, &user, raw, template)
}

#[debug_handler]
async fn create_post_handler(
    State(state): State<AppState>,
    user: User,
    form: Form<CreateForm>,
) -> Result<Redirect, RenderedArticle> {
    let path = &form.path;
    let pathset = match get_paths(&state.config, path) {
        None => return Err(render_error(&state, &user, ErrorMessage::bad_request())),
        Some(paths) => paths
    };

    if let Err(err) = check_access(&user, &state, &state.config.create_access) {
        return Err(err);
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
        Err(err) => Err(render_error(&state, &user, ErrorMessage::from(err)))
    }
}

fn render_article(state: AppState, user: &User, raw: RawArticle, template: &str) -> Result<RenderedArticle, RenderedArticle> {
    match state.renderer.render_article(&user, &raw, template) {
        Ok(html) => Ok(RenderedArticle::ok(html)),
        Err(err) => Err(RenderedArticle::internal_error(state.renderer.render_error(&user, &err.into())))
    }
}

fn render_error(state: &AppState, user: &User, error: ErrorMessage) -> RenderedArticle {
    RenderedArticle::error(&error, state.renderer.render_error(&user, &error))
}

fn check_access(user: &User, state: &AppState, access: &Access) -> Result<(), RenderedArticle> {
    match user.check_authorization(access) {
        Authorization::Unauthorized => Err(render_error(state, &user, ErrorMessage::forbidden())),
        Authorization::AuthenticationRequired => Err(render_error(state, &user, ErrorMessage::unauthenticated())),
        _ => Ok(())
    }
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
