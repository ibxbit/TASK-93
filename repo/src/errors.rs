use rocket::{
    http::Status,
    response::{self, Responder},
    serde::json::Json,
    Request,
};
use serde::Serialize;
use thiserror::Error;

use crate::middleware::correlation::CorrelationId;

// ── Error variants ────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type AppResult<T> = Result<T, AppError>;

// ── Wire format ───────────────────────────────────────────────────────────────

/// JSON body returned on every error response.
#[derive(Serialize)]
pub struct ErrorBody {
    /// Machine-readable error code.
    pub code: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Echoes the request correlation ID to simplify log correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

// ── Rocket Responder ──────────────────────────────────────────────────────────

impl<'r> Responder<'r, 'static> for AppError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        let (status, code) = match &self {
            AppError::NotFound(_) => (Status::NotFound, "NOT_FOUND"),
            AppError::BadRequest(_) => (Status::BadRequest, "BAD_REQUEST"),
            AppError::UnprocessableEntity(_) => {
                (Status::UnprocessableEntity, "UNPROCESSABLE_ENTITY")
            }
            AppError::Conflict(_) => (Status::Conflict, "CONFLICT"),
            AppError::Unauthorized(_) => (Status::Unauthorized, "UNAUTHORIZED"),
            AppError::Forbidden(_) => (Status::Forbidden, "FORBIDDEN"),
            AppError::Internal(_) | AppError::Config(_) => {
                (Status::InternalServerError, "INTERNAL_ERROR")
            }
        };

        // Pull the correlation ID that was stamped by CorrelationFairing.
        let cid = req.local_cache(|| CorrelationId(String::new())).0.clone();

        // Internal/Config variants must never leak implementation details to clients.
        let message = match &self {
            AppError::Internal(detail) | AppError::Config(detail) => {
                // Always print the full error details to stdout for debugging
                use std::io::Write;
                eprintln!("[INTERNAL_ERROR] correlation_id={cid} error={detail}");
                let _ = std::io::stderr().flush();
                tracing::error!(error = %detail, correlation_id = %cid, "internal_error");
                format!("INTERNAL_ERROR: {detail} (correlation_id: {cid})")
            }
            other => other.to_string(),
        };

        let body = ErrorBody {
            code,
            message,
            correlation_id: if cid.is_empty() { None } else { Some(cid) },
        };

        let mut response = Json(body).respond_to(req)?;
        response.set_status(status);
        Ok(response)
    }
}
