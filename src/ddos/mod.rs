pub mod middleware;

pub use middleware::ddos_protection_middleware;

use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

// Global rate limiter to track overall API load
pub struct GlobalRateLimiter {
    // Track total requests per window
    total_requests: Mutex<u32>,
    // Current rate (requests per second)
    current_rate: Mutex<f64>,
    // Last reset time
    last_reset: Mutex<Instant>,
    // Maximum global requests per window
    max_requests: u32,
    // Window duration for global rate limiting
    window: Duration,
    // Is the system currently under attack?
    under_attack: Mutex<bool>,
}

impl GlobalRateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            total_requests: Mutex::new(0),
            current_rate: Mutex::new(0.0),
            last_reset: Mutex::new(Instant::now()),
            max_requests,
            window: Duration::from_secs(window_seconds),
            under_attack: Mutex::new(false),
        }
    }

    // Track a new request and determine if we're under DDoS
    pub fn track_request(&self) -> bool {
        let now = Instant::now();

        // Lock the mutexes
        let mut total = self.total_requests.lock().unwrap();
        let mut last_reset = self.last_reset.lock().unwrap();
        let mut current_rate = self.current_rate.lock().unwrap();
        let mut under_attack = self.under_attack.lock().unwrap();

        // Check if we need to reset the window
        let elapsed = now.duration_since(*last_reset);
        if elapsed >= self.window {
            // Calculate the current rate
            *current_rate = *total as f64 / elapsed.as_secs_f64();

            // Reset the counter
            *total = 1;
            *last_reset = now;

            // Check if we're under attack
            if *current_rate > self.max_requests as f64 / self.window.as_secs_f64() * 0.8 {
                tracing::warn!(
                    "Possible DDoS attack detected! Current rate: {:.2} req/s",
                    *current_rate
                );
                *under_attack = true;
            } else {
                *under_attack = false;
            }
        } else {
            // Increment the counter
            *total += 1;

            // Check if we're over the maximum global limit
            if *total > self.max_requests {
                tracing::warn!(
                    "Global rate limit exceeded! {} requests in {}s",
                    *total,
                    elapsed.as_secs()
                );
                *under_attack = true;
                return true;
            }
        }

        // Return whether we're currently under attack
        *under_attack
    }

    // Get the current status of the system
    pub fn is_under_attack(&self) -> bool {
        *self.under_attack.lock().unwrap()
    }

    // Get current rate
    pub fn get_current_rate(&self) -> f64 {
        *self.current_rate.lock().unwrap()
    }
}

// IP Blacklist for temporarily blocking misbehaving clients
pub struct IpBlacklist {
    // Set of blocked IPs
    blocked_ips: Mutex<HashMap<IpAddr, Instant>>,
    // Block duration
    block_duration: Duration,
}

impl IpBlacklist {
    pub fn new(block_minutes: u64) -> Self {
        Self {
            blocked_ips: Mutex::new(HashMap::new()),
            block_duration: Duration::from_secs(block_minutes * 60),
        }
    }

    // Check if an IP is blacklisted
    pub fn is_blacklisted(&self, ip: &IpAddr) -> bool {
        let mut blocked_ips = self.blocked_ips.lock().unwrap();

        if let Some(block_time) = blocked_ips.get(ip) {
            // Check if the block has expired
            if Instant::now().duration_since(*block_time) < self.block_duration {
                return true;
            } else {
                // Remove from blacklist if expired
                blocked_ips.remove(ip);
                return false;
            }
        }

        false
    }

    // Blacklist an IP
    pub fn blacklist_ip(&self, ip: IpAddr) {
        let mut blocked_ips = self.blocked_ips.lock().unwrap();
        blocked_ips.insert(ip, Instant::now());
        tracing::warn!("IP {} has been blacklisted for suspicious activity", ip);
    }

    // Clean up expired entries
    pub fn cleanup(&self) {
        let mut blocked_ips = self.blocked_ips.lock().unwrap();
        let now = Instant::now();
        blocked_ips.retain(|_, time| now.duration_since(*time) < self.block_duration);
    }
}

// Suspicious activity detector
pub struct ActivityDetector {
    // Track consecutive failures by IP
    failures: Mutex<HashMap<IpAddr, u32>>,
    // Track unique paths accessed by IP recently
    path_diversity: Mutex<HashMap<IpAddr, HashSet<String>>>,
    // Track request timestamps by IP
    request_timing: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    // Threshold for consecutive failures
    max_failures: u32,
    // Threshold for path diversity in short time
    max_path_diversity: usize,
    // Time window for request analysis
    timing_window: Duration,
    // Minimum time between requests (for detecting automated tools)
    min_request_interval: Duration,
}

impl ActivityDetector {
    pub fn new() -> Self {
        Self {
            failures: Mutex::new(HashMap::new()),
            path_diversity: Mutex::new(HashMap::new()),
            request_timing: Mutex::new(HashMap::new()),
            max_failures: 10,                       // 10 consecutive failures
            max_path_diversity: 20,                 // 20 unique paths in a short time
            timing_window: Duration::from_secs(60), // 1 minute window
            min_request_interval: Duration::from_millis(50), // 50ms minimum between requests
        }
    }

    // Track a failed request
    pub fn track_failure(&self, ip: &IpAddr) -> bool {
        let mut failures = self.failures.lock().unwrap();
        let count = failures.entry(*ip).and_modify(|c| *c += 1).or_insert(1);

        if *count >= self.max_failures {
            tracing::warn!("IP {} has reached failure threshold ({})", ip, *count);
            failures.remove(ip);
            return true;
        }

        false
    }

    // Reset failure count on successful request
    pub fn reset_failures(&self, ip: &IpAddr) {
        let mut failures = self.failures.lock().unwrap();
        failures.remove(ip);
    }

    // Track path diversity (many unique paths in short time = possible scanning)
    pub fn track_path(&self, ip: &IpAddr, path: &str) -> bool {
        let mut path_diversity = self.path_diversity.lock().unwrap();

        let paths = path_diversity.entry(*ip).or_insert_with(HashSet::new);
        paths.insert(path.to_string());

        if paths.len() > self.max_path_diversity {
            tracing::warn!(
                "IP {} accessed too many unique paths ({}) - possible scanning",
                ip,
                paths.len()
            );
            path_diversity.remove(ip);
            return true;
        }

        false
    }

    // Track request timing (too fast = probably automated)
    pub fn track_timing(&self, ip: &IpAddr) -> bool {
        let now = Instant::now();
        let mut request_timing = self.request_timing.lock().unwrap();

        let timings = request_timing.entry(*ip).or_insert_with(Vec::new);

        // Prune old timestamps
        timings.retain(|time| now.duration_since(*time) < self.timing_window);

        // Check timing between requests
        if let Some(last_time) = timings.last() {
            let interval = now.duration_since(*last_time);
            if interval < self.min_request_interval && timings.len() > 5 {
                tracing::warn!(
                    "IP {} is making requests too quickly ({:?} interval)",
                    ip,
                    interval
                );
                return true;
            }
        }

        // Add new timestamp
        timings.push(now);

        false
    }

    // Periodic cleanup to prevent memory leaks
    pub fn cleanup(&self) {
        let now = Instant::now();

        // Clean up path diversity tracking
        let mut path_diversity = self.path_diversity.lock().unwrap();
        path_diversity.clear();

        // Clean up request timing tracking
        let mut request_timing = self.request_timing.lock().unwrap();
        for timings in request_timing.values_mut() {
            timings.retain(|time| now.duration_since(*time) < self.timing_window);
        }
        request_timing.retain(|_, timings| !timings.is_empty());
    }
}
