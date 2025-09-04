use crate::auth::*;
use crate::extractors::Form;
use crate::responses::TemplatedResponse;
use crate::*;
use axum::extract::State;
use axum::routing::post;
use axum_extra::extract::SignedCookieJar;
use axum_extra::extract::cookie::Cookie;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/special:login", get(get_handler).post(post_handler))
        .route("/special:logout", post(logout_handler))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct LoginForm {
    /// The username to login under. [None] if authenticating in Single-User mode.
    pub username: Option<Username>,
    /// The password associated with the account.
    pub password: String,
    /// The ID of the current session, used to prevent CSRF attacks. Must match the ID set in the session cookie.
    pub session_id: String,
}

impl AntiCsrfForm for LoginForm {
    fn session(&self) -> &str {
        &self.session_id
    }
}

#[debug_handler]
async fn get_handler(
    State(_): State<AppState>,
    session: Session,
    jar: SignedCookieJar,
) -> Result<(SignedCookieJar, TemplatedResponse), ErrorResponse> {
    match (&session.user, &session.id) {
        (User::Anonymous, None) => {
            let mut context = context("Login");
            let session = Session::new(User::Anonymous);
            context.insert("session_id", &session.id);
            let cookie = Cookie::from(session);
            let jar = jar.add(cookie);
            Ok((jar, TemplatedResponse::new("login.tera", context)))
        }
        (User::Anonymous, Some(_)) => {
            let mut context = context("Login");
            context.insert("session_id", &session.id);
            Ok((jar, TemplatedResponse::new("login.tera", context)))
        }
        _ => Err(ErrorResponse::already_authenticated()),
    }
}

#[debug_handler]
async fn post_handler(
    State(state): State<AppState>,
    jar: SignedCookieJar,
    form: Form<LoginForm>,
) -> Result<(SignedCookieJar, Redirect), ErrorResponse> {
    let session = Session::from(&jar);
    if session.user != User::Anonymous {
        return Err(ErrorResponse::already_authenticated());
    }
    if state.config.auth_mode == AuthenticationMode::Anonymous {
        return Err(ErrorResponse::bad_request());
    }
    let account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => config,
        Err(err) => return Err(ErrorResponse::from(err)),
    };

    let user: Option<User> = match state.config.auth_mode {
        AuthenticationMode::Anonymous => unreachable!(),
        AuthenticationMode::Single => account_config
            .single_password
            .map_or(false, |hash| verify_password(&form.password, &hash).is_ok())
            .then(|| User::SingleUser),
        AuthenticationMode::Multi => match &form.username {
            None => None,
            Some(username) => {
                if let Some(acc) = account_config.accounts.iter().find(|acc| &acc.username == username) {
                    acc.verify_password(&form.password)
                        .map_or(None, |_| Some(User::Account(acc.username.clone())))
                } else {
                    None
                }
            }
        },
    };

    let user = match user {
        None => return Err(ErrorResponse::invalid_credentials().into()),
        Some(u) => u,
    };

    let session = Session::new(user);
    let cookie = Cookie::from(session);
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
