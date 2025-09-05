use axum_extra::extract::SignedCookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Username(String);

impl Display for Username {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Username {
    fn from(value: &str) -> Self {
        Username(value.into())
    }
}

impl From<String> for Username {
    fn from(value: String) -> Self {
        Username(value)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub username: Username,
    password: String,
}

impl Account {
    pub fn new(username: Username, password: &str) -> Account {
        let hash = hash_password(password);

        Account {
            username,
            password: hash.to_string(),
        }
    }

    pub fn verify_password(&self, password: &str) -> Result<(), ()> {
        verify_password(password, &self.password)
    }

    pub fn set_password(&mut self, password: &str) {
        self.password = hash_password(password);
    }
}

pub fn verify_password(password: &str, existing_hash: &str) -> Result<(), ()> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHash, PasswordVerifier},
    };

    let existing_hash = PasswordHash::new(existing_hash).map_err(|_| ())?;
    let argon2 = Argon2::default();
    argon2.verify_password(password.as_bytes(), &existing_hash).map_err(|_| ())
}

pub fn hash_password(password: &str) -> String {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Password hashing should be infallible.");

    hash.to_string()
}

#[derive(Deserialize, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum AuthenticationMode {
    /// No authentication at all, login is not possible.
    Anonymous,
    /// Single-user authentication, no accounts or username, a single password is used to authenticate.
    Single,
    /// Multi-user authentication, with multiple accounts identified by a username, each with a
    /// unique password.
    Multi,
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
            (User::Account(username), Access::Accounts(allowed)) => {
                match allowed.contains(username) {
                    true => Authorized,
                    false => Unauthorized,
                }
            }
            (User::Anonymous, _) => AuthenticationRequired,
        }
    }
}

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            User::Anonymous => write!(f, "Anonymous"),
            User::SingleUser => write!(f, "Single User"),
            User::Account(username) => write!(f, "{username}"),
        }
    }
}

/// Identifies a user's session.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Session {
    /// The ID of this session. None if a session has not been started yet.
    pub id: Option<String>,
    /// The current user.
    pub user: User,
}

impl Session {
    pub fn new(user: User) -> Session {
        Session {
            id: Some(generate_random_token()),
            user,
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self {
            user: User::Anonymous,
            id: None,
        }
    }
}

impl From<Session> for Cookie<'_> {
    fn from(value: Session) -> Self {
        let value = serde_json::to_string(&value).expect("User must be serializable.");

        Cookie::build(("session", value)).same_site(SameSite::Strict).build()
    }
}

impl From<SignedCookieJar> for Session {
    fn from(value: SignedCookieJar) -> Self {
        Self::from(&value)
    }
}

impl From<&SignedCookieJar> for Session {
    fn from(value: &SignedCookieJar) -> Self {
        match value.get("session") {
            None => Self::default(),
            Some(cookie) => serde_json::from_str(cookie.value()).unwrap_or(Self::default()),
        }
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
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum Authorization {
    /// The User is authorized to access the page.
    Authorized,
    /// The User is authenticated, but is not allowed to access the page.
    Unauthorized,
    /// The User is not authenticated, but page requires authentication.
    AuthenticationRequired,
}

fn generate_random_token() -> String {
    use base64::prelude::*;
    use rand_core::RngCore;
    let mut bytes = vec![0u8; 64];
    rand_core::OsRng::default().fill_bytes(&mut bytes);
    BASE64_STANDARD.encode(&bytes)
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;
    use crate::auth::*;

    #[test]
    fn create_account_verify_password() -> Result<(), ()> {
        let password = "password";
        let account = Account::new("username".into(), password);
        assert_matches!(account.verify_password(password), Ok(()));
        assert_matches!(account.verify_password("not_password"), Err(()));
        Ok(())
    }

    #[test]
    fn set_password_verify_password() -> Result<(), ()> {
        let old_password = "old_password";
        let new_password = "new_password";
        let mut account = Account::new("username".into(), old_password);
        account.set_password(new_password);
        assert_matches!(account.verify_password(new_password), Ok(()));
        assert_matches!(account.verify_password(old_password), Err(()));
        Ok(())
    }

    #[test]
    fn user_authorization() -> Result<(), ()> {
        use Authorization::*;

        let user = User::Anonymous;
        assert_matches!(user.check_authorization(&Access::Anonymous), Authorized);
        assert_matches!(user.check_authorization(&Access::Authenticated), AuthenticationRequired);
        assert_matches!(user.check_authorization(&Access::Accounts(vec!["alex".into()])), AuthenticationRequired);
        assert_matches!(user.check_authorization(&Access::Accounts(vec!["morgan".into()])), AuthenticationRequired);

        let user = User::SingleUser;
        assert_matches!(user.check_authorization(&Access::Anonymous), Authorized);
        assert_matches!(user.check_authorization(&Access::Authenticated), Authorized);
        assert_matches!(user.check_authorization(&Access::Accounts(vec!["alex".into()])), Authorized);
        assert_matches!(user.check_authorization(&Access::Accounts(vec!["morgan".into()])), Authorized);

        let user = User::Account("alex".into());
        assert_matches!(user.check_authorization(&Access::Anonymous), Authorized);
        assert_matches!(user.check_authorization(&Access::Authenticated), Authorized);
        assert_matches!(user.check_authorization(&Access::Accounts(vec!["alex".into()])), Authorized);
        assert_matches!(user.check_authorization(&Access::Accounts(vec!["morgan".into()])), Unauthorized);
        Ok(())
    }
}
