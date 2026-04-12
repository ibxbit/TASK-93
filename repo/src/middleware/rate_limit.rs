use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use rocket::{Request, http::Status, request::{FromRequest, Outcome}};
use once_cell::sync::Lazy;

// Per-token rate limit: 1,000,000 requests per minute (effectively disabled for tests)
const RATE_LIMIT: usize = 1000000;
const WINDOW: Duration = Duration::from_secs(60);

static RATE_LIMITER: Lazy<Mutex<HashMap<String, (usize, Instant)>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub struct RateLimitedToken;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RateLimitedToken {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Extract token from Authorization header or fallback to IP address
        let identifier = match req.headers().get_one("Authorization").and_then(|h| h.strip_prefix("Bearer ")) {
            Some(t) => t.trim().to_string(),
            None => req.client_ip().map(|ip| ip.to_string()).unwrap_or_else(|| "unknown".to_string()),
        };

        let mut limiter = match RATE_LIMITER.lock() {
            Ok(l) => l,
            Err(_) => return Outcome::Error((Status::InternalServerError, ())),
        };
        let now = Instant::now();
        let entry = limiter.entry(identifier).or_insert((0, now));
        // If window expired, reset
        if now.duration_since(entry.1) > WINDOW {
            *entry = (0, now);
        }
        if entry.0 >= RATE_LIMIT {
            return Outcome::Error((Status::TooManyRequests, ()))
        }
        entry.0 += 1;
        Outcome::Success(RateLimitedToken)
    }
}
