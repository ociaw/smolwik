use std::path::Path;
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::SignedCookieJar;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Username(String);

#[derive(Deserialize, Debug, Clone)]
pub struct Account {
    pub username: Username,
    pub password: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] tokio::io::Error),
    #[error("Deserialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountConfig {
    pub single_password: Option<String>,
    pub accounts: Vec<Account>,
}

impl AccountConfig {
    pub async fn from_file<P>(path: P) -> Result<AccountConfig, ConfigError>
    where P : AsRef<Path> {
        let mut file = File::open(path).await?;
        let mut str = String::new();
        file.read_to_string(&mut str).await?;
        Ok(serde_json::from_str(&str)?)
    }
}

#[derive(Deserialize, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum AuthenticationMode {
    /// No authentication at all, login is not possible.
    Anonymous,
    /// Single-user authentication, no accounts or username, a single password is used to authenticate.
    Single,
    /// Multi-user authentication, with multiple accounts identified by a username, each with a
    /// unique password.
    Multi
}

impl AuthenticationMode {
    pub fn variant_string(&self) -> &'static str {
        match self {
            AuthenticationMode::Anonymous => "anonymous",
            AuthenticationMode::Single => "single",
            AuthenticationMode::Multi => "multi",
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum User {
    /// No user is authenticated.
    Anonymous,
    /// The user is authenticated in single-user mode.
    SingleUser,
    /// The user is authenticated with the specified username.
    Account(Username),
}

impl User {
    pub fn check_authorization(&self, access: &Access) -> Authorization {
        use Authorization::*;
        match (self, access) {
            // Anonymous access means anyone can access this.
            (_, Access::Anonymous) => Authorized,
            // If in single user mode, the authenticated user can always access everything.
            (User::SingleUser, _) => Authorized,
            // Authenticated means that any authenticated user has access.
            (User::Account(_), Access::Authenticated) => Authorized,
            // The authenticated user must have a user name that matches one of the specified names.
            (User::Account(user), Access::Accounts(allowed)) =>
                if allowed.contains(user) { Authorized } else { Unauthorized },
            (User::Anonymous, _) => AuthenticationRequired,
        }
    }
}

impl From<SignedCookieJar> for User {
    fn from(value: SignedCookieJar) -> Self {
        match value.get("user") {
            None => User::Anonymous,
            Some(cookie) => serde_json::from_str(cookie.value()).unwrap_or(User::Anonymous)
        }
    }
}

impl From<User> for Cookie<'_> {
    fn from(value: User) -> Self {
        let value = serde_json::to_string(&value).expect("User must be serializable.");

        Cookie::build(("user", value))
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Strict)
            .build()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Access {
    /// Anyone has access.
    Anonymous,
    /// Any authenticated user has access.
    Authenticated,
    /// The username of the authenticated user must be contained in the list of usernames to have access.
    Accounts(Vec<Username>),
}

impl Access {
    pub fn variant_string(&self) -> &'static str {
        match self {
            Access::Anonymous => "anonymous",
            Access::Authenticated => "authenticated",
            Access::Accounts(_) => "accounts",
        }
    }
}

/// The result of an access check.
pub enum Authorization {
    /// The User is authorized to access the page.
    Authorized,
    /// The User is authenticated, but is not allowed to access the page.
    Unauthorized,
    /// The User is not authenticated, but page requires authentication.
    AuthenticationRequired,
}
