use crate::auth::*;
use crate::extractors::Form;
use crate::*;
use axum::extract::State;
use axum::response::Redirect;
use axum::routing::post;
use axum::{Router, debug_handler, extract, routing::get};
use serde::Deserialize;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/special:admin", get(admin_get_handler))
        .route("/special:admin:account", get(account_get_handler))
        .route(
            "/special:admin:add_account",
            get(add_account_get_handler).post(add_account_post_handler),
        )
        .route("/special:admin:change_password", post(change_password_post_handler))
        .with_state(state.clone())
        .layer(from_fn_with_state(state, authorize_middleware))
}

#[derive(Deserialize)]
struct EditAccountQuery {
    pub username: Username,
}

#[derive(Deserialize)]
struct AddAccountForm {
    /// The username of the new account.
    pub username: Username,
    /// The password of the new account.
    pub password: String,
    /// The ID of the current session, used to prevent CSRF attacks. Must match the ID set in the session cookie.
    pub session_id: String,
}

impl AntiCsrfForm for AddAccountForm {
    fn session(&self) -> &str {
        &self.session_id
    }
}

#[derive(Deserialize)]
struct ChangePasswordForm {
    /// The username of the account to change the password of. [None] for Single-User mode.
    pub username: Option<Username>,
    /// The new password.
    pub password: String,
    /// The ID of the current session, used to prevent CSRF attacks. Must match the ID set in the session cookie.
    pub session_id: String,
}

impl AntiCsrfForm for ChangePasswordForm {
    fn session(&self) -> &str {
        &self.session_id
    }
}

async fn authorize_middleware(
    State(state): State<AppState>,
    session: Session,
    request: Request<Body>,
    next: Next,
) -> Response {
    match session.user.check_authorization(&state.config.administrator_access) {
        Authorization::Authorized => next.run(request).await,
        Authorization::Unauthorized => render_error(&state, &session, ErrorResponse::forbidden()),
        Authorization::AuthenticationRequired => render_error(&state, &session, ErrorResponse::unauthenticated()),
    }
}

#[debug_handler]
async fn admin_get_handler() -> Result<TemplatedResponse, ErrorResponse> {
    let account_config = load_account_config().await.map_err(|err| err)?;
    let accounts = account_config.accounts.iter().map(|acc| &acc.username).collect::<Vec<_>>();

    let mut context = context("Admin");
    context.insert("admin__accounts", &accounts);

    Ok(TemplatedResponse::new("admin.tera", context))
}

#[debug_handler]
async fn account_get_handler(query: extract::Query<EditAccountQuery>) -> Result<TemplatedResponse, ErrorResponse> {
    let account_config = load_account_config().await.map_err(|err| err)?;
    let account = match account_config.find_by_username(&query.username) {
        None => return Err(ErrorResponse::account_not_found(&query.username)),
        Some(acc) => acc,
    };

    let mut context = context("Editing Account");
    context.insert("admin__username", &account.username);
    Ok(TemplatedResponse::new("admin.account.tera", context))
}

#[debug_handler]
async fn add_account_get_handler() -> TemplatedResponse {
    TemplatedResponse::new("admin.add_account.tera", context("Add Account"))
}

#[debug_handler]
async fn add_account_post_handler(
    State(_): State<AppState>,
    session: Session,
    form: Form<AddAccountForm>,
) -> Result<Redirect, ErrorResponse> {
    // Validate the session ID to prevent CSRF attacks.
    if !form.is_valid(session.id.as_deref()) {
        return Err(ErrorResponse::bad_request());
    }

    let mut account_config = match load_account_config().await {
        Ok(config) => config,
        Err(err) => return Err(err),
    };

    match account_config.find_by_username_mut(&form.username) {
        None => account_config
            .accounts
            .push(Account::new(form.username.clone(), &form.password)),
        Some(acc) => {
            return Err(ErrorResponse::conflict(
                "Account Already Exists",
                format!("An account with the username `{}` already exists.", acc.username),
            ));
        }
    };

    save_account_config(&account_config)
        .await
        .map_or_else(|err| Err(err), |_| Ok(Redirect::to("/")))
}

#[debug_handler]
async fn change_password_post_handler(
    State(_): State<AppState>,
    form: Form<ChangePasswordForm>,
) -> Result<Redirect, ErrorResponse> {
    let mut account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => config,
        Err(err) => return Err(ErrorResponse::from(err)),
    };

    match &form.username {
        Some(username) => match account_config.find_by_username_mut(username) {
            Some(acc) => acc.set_password(&form.password),
            None => return Err(ErrorResponse::account_not_found(username)),
        },
        None => account_config.single_password = Some(hash_password(&form.password)),
    }

    save_account_config(&account_config)
        .await
        .map_or_else(|err| Err(err), |_| Ok(Redirect::to("/")))
}

async fn load_account_config() -> Result<AccountConfig, ErrorResponse> {
    match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => Ok(config),
        Err(err) => Err(ErrorResponse::from(err)),
    }
}

async fn save_account_config(config: &AccountConfig) -> Result<(), ErrorResponse> {
    match config.write_to_file("accounts.toml").await {
        Ok(()) => Ok(()),
        Err(err) => Err(ErrorResponse::from(err)),
    }
}
