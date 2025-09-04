use crate::auth::{Session, User};
use crate::{AntiCsrfForm, AppState, ErrorResponse};
use axum_core::extract::{FromRef, FromRequest, FromRequestParts};
use axum_extra::extract::SignedCookieJar;
use axum_extra::extract::cookie::Key;
use http::Request;
use http::request::Parts;
use serde::de::DeserializeOwned;
use std::convert::Infallible;
use std::ops::Deref;

impl<S> FromRequestParts<S> for Session
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let jar: SignedCookieJar<Key> = SignedCookieJar::from_request_parts(parts, &state).await.unwrap();

        Ok(match jar.get("session") {
            None => Session::default(),
            Some(cookie) => serde_json::from_str(cookie.value()).unwrap_or(Session::default()),
        })
    }
}

impl<S> FromRequestParts<S> for User
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::RequestPartsExt;
        let session = parts.extract_with_state::<Session, S>(state).await?;
        Ok(session.user)
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
    AppState: FromRef<S>,
    T: DeserializeOwned + AntiCsrfForm,
    S: Send + Sync,
{
    type Rejection = ErrorResponse;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Form<T>, Self::Rejection> {
        use axum::RequestPartsExt;
        let (mut parts, body) = req.into_parts();
        let session = parts.extract_with_state::<Session, S>(state).await.unwrap();
        let form = axum::extract::Form::from_request(Request::from_parts(parts, body), state)
            .await
            .map(|f: axum::Form<T>| Form(f.0))
            .map_err(|rej| ErrorResponse::from(rej))?;
        if !form.0.is_valid(session.id.as_deref()) {
            return Err(ErrorResponse::bad_request());
        }
        Ok(form)
    }
}

impl<T> Deref for Form<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
