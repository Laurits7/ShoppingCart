use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

/// API error, rendered as an RFC 9457 `application/problem+json` document.
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    /// Also used for things the user may not see, so existence never leaks (SPEC §7.3).
    NotFound,
    Conflict(String),
    TooManyRequests,
    /// Something outside (e.g. Open Food Facts) is off or unreachable.
    Unavailable(String),
    Internal(anyhow::Error),
}

pub type ApiResult<T> = Result<T, AppError>;

/// RFC 9457 problem details, the body of every error response (documentation only).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub struct Problem {
    r#type: String,
    title: String,
    status: u16,
    detail: Option<String>,
}

pub fn bad(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        AppError::Internal(e.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, detail) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, Some(m)),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, None),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, Some(m)),
            AppError::NotFound => (StatusCode::NOT_FOUND, None),
            AppError::Conflict(m) => (StatusCode::CONFLICT, Some(m)),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                Some("Too many attempts, try again later".into()),
            ),
            AppError::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, Some(m)),
            AppError::Internal(e) => {
                tracing::error!("internal error: {e:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, None)
            }
        };
        let body = json!({
            "type": "about:blank",
            "title": status.canonical_reason().unwrap_or("Error"),
            "status": status.as_u16(),
            "detail": detail,
        });
        (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(body),
        )
            .into_response()
    }
}

/// A failed insert: a duplicate id (e.g. a client reusing an id that belongs to something
/// else) is a 409, anything else an internal error.
pub fn insert_err(e: sqlx::Error) -> AppError {
    match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::Conflict("id already in use".into())
        }
        e => e.into(),
    }
}
