use crate::*;
use crate::auth::*;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::SignedCookieJar;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: Option<Username>,
    pub password: String,
}

pub async fn get(State(state): State<AppState>, jar: SignedCookieJar) -> Response {
    match User::from(jar) {
        User::Anonymous => render_template(state, "login.tera", "Login").into_response(),
        _ => render_error(state, ErrorMessage::already_authenticated())
    }
}

#[debug_handler]
pub async fn post(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    form: Form<LoginForm>
) -> Result<(SignedCookieJar, Redirect), Response> {
    if matches!(state.auth_mod, AuthenticationMode::Anonymous) {
        return Err(render_error(state, ErrorMessage::bad_request()))
    }
    let account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => config,
        Err(err) => return Err(render_error(state, err.into())),
    };

    let user: Option<User> = match state.auth_mod {
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
        None => return Err(render_error(state, ErrorMessage::invalid_credentials())),
        Some(u) => u,
    };

    let cookie = Cookie::from(user);
    let jar = jar.add(cookie);
    Ok((jar, Redirect::to("/")))
}

fn render_error(state: AppState, error: ErrorMessage) -> Response {
    let mut response = Html(state.renderer.render_error(&error)).into_response();
    *response.status_mut() = error.status_code;
    response
}
