use std::time::Instant;

use rocket::{
    fairing::{Fairing, Info, Kind},
    Data, Request, Response,
};

use super::correlation::CorrelationId;

/// Request-local store for the wall-clock instant the request arrived.
/// Used to compute end-to-end handler latency in the response phase.
pub struct RequestStart(pub Instant);

/// Fairing that emits structured JSON log lines for every HTTP request.
///
/// Log fields:
/// - `correlation_id` — ties request + response lines together
/// - `method`, `path` — routing context
/// - `status`         — HTTP response status (response phase only)
/// - `duration_ms`    — total handler latency in milliseconds
///
/// **Attachment order matters**: `CorrelationFairing` must be attached *before*
/// this fairing so the correlation ID is already cached when `on_request` runs.
pub struct RequestLoggerFairing;

#[rocket::async_trait]
impl Fairing for RequestLoggerFairing {
    fn info(&self) -> Info {
        Info {
            name: "Request Logger",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        // Record arrival time before anything else so latency is accurate.
        req.local_cache(|| RequestStart(Instant::now()));

        let cid = req.local_cache(|| CorrelationId(String::new())).0.as_str();

        tracing::info!(
            correlation_id = cid,
            method         = %req.method(),
            path           = %req.uri().path(),
            "request.received",
        );
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let elapsed_ms = req
            .local_cache(|| RequestStart(Instant::now()))
            .0
            .elapsed()
            .as_millis();

        let cid = req.local_cache(|| CorrelationId(String::new())).0.as_str();

        let status = res.status().code;

        if status >= 500 {
            tracing::error!(
                correlation_id = cid,
                method         = %req.method(),
                path           = %req.uri().path(),
                status,
                duration_ms    = elapsed_ms,
                "request.error",
            );
        } else if status >= 400 {
            tracing::warn!(
                correlation_id = cid,
                method         = %req.method(),
                path           = %req.uri().path(),
                status,
                duration_ms    = elapsed_ms,
                "request.client_error",
            );
        } else {
            tracing::info!(
                correlation_id = cid,
                method         = %req.method(),
                path           = %req.uri().path(),
                status,
                duration_ms    = elapsed_ms,
                "request.completed",
            );
        }
    }
}
