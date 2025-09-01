use axum_core::body::Body;
use axum_core::response::{IntoResponse, Response};
use tera::Context;
use crate::ErrorMessage;

pub struct TemplateResponse {
    pub template: &'static str,
    pub context: Context,
    pub error: Option<ErrorMessage>,
}

impl TemplateResponse {
    pub fn from_template(template: &'static str, context: Context) -> TemplateResponse {
        TemplateResponse {
            template,
            context,
            error: None,
        }
    }
}

impl IntoResponse for TemplateResponse {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        let extensions = response.extensions_mut();
        extensions.insert(self.template);
        extensions.insert(self.context);
        if let Some(error) = self.error {
            extensions.insert(error);
        }
        response
    }
}

impl From<ErrorMessage> for TemplateResponse {
    fn from(value: ErrorMessage) -> Self {
        TemplateResponse {
            template: "error",
            context: Context::new(),
            error: Some(value),
        }
    }
}
