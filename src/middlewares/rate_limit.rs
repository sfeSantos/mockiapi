use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use log::warn;
use warp::http::Method;
use crate::models::{RateLimit, RateLimited};

/// The requests one HTTP method made in the current window.
#[derive(Debug)]
struct Window {
    method: Method,
    started: Instant,
    count: usize,
}

/// Fixed-window request counter of a single route, with one window per HTTP method.
///
/// Each route owns its limiter, so requests to different routes never wait on each other
/// and no key is built to find the counter. A route sees a handful of methods at most,
/// which makes a short list cheaper than a map.
#[derive(Debug)]
pub struct RateLimiter {
    limit: RateLimit,
    windows: Mutex<Vec<Window>>,
}

impl RateLimiter {
    pub fn new(limit: RateLimit) -> Self {
        Self { limit, windows: Mutex::new(Vec::new()) }
    }

    /// Counts a request, failing when the method used up its share of the window.
    pub fn check(&self, method: &Method) -> Result<(), RateLimited> {
        self.check_at(method, Instant::now())
    }

    fn check_at(&self, method: &Method, now: Instant) -> Result<(), RateLimited> {
        // A poisoned lock only means another request panicked. The counters are still valid.
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(window) = windows.iter_mut().find(|window| window.method == method) else {
            windows.push(Window { method: method.clone(), started: now, count: 1 });

            return self.allow(1, method);
        };

        if now.duration_since(window.started) > Duration::from_millis(self.limit.window_ms) {
            window.started = now;
            window.count = 0;
        }

        window.count = window.count.saturating_add(1);

        self.allow(window.count, method)
    }

    fn allow(&self, count: usize, method: &Method) -> Result<(), RateLimited> {
        if count > self.limit.requests {
            warn!("Rate limit exceeded | Method: {} | Current count: {} | Limit: {}", method, count, self.limit.requests);

            return Err(RateLimited);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter(requests: usize, window_ms: u64) -> RateLimiter {
        RateLimiter::new(RateLimit { requests, window_ms })
    }

    #[test]
    fn allows_up_to_the_limit_within_a_window() {
        let limiter = limiter(2, 1_000);
        let now = Instant::now();

        assert!(limiter.check_at(&Method::GET, now).is_ok());
        assert!(limiter.check_at(&Method::GET, now).is_ok());
        assert!(limiter.check_at(&Method::GET, now).is_err());
    }

    #[test]
    fn a_new_window_resets_the_count() {
        let limiter = limiter(1, 1_000);
        let now = Instant::now();

        assert!(limiter.check_at(&Method::GET, now).is_ok());
        assert!(limiter.check_at(&Method::GET, now + Duration::from_millis(500)).is_err());
        assert!(limiter.check_at(&Method::GET, now + Duration::from_millis(1_001)).is_ok());
    }

    #[test]
    fn each_method_has_its_own_window() {
        let limiter = limiter(1, 1_000);
        let now = Instant::now();

        assert!(limiter.check_at(&Method::GET, now).is_ok());
        assert!(limiter.check_at(&Method::POST, now).is_ok());
        assert!(limiter.check_at(&Method::POST, now).is_err());
    }

    #[test]
    fn a_limit_of_zero_refuses_everything() {
        let limiter = limiter(0, 1_000);

        assert!(limiter.check(&Method::GET).is_err());
    }
}
