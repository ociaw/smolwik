use axum_core::body::Body;
use axum_core::response::{IntoResponse, Response};
use tera::Context;
use crate::{AppState, ErrorMessage};
use crate::auth::User;

pub struct TemplateResponse {
    pub state: AppState,
    pub user: User,
    pub template: &'static str,
    pub context: Context,
    pub error: Option<ErrorMessage>,
}

impl IntoResponse for TemplateResponse {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::empty());
        let extensions = response.extensions_mut();
        extensions.insert(self.state);
        extensions.insert(self.user);
        extensions.insert(self.template);
        extensions.insert(self.context);
        if let Some(error) = self.error {
            extensions.insert(error);
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
            context: Context::new(),
            error: Some(error),
        }
    }

    pub fn from_template(state: impl Into<AppState>, user: impl Into<User>, template: &'static str, context: Context) -> TemplateResponse {
        TemplateResponse {
            state: state.into(),
            user: user.into(),
            template,
            context,
            error: None,
        }
    }
}