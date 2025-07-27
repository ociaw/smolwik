use crate::page::{PageReadError, PageWriteError};
use axum::http::StatusCode;
use crate::config::ConfigError;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ErrorMessage {
    pub status_code: StatusCode,
    pub title: String,
    pub details: String,
}

impl ErrorMessage {
    pub fn bad_request() -> Self {
        ErrorMessage {
            status_code: StatusCode::BAD_REQUEST,
            title: "Bad request".to_owned(),
            details: "Invalid data in request.".to_owned()
        }
    }

    pub fn unauthenticated() -> Self {
        ErrorMessage {
            status_code: StatusCode::UNAUTHORIZED,
            title: "Authentication required".to_owned(),
            details: "Authentication is required to view this page, please log in.".to_owned()
        }
    }

    pub fn invalid_credentials() -> Self {
        ErrorMessage {
            status_code: StatusCode::UNAUTHORIZED,
            title: "Invalid credentials".to_owned(),
            details: "Invalid username or password provided.".to_owned()
        }
    }
    
    pub fn already_authenticated() -> Self {
        ErrorMessage {
            status_code: StatusCode::BAD_REQUEST,
            title: "Already logged in".to_owned(),
            details: "You are already logged in.".to_owned(),
        }
    }

    pub fn forbidden() -> Self {
        ErrorMessage {
            status_code: StatusCode::FORBIDDEN,
            title: "Access forbidden".to_owned(),
            details: "Access to this page is forbidden.".to_owned()
        }
    }

    pub fn not_found(path: &str) -> Self {
        ErrorMessage {
            status_code: StatusCode::NOT_FOUND,
            title: "Page not found".to_owned(),
            details: format!("The requested path could not be found: {}", path),
        }
    }

    pub fn internal_error<S>(details: S) -> Self
        where S : Into<String> {
        ErrorMessage {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            title: "An internal error occurred.".to_owned(),
            details: details.into(),
        }
    }
}

impl From<PageWriteError> for ErrorMessage {
    fn from(value: PageWriteError) -> Self {
        match value {
            PageWriteError::InvalidPath => Self::bad_request(),
            PageWriteError::IoError(err) => Self::internal_error(err.to_string())
        }
    }
}

impl From<PageReadError> for ErrorMessage {
    fn from(value: PageReadError) -> Self {
        match value {
            // For transient IO errors, we don't want to save the response, so we return an error.
            PageReadError::IoError(err) => Self::internal_error(err.to_string()),
            // These errors are not transient, and need to be fixed in some way. We render the
            // page with an error message and return that.
            PageReadError::NotFound => Self::not_found(""),
            _ => Self::internal_error(value.to_string()),
        }
    }
}

impl From<ConfigError> for ErrorMessage {
    fn from(value: ConfigError) -> Self {
        Self::internal_error(value.to_string())
    }
}

impl From<tera::Error> for ErrorMessage {
    fn from(value: tera::Error) -> Self {
        Self::internal_error(value.to_string())
    }
}
