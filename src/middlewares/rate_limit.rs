use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use log::{info, warn};
use tokio::sync::Mutex;
use crate::models::{RateLimit, RateLimited};

pub type RateLimitTracker = Arc<Mutex<HashMap<String, (Instant, usize)>>>;

/// Initialize the rate limit tracker
pub fn new_rate_limit() -> RateLimitTracker {
    Arc::new(Mutex::new(HashMap::new()))
}

pub async fn check_rate_limit(
    path: String,
    method: &str,
    rate_limit: Option<&RateLimit>,
    rate_limiter: RateLimitTracker,
) -> Result<(), warp::Rejection> {
    if let Some(limit) = rate_limit {
        let mut rate_tracker = rate_limiter.lock().await;

        // Use both path and method as the key for rate limiting
        let key = format!("{}|{}", path, method); // Combine path and method

        let now = Instant::now();
        let (start_time, count) = rate_tracker.entry(key.clone()).or_insert((now, 0));

        if now.duration_since(*start_time).as_millis() as u64 > limit.window_ms {
            *start_time = now;
            *count = 1;
            info!("🕛 Rate window expired for path: {}. Resetting counter.", key);
        } else {
            *count += 1;

            if *count > limit.requests {
                warn!(
                    "⚠️ Rate limit exceeded for path: {} | Method: {} | Current count: {} | Limit: {}",
                    path,
                    method,
                    *count,
                    limit.requests
                );
                return Err(warp::reject::custom(RateLimited));
            }
        }
    }

    Ok(())
}