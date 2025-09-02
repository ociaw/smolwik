use std::error::Error;
use axum::extract::rejection::FormRejection;
use crate::article::{ArticleReadError, ArticleWriteError};
use axum::http::StatusCode;
use axum_core::body::Body;
use axum_core::response::{IntoResponse, Response};
use tera::Context;
use crate::config::ConfigReadError;
use crate::filesystem::FileWriteError;
use crate::routes::discovery::DiscoveryTreeError;

pub struct TemplatedResponse {
    pub template: &'static str,
    pub context: Context,
}

impl TemplatedResponse {
    pub fn new(template: &'static str, context: Context) -> TemplatedResponse {
        TemplatedResponse {
            template,
            context,
        }
    }
}

impl IntoResponse for TemplatedResponse {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        let extensions = response.extensions_mut();
        extensions.insert(self.template);
        extensions.insert(self.context);
        response
    }
}


#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ErrorResponse {
    pub status_code: StatusCode,
    pub title: String,
    pub details: String,
}

impl ErrorResponse {
    pub fn bad_request() -> Self {
        ErrorResponse {
            status_code: StatusCode::BAD_REQUEST,
            title: "Bad request".to_owned(),
            details: "Invalid data in request.".to_owned()
        }
    }

    pub fn bad_request_with_details(details: impl Into<String>) -> Self {
        ErrorResponse {
            status_code: StatusCode::BAD_REQUEST,
            title: "Bad request".to_owned(),
            details: details.into(),
        }
    }

    pub fn unauthenticated() -> Self {
        ErrorResponse {
            status_code: StatusCode::UNAUTHORIZED,
            title: "Authentication required".to_owned(),
            details: "Authentication is required to view this page, please log in.".to_owned()
        }
    }

    pub fn invalid_credentials() -> Self {
        ErrorResponse {
            status_code: StatusCode::UNAUTHORIZED,
            title: "Invalid credentials".to_owned(),
            details: "Invalid username or password provided.".to_owned()
        }
    }
    
    pub fn already_authenticated() -> Self {
        ErrorResponse {
            status_code: StatusCode::BAD_REQUEST,
            title: "Already logged in".to_owned(),
            details: "You are already logged in.".to_owned(),
        }
    }

    pub fn forbidden() -> Self {
        ErrorResponse {
            status_code: StatusCode::FORBIDDEN,
            title: "Access forbidden".to_owned(),
            details: "Access to this page is forbidden.".to_owned()
        }
    }

    pub fn account_not_found(username: &crate::auth::Username) -> Self {
        ErrorResponse {
            status_code: StatusCode::NOT_FOUND,
            title: "Account not found".to_owned(),
            details: format!("An account could not be found with the provided username: {}", username),
        }
    }

    pub fn path_not_found(path: &str) -> Self {
        ErrorResponse {
            status_code: StatusCode::NOT_FOUND,
            title: "Page not found".to_owned(),
            details: format!("The requested path could not be found: {}", path),
        }
    }

    pub fn conflict(title: impl Into<String>, details: impl Into<String>) -> Self {
        ErrorResponse {
            status_code: StatusCode::CONFLICT,
            title: title.into(),
            details: details.into(),
        }
    }

    pub fn unsupported_media_type(details: impl Into<String>) -> Self {
        ErrorResponse {
            status_code: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            title: "Unsupported media type".to_owned(),
            details: details.into(),
        }
    }

    pub fn unprocessable_entity(details: impl Into<String>) -> Self {
        ErrorResponse {
            status_code: StatusCode::UNPROCESSABLE_ENTITY,
            title: "Unprocessable entity".to_owned(),
            details: details.into(),
        }
    }

    pub fn internal_error<S>(details: S) -> Self
        where S : Into<String> {
        ErrorResponse {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            title: "An internal error occurred.".to_owned(),
            details: details.into(),
        }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = self.status_code;
        let extensions = response.extensions_mut();
        extensions.insert(self);
        response
    }
}

impl From<ArticleWriteError> for ErrorResponse {
    fn from(value: ArticleWriteError) -> Self {
        match value {
            ArticleWriteError::InvalidPath { source: _, path } => Self::bad_request_with_details(format!("Not a valid path: <code>{path}</code>")),
            ArticleWriteError::ConflictingWriteInProgress { path } => Self::conflict(
                "Conflicting article update in progress",
                format!("A conflicting update was made to the article at <code>{path}</code> while saving this. Saving will clobber those changes.")
            ),
            ArticleWriteError::UnhandlableWriteError { source: _, path: _ } => Self::internal_error(value.to_string())
        }
    }
}

impl From<ArticleReadError> for ErrorResponse {
    fn from(value: ArticleReadError) -> Self {
        match value {
            // For transient IO errors, we don't want to cache the response, so we return an error.
            ArticleReadError::IoError { source, path: _ } => Self::internal_error(source.to_string()),
            // These errors are not transient, and need to be fixed in some way. We render the
            // article with an error message and return that.
            ArticleReadError::NotFound { path } => Self::path_not_found(&path),
            _ => Self::internal_error(value.to_string()),
        }
    }
}

impl From<DiscoveryTreeError> for ErrorResponse {
    fn from(value: DiscoveryTreeError) -> Self {
        match value {
            // For transient IO errors, we don't want to cache the response, so we return an error.
            DiscoveryTreeError::DirectoryOpenError { source } => Self::internal_error(source.to_string()),
            DiscoveryTreeError::EntryOpenError { source } => Self::internal_error(source.to_string()),
            DiscoveryTreeError::ArticleReadError { source } => Self::internal_error(source.to_string()),
        }
    }
}

impl From<ConfigReadError> for ErrorResponse {
    fn from(value: ConfigReadError) -> Self {
        Self::internal_error(value.to_string())
    }
}

impl From<FileWriteError> for ErrorResponse {
    fn from(value: FileWriteError) -> Self {
        match value {
            FileWriteError::ConflictingWriteInProgress { filepath, tmp_path: _ } => {
                let filename = filepath.file_name().expect("File name should always be valid here.").to_string_lossy();
                ErrorResponse::conflict(
                    "Conflicting file update in progress",
                    format!("A conflicting update was made to {filename} while saving this. Please try again.")
            )},
            FileWriteError::UnhandlableWriteError { source, filepath: _ } => Self::internal_error(source.to_string())
        }
    }
}

impl From<tera::Error> for ErrorResponse {
    fn from(value: tera::Error) -> Self {
        let message = value.to_string();
        // Try to improve the error message by getting the underlying cause. Tera wraps the useful
        // error message with an unhelpful error.
        if let Some(source) = value.source() && message.starts_with("Failed to render ") {
            Self::internal_error(source.to_string())
        }
        else {
            Self::internal_error(message)
        }
    }
}

impl From<FormRejection> for ErrorResponse {
    fn from(value: FormRejection) -> Self {
        match value {
            FormRejection::InvalidFormContentType(err) => Self::unsupported_media_type(err.body_text()),
            FormRejection::FailedToDeserializeForm(err) => Self::bad_request_with_details(err.body_text()),
            FormRejection::FailedToDeserializeFormBody(err) => Self::unprocessable_entity(err.body_text()),
            FormRejection::BytesRejection(err) => Self::bad_request_with_details(err.body_text()),
            err => Self::bad_request_with_details(err.body_text()),
        }
    }
}