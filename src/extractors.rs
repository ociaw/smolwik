use crate::auth::User;
use crate::template::TemplateResponse;
use crate::{AppState, ErrorMessage};
use axum_core::extract::{FromRef, FromRequest, FromRequestParts};
use axum_extra::extract::cookie::Key;
use axum_extra::extract::SignedCookieJar;
use http::request::Parts;
use http::Request;
use serde::de::DeserializeOwned;
use std::convert::Infallible;
use std::ops::Deref;

impl<S> FromRequestParts<S> for User
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let jar: SignedCookieJar<Key> = SignedCookieJar::from_request_parts(parts, &state).await.unwrap();

        Ok(match jar.get("user") {
            None => User::Anonymous,
            Some(cookie) => serde_json::from_str(cookie.value()).unwrap_or(User::Anonymous)
        })
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(input: &AppState) -> Self {
        Key::from(&input.config.secret_key)
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Form<T>(pub T);

impl<T, S> FromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = TemplateResponse;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Form<T>, Self::Rejection> {
        let (parts, body) = req.into_parts();
        axum::extract::Form::from_request(Request::from_parts(parts, body), state).await
            .map(|f| Form(f.0))
            .map_err(|rej| ErrorMessage::from(rej).into())
    }
}

impl<T> Deref for Form<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
