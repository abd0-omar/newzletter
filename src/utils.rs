use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub fn e500<T>(e: T) -> Response
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    tracing::error!(cause_chain = ?e);
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

pub fn e400<T>(e: T) -> Response
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    tracing::error!(cause_chain = ?e);
    StatusCode::BAD_REQUEST.into_response()
}
