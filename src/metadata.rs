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
