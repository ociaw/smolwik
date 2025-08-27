use std::error::Error;
use axum::extract::rejection::FormRejection;
use crate::article::{ArticleReadError, ArticleWriteError};
use axum::http::StatusCode;
use crate::config::ConfigReadError;
use crate::filesystem::FileWriteError;

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

    pub fn bad_request_with_details(details: impl Into<String>) -> Self {
        ErrorMessage {
            status_code: StatusCode::BAD_REQUEST,
            title: "Bad request".to_owned(),
            details: details.into(),
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

    pub fn account_not_found(username: &crate::auth::Username) -> Self {
        ErrorMessage {
            status_code: StatusCode::NOT_FOUND,
            title: "Account not found".to_owned(),
            details: format!("An account could not be found with the provided username: {}", username),
        }
    }

    pub fn path_not_found(path: &str) -> Self {
        ErrorMessage {
            status_code: StatusCode::NOT_FOUND,
            title: "Page not found".to_owned(),
            details: format!("The requested path could not be found: {}", path),
        }
    }

    pub fn conflict(title: impl Into<String>, details: impl Into<String>) -> Self {
        ErrorMessage {
            status_code: StatusCode::CONFLICT,
            title: title.into(),
            details: details.into(),
        }
    }

    pub fn unsupported_media_type(details: impl Into<String>) -> Self {
        ErrorMessage {
            status_code: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            title: "Unsupported media type".to_owned(),
            details: details.into(),
        }
    }

    pub fn unprocessable_entity(details: impl Into<String>) -> Self {
        ErrorMessage {
            status_code: StatusCode::UNPROCESSABLE_ENTITY,
            title: "Unprocessable entity".to_owned(),
            details: details.into(),
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

impl From<ArticleWriteError> for ErrorMessage {
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

impl From<ArticleReadError> for ErrorMessage {
    fn from(value: ArticleReadError) -> Self {
        match value {
            // For transient IO errors, we don't want to save the response, so we return an error.
            ArticleReadError::IoError(err) => Self::internal_error(err.to_string()),
            // These errors are not transient, and need to be fixed in some way. We render the
            // article with an error message and return that.
            ArticleReadError::NotFound => Self::path_not_found(""),
            _ => Self::internal_error(value.to_string()),
        }
    }
}

impl From<ConfigReadError> for ErrorMessage {
    fn from(value: ConfigReadError) -> Self {
        Self::internal_error(value.to_string())
    }
}

impl From<FileWriteError> for ErrorMessage {
    fn from(value: FileWriteError) -> Self {
        match value {
            FileWriteError::ConflictingWriteInProgress { filepath, tmp_path: _ } => {
                let filename = filepath.file_name().expect("File name should always be valid here.").to_string_lossy();
                ErrorMessage::conflict(
                    "Conflicting file update in progress",
                    format!("A conflicting update was made to {filename} while saving this. Please try again.")
            )},
            FileWriteError::UnhandlableWriteError { source, filepath: _ } => Self::internal_error(source.to_string())
        }
    }
}

impl From<tera::Error> for ErrorMessage {
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

impl From<FormRejection> for ErrorMessage {
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