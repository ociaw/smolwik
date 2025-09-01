use axum_core::body::Body;
use axum_core::response::{IntoResponse, Response};
use tera::Context;

pub struct TemplateResponse {
    pub template: &'static str,
    pub context: Context,
}

impl TemplateResponse {
    pub fn from_template(template: &'static str, context: Context) -> TemplateResponse {
        TemplateResponse {
            template,
            context,
        }
    }
}

impl IntoResponse for TemplateResponse {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        let extensions = response.extensions_mut();
        extensions.insert(self.template);
        extensions.insert(self.context);
        response
    }
}
