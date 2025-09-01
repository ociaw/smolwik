use crate::*;
use crate::auth::*;
use axum::extract::State;
use axum::routing::post;
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::SignedCookieJar;
use crate::extractors::Form;
use crate::template::TemplateResponse;

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
async fn get_handler(State(state): State<AppState>, user: User) -> TemplateResponse {
    match &user {
        User::Anonymous => TemplateResponse::from_template(state, user, "login.tera", context("Login")),
        _ => TemplateResponse::from_error(state, user, ErrorMessage::already_authenticated()),
    }
}

#[debug_handler]
async fn post_handler(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    form: Form<LoginForm>
) -> Result<(SignedCookieJar, Redirect), TemplateResponse> {
    let user = User::from(&jar);
    if user != User::Anonymous {
        return Err(TemplateResponse::from_error(state, user, ErrorMessage::already_authenticated()))
    }
    if state.config.auth_mode == AuthenticationMode::Anonymous {
        return Err(TemplateResponse::from_error(state, user, ErrorMessage::bad_request()))
    }
    let account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => config,
        Err(err) => return Err(TemplateResponse::from_error(state, user, err.into())),
    };

    let user: Option<User> = match state.config.auth_mode {
        AuthenticationMode::Anonymous => unreachable!(),
        AuthenticationMode::Single => account_config.single_password
            .map_or(false, |hash| verify_password(&form.password, &hash).is_ok())
            .then(|| User::SingleUser),
        AuthenticationMode::Multi => {
            match &form.username {
                None => None,
                Some(username) => {
                    if let Some(acc) = account_config.accounts.iter().find(|acc| &acc.username == username) {
                        acc.verify_password(&form.password).map_or(None, |_| Some(User::Account(acc.username.clone())))
                    }
                    else {
                        None
                    }
                }
            }
        }
    };

    let user = match user {
        None => return Err(TemplateResponse::from_error(state, User::Anonymous, ErrorMessage::invalid_credentials())),
        Some(u) => u,
    };

    let cookie = Cookie::from(user);
    let jar = jar.add(cookie);
    Ok((jar, Redirect::to("/")))
}

#[debug_handler]
async fn logout_handler(State(_state): State<AppState>, mut jar: SignedCookieJar) -> (SignedCookieJar, Redirect) {
    if let Some(cookie) = jar.get("user") {
        jar = jar.remove(cookie)
    }

    (jar, Redirect::to("/"))
}

