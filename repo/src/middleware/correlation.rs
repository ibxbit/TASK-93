use rocket::{
    fairing::{Fairing, Info, Kind},
    http::Header,
    Data, Request, Response,
};
use uuid::Uuid;

/// Request-local store for the correlation ID.
///
/// Stored via `Request::local_cache` so both fairings and route handlers can
/// read it without passing it explicitly through every function call.
pub struct CorrelationId(pub String);

/// Fairing that stamps every request with a correlation ID.
///
/// Behaviour:
/// - If the caller already supplies `X-Correlation-ID`, that value is reused
///   (useful for end-to-end tracing across services).
/// - Otherwise a new UUIDv4 is generated.
/// - The final ID is echoed back in the `X-Correlation-ID` response header.
pub struct CorrelationFairing;

#[rocket::async_trait]
impl Fairing for CorrelationFairing {
    fn info(&self) -> Info {
        Info {
            name: "Correlation ID",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        let id = req
            .headers()
            .get_one("X-Correlation-ID")
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        req.local_cache(|| CorrelationId(id));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let id = req.local_cache(|| CorrelationId(String::new())).0.clone();
        if !id.is_empty() {
            res.set_header(Header::new("X-Correlation-ID", id));
        }
    }
}
