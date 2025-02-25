use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, StatusCode},
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

pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset: u64,    // Seconds until rate limit resets
    pub limited: bool, // Whether the request is rate limited
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    // Check if a request should be rate limited and return rate limit info
    pub fn check_rate_limit(&self, ip: &str) -> RateLimitInfo {
        let mut requests = self.requests.lock().unwrap();
        let now = Instant::now();

        // If IP exists in map, check if it's within rate limits
        if let Some((count, time)) = requests.get_mut(ip) {
            // If last request was outside our window, reset counter
            if now.duration_since(*time) > self.window {
                *count = 1;
                *time = now;

                return RateLimitInfo {
                    limit: self.max_requests,
                    remaining: self.max_requests - 1,
                    reset: self.window.as_secs(),
                    limited: false,
                };
            }

            // Calculate time until reset
            let elapsed = now.duration_since(*time);
            let reset_in = if elapsed >= self.window {
                0
            } else {
                self.window.as_secs() - elapsed.as_secs()
            };

            // If within window but over limit, reject
            if *count >= self.max_requests {
                return RateLimitInfo {
                    limit: self.max_requests,
                    remaining: 0,
                    reset: reset_in,
                    limited: true,
                };
            }

            // Within window and under limit, increment and accept
            *count += 1;

            return RateLimitInfo {
                limit: self.max_requests,
                remaining: self.max_requests - *count,
                reset: reset_in,
                limited: false,
            };
        } else {
            // First request from this IP
            requests.insert(ip.to_string(), (1, now));

            return RateLimitInfo {
                limit: self.max_requests,
                remaining: self.max_requests - 1,
                reset: self.window.as_secs(),
                limited: false,
            };
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

    // Check rate limit and get rate limit info
    let rate_limit_info = rate_limiter.check_rate_limit(&ip);

    // If rate limited, return 429 with headers
    if rate_limit_info.limited {
        // Create a response with rate limit headers
        let mut response = (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();

        // Add rate limit headers
        let headers = response.headers_mut();
        headers.insert(
            "X-RateLimit-Limit",
            HeaderValue::from(rate_limit_info.limit),
        );
        headers.insert("X-RateLimit-Remaining", HeaderValue::from(0));
        headers.insert(
            "X-RateLimit-Reset",
            HeaderValue::from(rate_limit_info.reset),
        );

        return response;
    }

    // Proceed with the request
    let mut response = next.run(req).await;

    // Add rate limit headers to the successful response
    let headers = response.headers_mut();
    headers.insert(
        "X-RateLimit-Limit",
        HeaderValue::from(rate_limit_info.limit),
    );
    headers.insert(
        "X-RateLimit-Remaining",
        HeaderValue::from(rate_limit_info.remaining),
    );
    headers.insert(
        "X-RateLimit-Reset",
        HeaderValue::from(rate_limit_info.reset),
    );

    response
}
