use std::io::{Error, ErrorKind};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use crate::metadata::Metadata;
use crate::render;

#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub status_code: StatusCode,
    pub html: Html<String>,
}

impl RenderedPage {
    pub fn ok(html: String) -> RenderedPage {
        RenderedPage {
            status_code: StatusCode::OK,
            html: Html(html),
        }
    }

    pub fn not_found(html: String) -> RenderedPage {
        RenderedPage {
            status_code: StatusCode::NOT_FOUND,
            html: Html(html),
        }
    }

    pub fn internal_error(html: String) -> RenderedPage {
        RenderedPage {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            html: Html(html),
        }
    }
}

impl IntoResponse for RenderedPage {
    fn into_response(self) -> Response {
        let mut response = self.html.into_response();
        *response.status_mut() = self.status_code;
        response
    }
}

impl From<Error> for RenderedPage {
    fn from(value: Error) -> Self {
        match value.kind() {
            ErrorKind::NotFound | ErrorKind::IsADirectory | ErrorKind::InvalidInput | ErrorKind::InvalidFilename => RenderedPage::not_found(render::not_found()),
            // TODO: Log this case
            _ => RenderedPage::internal_error(render::generic_error()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawPage {
    pub metadata: Metadata,
    pub markdown: String,
}
