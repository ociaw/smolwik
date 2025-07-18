use serde::{Deserialize, Serialize};
use crate::auth::Access;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
    /// The title of this page.
    pub title: String,
    /// Access to edit this page.
    pub edit_access: Access,
    /// Access to view this page.
    pub view_access: Access,
}

impl Metadata {
    pub fn bad_request() -> Metadata {
        Metadata {
            title: "Bad request".to_owned(),
            edit_access: Access::Authenticated,
            view_access: Access::Anonymous,
        }
    }
    pub fn not_found() -> Metadata {
        Metadata {
            title: "Page not found".to_owned(),
            edit_access: Access::Authenticated,
            view_access: Access::Anonymous,
        }
    }
    pub fn internal_error() -> Metadata {
        Metadata {
            title: "An error occurred when opening this page.".to_owned(),
            edit_access: Access::Authenticated,
            view_access: Access::Anonymous,
        }
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            title: String::new(),
            edit_access: Access::Authenticated,
            view_access: Access::Anonymous,
        }
    }
}
