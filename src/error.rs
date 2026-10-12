//! The single error type every handler returns, and its mapping to HTTP responses.

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

use crate::mode::UnsupportedMode;
use crate::scoring::Rejection;
use crate::store::{InvalidName, StoreError};
use crate::token::TokenError;

/// Anything a request can fail with.
///
/// `#[from]` generates `From` impls, so `?` converts each domain error automatically, much
/// like raising a subclass of a common exception in Python.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Json(#[from] JsonRejection),
    #[error(transparent)]
    Query(#[from] QueryRejection),
    #[error(transparent)]
    Path(#[from] PathRejection),
    #[error(transparent)]
    InvalidMode(#[from] UnsupportedMode),
    #[error(transparent)]
    InvalidName(#[from] InvalidName),
    #[error(transparent)]
    InvalidToken(#[from] TokenError),
    #[error(transparent)]
    Rejected(#[from] Rejection),
    #[error("result not found")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// JSON error body: a stable machine-readable `error` code plus a human `message`.
#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: String,
    /// The specific anti-cheat rule, for [`AppError::Rejected`] only.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<Rejection>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::Json(rejection) => (rejection.status(), "invalid_body"),
            Self::Query(rejection) => (rejection.status(), "invalid_query"),
            Self::Path(rejection) => (rejection.status(), "invalid_path"),
            Self::InvalidMode(_) => (StatusCode::BAD_REQUEST, "invalid_mode"),
            Self::InvalidName(_) => (StatusCode::BAD_REQUEST, "invalid_name"),
            Self::InvalidToken(_) => (StatusCode::BAD_REQUEST, "invalid_token"),
            Self::Rejected(_) => (StatusCode::UNPROCESSABLE_ENTITY, "result_rejected"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Store(StoreError::Duplicate) => (StatusCode::CONFLICT, "already_submitted"),
            Self::Store(StoreError::Backend(error)) => {
                tracing::error!(%error, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        let message = match &self {
            // Never leak database details to clients.
            Self::Store(StoreError::Backend(_)) => "internal server error".to_owned(),
            other => other.to_string(),
        };
        let reason = match &self {
            Self::Rejected(rejection) => Some(*rejection),
            _ => None,
        };
        (
            status,
            Json(ErrorBody {
                error: code,
                message,
                reason,
            }),
        )
            .into_response()
    }
}
