use crate::ddos::{ActivityDetector, GlobalRateLimiter, IpBlacklist};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{net::IpAddr, str::FromStr, sync::Arc};

// DDoS protection middleware
pub async fn ddos_protection_middleware(
    State(ddos_state): State<DdosProtectionState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // Get client IP address
    let ip_str = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .trim();

    // Parse IP address
    let ip = match IpAddr::from_str(ip_str) {
        Ok(ip) => ip,
        Err(_) => {
            // Invalid IP - this is suspicious
            return (StatusCode::BAD_REQUEST, "Invalid IP address").into_response();
        }
    };

    // Check if IP is blacklisted
    if ddos_state.blacklist.is_blacklisted(&ip) {
        return (
            StatusCode::FORBIDDEN,
            "IP address blocked due to suspicious activity",
        )
            .into_response();
    }

    // Track request path for diversity analysis
    let path = req.uri().path().to_string();
    if ddos_state.detector.track_path(&ip, &path) {
        // Too many unique paths - blacklist
        ddos_state.blacklist.blacklist_ip(ip);
        return (
            StatusCode::FORBIDDEN,
            "Too many unique requests - scanning detected",
        )
            .into_response();
    }

    // Track request timing
    if ddos_state.detector.track_timing(&ip) {
        // Requests coming too fast - blacklist
        ddos_state.blacklist.blacklist_ip(ip);
        return (
            StatusCode::FORBIDDEN,
            "Request rate abnormal - automated tools detected",
        )
            .into_response();
    }

    // Track in global limiter
    let under_attack = ddos_state.global_limiter.track_request();

    // If we're under attack and this isn't a whitelisted IP, apply stricter limits
    if under_attack && !is_whitelisted(&ip) {
        // In attack mode, we might want to be more selective
        // For example, reject some percentage of requests, or prioritize certain clients
        // For now, we'll just reject with a 503 Service Unavailable
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Server is currently under high load. Please try again later.",
        )
            .into_response();
    }

    // Proceed with request
    let response = next.run(req).await;

    // Check response status to track failures
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        if ddos_state.detector.track_failure(&ip) {
            // Too many failures, blacklist the IP
            ddos_state.blacklist.blacklist_ip(ip);
        }
    } else {
        // Reset failure count on success
        ddos_state.detector.reset_failures(&ip);
    }

    // Extract headers and body from the response
    let (parts, body) = response.into_parts();

    // Create a new response with the same parts
    let mut response_with_headers = Response::from_parts(parts, body);

    // Add security headers
    let headers = response_with_headers.headers_mut();
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );

    if under_attack {
        headers.insert("X-Under-Attack-Mode", HeaderValue::from_static("true"));
    }

    response_with_headers
}

// Check if an IP is whitelisted (e.g., your own servers, trusted clients)
fn is_whitelisted(ip: &IpAddr) -> bool {
    // You would implement your own logic here
    // For example, check against a list of known good IPs
    match ip {
        // Example: whitelist localhost and your own network
        IpAddr::V4(v4) => {
            // Localhost
            v4.octets()[0] == 127 ||
            // Your office/data center IPs
            (v4.octets()[0] == 10) ||  // 10.0.0.0/8
            (v4.octets()[0] == 172 && v4.octets()[1] >= 16 && v4.octets()[1] <= 31)
            // 172.16.0.0/12
        }
        IpAddr::V6(_) => {
            // Implement IPv6 whitelist logic if needed
            false
        }
    }
}

// State for DDoS protection
#[derive(Clone)]
pub struct DdosProtectionState {
    pub global_limiter: Arc<GlobalRateLimiter>,
    pub blacklist: Arc<IpBlacklist>,
    pub detector: Arc<ActivityDetector>,
}

impl DdosProtectionState {
    pub fn new() -> Self {
        Self {
            global_limiter: Arc::new(GlobalRateLimiter::new(1000, 10)), // 1000 requests per 10 seconds globally
            blacklist: Arc::new(IpBlacklist::new(30)),                  // Block for 30 minutes
            detector: Arc::new(ActivityDetector::new()),
        }
    }

    // Start background tasks for monitoring and cleanup
    pub fn start_background_tasks(&self) -> tokio::task::JoinHandle<()> {
        let blacklist = self.blacklist.clone();
        let detector = self.detector.clone();
        let global_limiter = self.global_limiter.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;

                // Clean up blacklist
                blacklist.cleanup();

                // Clean up activity detector
                detector.cleanup();

                // Log current request rate
                let rate = global_limiter.get_current_rate();
                tracing::info!("Current global request rate: {:.2} req/s", rate);

                // If under attack, log it
                if global_limiter.is_under_attack() {
                    tracing::warn!("System is currently under attack mode");
                }
            }
        })
    }
}
