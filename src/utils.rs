use std::time::Duration;
use tokio::time::sleep;
use log::info;
use warp::{Filter, Rejection, Reply};
use warp::http::Response;
use warp::hyper::Body;
use crate::models::{Endpoint, NotFound, RateLimited, Unauthorized};
use crate::middlewares::rate_limit::RateLimitTracker;

pub async fn add_possible_delay(endpoint: &Endpoint) {
    if let Some(delay) = endpoint.delay {
        info!("⏳ Applying delay of {} ms", delay);
        sleep(Duration::from_millis(delay)).await;
    }
}

pub fn with_rate_limiter(rate_limiter: RateLimitTracker) -> 
           impl Filter<Extract = (RateLimitTracker,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || rate_limiter.clone())
}

/// Custom rejection handler for returning proper error responses
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