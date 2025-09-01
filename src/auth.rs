use std::fmt::{Display, Formatter};
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::SignedCookieJar;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Username(String);

impl Display for Username {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub username: Username,
    password: String,
}

impl Account {
    pub fn new(username: Username, password: &str) -> Account {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2
        };

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt).expect("Password hashing should be infallible.");

        Account { username, password: hash.to_string() }
    }

    pub fn verify_password(&self, password: &str) -> Result<(),()> {
        use argon2::{
            password_hash::{PasswordHash, PasswordVerifier},
            Argon2
        };

        let password_hash = PasswordHash::new(&self.password).map_err(|_| ())?;
        let argon2 = Argon2::default();
        argon2.verify_password(password.as_bytes(), &password_hash).map_err(|_| ())
    }

    pub fn set_password(&mut self, password: &str) {
        self.password = hash_password(password);
    }
}

pub fn verify_password(password: &str, existing_hash: &str) -> Result<(),()> {
    use argon2::{
        password_hash::{PasswordHash, PasswordVerifier},
        Argon2
    };

    let existing_hash = PasswordHash::new(existing_hash).map_err(|_| ())?;
    let argon2 = Argon2::default();
    argon2.verify_password(password.as_bytes(), &existing_hash).map_err(|_| ())
}

pub fn hash_password(password: &str) -> String {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt).expect("Password hashing should be infallible.");

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

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            User::Anonymous => write!(f, "Anonymous"),
            User::SingleUser => write!(f, "Single User"),
            User::Account(username) => write!(f, "{username}"),
        }
    }
}

impl From<SignedCookieJar> for User {
    fn from(value: SignedCookieJar) -> Self {
        Self::from(&value)
    }
}

impl From<&SignedCookieJar> for User {
    fn from(value: &SignedCookieJar) -> Self {
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
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum Authorization {
    /// The User is authorized to access the page.
    Authorized,
    /// The User is authenticated, but is not allowed to access the page.
    Unauthorized,
    /// The User is not authenticated, but page requires authentication.
    AuthenticationRequired,
}
