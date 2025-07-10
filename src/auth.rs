use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Username(String);

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Account {
    pub username: Username,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum User {
    /// No user is authenticated.
    Anonymous,
    /// The user is authenticated in single-user mode.
    SingleUser,
    /// The user is authenticated with the specified username.
    Account(Username),
}

impl User {
    pub fn has_access(&self, access: &Access) -> bool {
        // If in single user mode, the authenticated user can always access everything.
        if self == &User::SingleUser { return true }
        match access {
            // Anonymous access means anyone can access this.
            Access::Anonymous => true,
            // Authenticated means that any authenticated user has access.
            Access::Authenticated => matches!(self, User::Account(_)),
            // The authenticated user must have a user name that matches one of the specified names.
            Access::Accounts(names) => matches!(self, User::Account(name) if names.contains(name))
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
