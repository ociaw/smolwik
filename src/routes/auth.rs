use crate::*;
use crate::auth::*;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::SignedCookieJar;
use crate::extractors::Form;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/special:login", get(get_handler).post(post_handler))
        .route("/special:logout", post(logout_handler))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: Option<Username>,
    pub password: String,
}

#[debug_handler]
async fn get_handler(State(state): State<AppState>, user: User) -> Response {
    match &user {
        User::Anonymous => state.renderer.render_template(&user, "login.tera", "Login").map_or_else(
            |err| {
                let mut response = state.renderer.render_error(&user, &err.into()).into_response();
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                response
            },
            |s| Html(s).into_response()
        ),
        _ => render_error(state, &user, ErrorMessage::already_authenticated())
    }
}

#[debug_handler]
async fn post_handler(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    form: Form<LoginForm>
) -> Result<(SignedCookieJar, Redirect), Response> {
    let user = User::from(&jar);
    if !matches!(user, User::Anonymous) {
        return Err(render_error(state, &user, ErrorMessage::already_authenticated()))
    }
    if matches!(state.config.auth_mode, AuthenticationMode::Anonymous) {
        return Err(render_error(state, &user, ErrorMessage::bad_request()))
    }
    let account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => config,
        Err(err) => return Err(render_error(state, &user, err.into())),
    };

    let user: Option<User> = match state.config.auth_mode {
        AuthenticationMode::Anonymous => unreachable!(),
        AuthenticationMode::Single => account_config.single_password.map_or(false, |p| p == form.password).then(|| User::SingleUser),
        AuthenticationMode::Multi => {
            match &form.username {
                None => None,
                Some(username) =>
                    account_config.accounts.iter().any(|acc| &acc.username == username && &acc.password == &form.password).then(|| User::Account(username.clone())),
            }
        }
    };

    let user = match user {
        None => return Err(render_error(state, &User::Anonymous, ErrorMessage::invalid_credentials())),
        Some(u) => u,
    };

    let cookie = Cookie::from(user);
    let jar = jar.add(cookie);
    Ok((jar, Redirect::to("/")))
}

#[debug_handler]
async fn logout_handler(State(state): State<AppState>, mut jar: SignedCookieJar) -> (SignedCookieJar, Redirect) {
    if let Some(cookie) = jar.get("user") {
        jar = jar.remove(cookie)
    }

    (jar, Redirect::to("/"))
}

