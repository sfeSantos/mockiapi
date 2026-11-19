use std::time::Duration;
use log::info;
use tokio::time::sleep;
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, Response, StatusCode};
use warp::hyper::Body;
use crate::models::Endpoint;

/// Waits for the delay the endpoint was registered with, if any, without blocking the thread.
pub async fn add_possible_delay(endpoint: &Endpoint) {
    if let Some(delay) = endpoint.delay {
        info!("⏳ Applying delay of {} ms", delay);
        sleep(Duration::from_millis(delay)).await;
    }
}

/// Builds a plain text response.
pub fn text_response(status: StatusCode, body: impl Into<Body>) -> Response<Body> {
    let mut response = Response::new(body.into());
    *response.status_mut() = status;
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));

    response
}

/// Builds a JSON response. The body is taken as is, so shared bytes are not copied.
pub fn json_response(status: StatusCode, body: impl Into<Body>) -> Response<Body> {
    let mut response = Response::new(body.into());
    *response.status_mut() = status;
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    response
}
