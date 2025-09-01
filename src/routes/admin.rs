use axum::{debug_handler, extract, routing::get, Router};
use axum::extract::State;
use axum::response::Redirect;
use axum::routing::post;
use serde::Deserialize;
use crate::auth::*;
use crate::*;
use crate::extractors::Form;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/special:admin", get(admin_get_handler))
        .route("/special:admin:account", get(account_get_handler))
        .route("/special:admin:add_account", get(add_account_get_handler).post(add_account_post_handler))
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
    pub username: Username,
    pub password: String,
}

#[derive(Deserialize)]
struct ChangePasswordForm {
    pub username: Option<Username>,
    pub password: String,
}

async fn authorize_middleware(State(state): State<AppState>, user: User, request: Request<Body>, next: Next) -> Response {
    match user.check_authorization(&state.config.administrator_access) {
        Authorization::Authorized => next.run(request).await,
        Authorization::Unauthorized => render_error(&state, &user, ErrorMessage::forbidden()),
        Authorization::AuthenticationRequired => render_error(&state, &user, ErrorMessage::unauthenticated()),
    }
}

#[debug_handler]
async fn admin_get_handler() -> Result<TemplateResponse, ErrorMessage> {
    let account_config = load_account_config().await.map_err(|err| err)?;
    let accounts = account_config.accounts.iter().map(|acc| &acc.username).collect::<Vec<_>>();

    let mut context = context("Admin");
    context.insert("admin__accounts", &accounts);

    Ok(TemplateResponse::from_template("admin.tera", context))
}

#[debug_handler]
async fn account_get_handler(query: extract::Query<EditAccountQuery>) -> Result<TemplateResponse, ErrorMessage> {
    let account_config = load_account_config().await.map_err(|err| err)?;
    let account = match account_config.find_by_username(&query.username) {
        None => return Err(ErrorMessage::account_not_found(&query.username)),
        Some(acc) => acc
    };

    let mut context = context("Editing Account");
    context.insert("admin__username", &account.username);
    Ok(TemplateResponse::from_template("admin.account.tera", context))
}

#[debug_handler]
async fn add_account_get_handler() -> TemplateResponse {
    TemplateResponse::from_template("admin.add_account.tera", context("Add Account"))
}

#[debug_handler]
async fn add_account_post_handler(form: Form<AddAccountForm>) -> Result<Redirect, ErrorMessage> {
    let mut account_config = match load_account_config().await {
        Ok(config) => config,
        Err(err) => return Err(err),
    };

    match account_config.find_by_username_mut(&form.username) {
        None => account_config.accounts.push(Account::new(form.username.clone(), &form.password)),
        Some(acc) => return Err(
            ErrorMessage::conflict("Account Already Exists", format!("An account with the username `{}` already exists.", acc.username))
        ),
    };

    save_account_config(&account_config).await
        .map_or_else(|err| Err(err), |_| Ok(Redirect::to("/")))
}

#[debug_handler]
async fn change_password_post_handler(form: Form<ChangePasswordForm>) -> Result<Redirect, ErrorMessage> {
    let mut account_config = match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => config,
        Err(err) => return Err(ErrorMessage::from(err)),
    };

    match &form.username {
        Some(username) =>
            match account_config.find_by_username_mut(username) {
                Some(acc) => acc.set_password(&form.password),
                None => return Err(ErrorMessage::account_not_found(username)),
            },
        None => account_config.single_password = Some(hash_password(&form.password)),
    }

    save_account_config(&account_config).await
        .map_or_else(|err| Err(err), |_| Ok(Redirect::to("/")))
}

async fn load_account_config() -> Result<AccountConfig, ErrorMessage> {
    match AccountConfig::from_file("accounts.toml").await {
        Ok(config) => Ok(config),
        Err(err) => Err(ErrorMessage::from(err)),
    }
}

async fn save_account_config(config: &AccountConfig) -> Result<(), ErrorMessage> {
    match config.write_to_file("accounts.toml").await {
        Ok(()) => Ok(()),
        Err(err) => Err(ErrorMessage::from(err)),
    }
}
