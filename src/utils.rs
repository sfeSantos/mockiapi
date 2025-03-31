use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use log::info;
use warp::{Filter, Rejection, Reply};
use warp::http::Response;
use warp::hyper::Body;
use crate::models::{Endpoint, NotFound, RateLimited, Unauthorized};
use crate::middlewares::rate_limit::RateLimitTracker;

/// Adds a delay to the request handling if the `Endpoint` specifies a delay.
///
/// This function checks if the `Endpoint` has a `delay` field set. If a delay is specified,
/// it logs the delay and then asynchronously sleeps for the given duration, pausing the
/// request processing before returning control to the caller.
///
/// # Arguments
///
/// * `endpoint` - A reference to an `Endpoint` object that contains configuration data,
/// including an optional delay in milliseconds.
///
/// # Notes
/// This function uses `tokio::time::sleep` to asynchronously wait, so it does not block
/// the thread while waiting.
pub async fn add_possible_delay(endpoint: &Endpoint) {
    if let Some(delay) = endpoint.delay {
        info!("⏳ Applying delay of {} ms", delay);
        sleep(Duration::from_millis(delay)).await;
    }
}

/// Creates a `warp` filter that provides access to a shared `RateLimitTracker`.
///
/// This function returns a `warp::Filter` that, when used in a route, provides the `RateLimitTracker`
/// for rate-limiting purposes. It clones the provided `RateLimitTracker` for each request, allowing
/// the rate limiter to be shared across multiple routes and requests.
///
/// # Arguments
///
/// * `rate_limiter` - A `RateLimitTracker` object used to track and enforce rate limits.
///
/// # Returns
///
/// Returns a `warp::Filter` that extracts the `RateLimitTracker` from the filter context, allowing it
/// to be used in route handlers.
///
/// # Notes
/// This filter ensures that the `RateLimitTracker` is cloned and passed to the handler for every request.
pub fn with_rate_limiter(rate_limiter: RateLimitTracker) -> 
           impl Filter<Extract = (RateLimitTracker,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || rate_limiter.clone())
}

/// Reconstructs the full URL by appending query parameters to the base path.
///
/// # Arguments
/// * `path` - The base path (e.g., "/api/user/123/item/456").
/// * `query_params` - An optional `HashMap` containing query parameters.
///
/// # Returns
/// * A `String` representing the full URL with query parameters if any exist.
pub fn reconstruct_full_url(path: &str, query_params: &Option<HashMap<String, String>>) -> String {
    if let Some(params) = query_params {
        if !params.is_empty() {
            let query_string = serde_urlencoded::to_string(params).unwrap_or_default();
            return format!("{}?{}", path, query_string);
        }
    }
    path.to_string()
}

/// Handles rejections in the Warp web framework by returning appropriate HTTP responses.
///
/// This function inspects the provided `Rejection` and determines the appropriate HTTP status
/// and response body. It handles the following rejection types:
///
/// - `Unauthorized`: Returns a `401 Unauthorized` response.
/// - `RateLimited`: Returns a `429 Too Many Requests` response.
/// - `NotFound`: Returns a `404 Not Found` response.
/// - Any other rejection is propagated unchanged.
///
/// # Arguments
///
/// * `err` - A `Rejection` object representing the error encountered in the request pipeline.
///
/// # Returns
///
/// A `Result` containing an HTTP response (`impl Reply`) if the rejection is handled,
/// or the original `Rejection` if it is not recognized.
///
/// This ensures that if an error occurs during request processing, `handle_rejection`
/// will return an appropriate response instead of simply failing.
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.find::<Unauthorized>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(401)
            .body(Body::from("Unauthorized\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<RateLimited>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(429)
            .body(Body::from("Rate limit exceeded\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<NotFound>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(404)
            .body(Body::from("Resource not found\n"))
            .unwrap();
        return Ok(response);
    }

    Err(err)
}