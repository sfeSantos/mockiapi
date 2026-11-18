use std::time::Duration;
use tokio::time::sleep;
use log::info;
use warp::{Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, Response, StatusCode};
use warp::hyper::Body;
use crate::models::{Endpoint, InvalidGrpcRequest, InvalidGraphQLRequest, MethodNotAllowed, NotFound, RateLimited, Unauthorized};

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
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::from("Unauthorized\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<RateLimited>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::from("Rate limit exceeded\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<NotFound>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Resource not found\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<MethodNotAllowed>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from("Method not allowed\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<InvalidGraphQLRequest>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid GraphQL request\n"))
            .unwrap();
        return Ok(response);
    } else if err.find::<InvalidGrpcRequest>().is_some() {
        let response: Response<Body> = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid gRPC request\n"))
            .unwrap();
        return Ok(response);
    }

    Err(err)
}

/// Builds a JSON response. The body is taken as is, so shared bytes are not copied.
pub fn json_response(status: StatusCode, body: impl Into<Body>) -> Response<Body> {
    let mut response = Response::new(body.into());
    *response.status_mut() = status;
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    response
}
