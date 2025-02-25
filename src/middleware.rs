use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::AppState;

// Define a rate limiting struct
pub struct RateLimiter {
    // Map IP addresses to a tuple of (request count, last request time)
    requests: Mutex<HashMap<String, (u32, Instant)>>,
    // Maximum requests per minute
    max_requests: u32,
    // Window duration for rate limiting
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    // Check if a request should be rate limited
    pub fn check_rate_limit(&self, ip: &str) -> bool {
        let mut requests = self.requests.lock().unwrap();
        let now = Instant::now();

        // If IP exists in map, check if it's within rate limits
        if let Some((count, time)) = requests.get_mut(ip) {
            // If last request was outside our window, reset counter
            if now.duration_since(*time) > self.window {
                *count = 1;
                *time = now;
                return false;
            }

            // If within window but over limit, reject
            if *count >= self.max_requests {
                return true;
            }

            // Within window and under limit, increment and accept
            *count += 1;
            *time = now;
            false
        } else {
            // First request from this IP
            requests.insert(ip.to_string(), (1, now));
            false
        }
    }

    // Clean up old entries (periodically call this)
    pub fn cleanup(&self) {
        let mut requests = self.requests.lock().unwrap();
        let now = Instant::now();
        requests.retain(|_, (_, time)| now.duration_since(*time) <= self.window);
    }
}

// Authentication middleware
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // Extract authorization header
    let auth_header = req.headers().get("Authorization");

    // Check if Authorization header exists and has the correct format
    match auth_header {
        Some(header) => {
            if let Ok(auth_str) = header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = &auth_str[7..]; // Skip "Bearer " prefix

                    // Compare with the API token from your environment
                    if token == state.env.api_token.secret_str() {
                        // Token is valid, proceed to next middleware or handler
                        return next.run(req).await;
                    }
                }
            }
            // Invalid token
            (StatusCode::UNAUTHORIZED, "Invalid API token").into_response()
        }
        // No authorization header
        None => (StatusCode::UNAUTHORIZED, "API token required").into_response(),
    }
}

// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(rate_limiter): State<Arc<RateLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // Get client IP address
    // In production, you might need to handle X-Forwarded-For headers
    let ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .trim()
        .to_string();

    // Check rate limit
    if rate_limiter.check_rate_limit(&ip) {
        // Rate limit exceeded
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    // Proceed with the request
    next.run(req).await
}
