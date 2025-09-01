use axum_core::body::Body;
use axum_core::response::{IntoResponse, Response};
use tera::Context;
use crate::{AppState, ErrorMessage};
use crate::auth::User;

pub struct TemplateResponse {
    pub state: AppState,
    pub user: User,
    pub template: &'static str,
    pub error: Option<ErrorMessage>,
    pub context: Option<Context>,
}

impl IntoResponse for TemplateResponse {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        let extensions = response.extensions_mut();
        extensions.insert(self.state);
        extensions.insert(self.user);
        extensions.insert(self.template);
        if let Some(error) = self.error {
            extensions.insert(error);
        }
        if let Some(context) = self.context {
            extensions.insert(context);
        }
        response
    }
}

impl TemplateResponse {
    pub fn from_error(state: impl Into<AppState>, user: impl Into<User>, error: ErrorMessage) -> TemplateResponse {
        TemplateResponse {
            state: state.into(),
            user: user.into(),
            template: "error",
            error: Some(error),
            context: None,
        }
    }

    pub fn from_template(state: impl Into<AppState>, user: impl Into<User>, template: &'static str, context: Option<Context>) -> TemplateResponse {
        TemplateResponse {
            state: state.into(),
            user: user.into(),
            template,
            error: None,
            context,
        }
    }
}