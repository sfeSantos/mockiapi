use warp::{http::StatusCode, test::request, Filter};
use std::sync::{Arc};
use tokio::sync::Mutex;
use std::time::{Duration};
use tokio::time::Instant;
use mockiapi::endpoint_handler::serve_dynamic_response;
use mockiapi::models::{Endpoint, RateLimit};
use mockiapi::rate_limit::{new_rate_limit};
use mockiapi::utils::handle_rejection;

#[tokio::test]
async fn test_non_existent_endpoint() {
    let endpoints = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let rate_limiter = new_rate_limit();
    
    let filter = warp::path::full()
        .and(warp::any().map(|| None))
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response);

    let res = request()
        .method("GET")
        .path("/nonexistent")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_valid_request_without_auth() {
    let mut endpoints_map = std::collections::HashMap::new();
    endpoints_map.insert(
        "/public".to_string(),
        Endpoint {
            method: vec!["GET".to_string()],
            file: "uploads/file.json".to_string(),
            status_code: Some(200),
            rate_limit: None,
            authentication: None,
            delay: None,
        },
    );
    let endpoints = Arc::new(Mutex::new(endpoints_map));
    let rate_limiter = new_rate_limit();

    let filter = warp::path::full()
        .and(warp::any().map(|| None))
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response);

    let res = request()
        .method("GET")
        .path("/public")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_valid_request_with_basic_auth() {
    let mut endpoints_map = std::collections::HashMap::new();
    endpoints_map.insert(
        "/protected".to_string(),
        Endpoint {
            method: vec!["GET".to_string()],
            file: "uploads/file.json".to_string(),
            status_code: Some(200),
            rate_limit: None,
            authentication: Some(String::from("{\"username\": \"user\", \"password\": \"pass\"}")),
            delay: None,
        },
    );
    let endpoints = Arc::new(Mutex::new(endpoints_map));
    let rate_limiter = new_rate_limit();

    let filter = warp::path::full()
        .and(warp::any().map(|| Some("Basic dXNlcjpwYXNz".to_string()))) // Base64 for user:pass
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response);

    let res = request()
        .method("GET")
        .path("/protected")
        .header("Authorization", "Basic dXNlcjpwYXNz") // Base64 for user:pass
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_authentication() {
    let mut endpoints_map = std::collections::HashMap::new();
    endpoints_map.insert(
        "/protected".to_string(),
        Endpoint {
            method: vec!["GET".to_string()],
            file: "test.json".to_string(),
            status_code: Some(200),
            rate_limit: None,
            authentication: Some(String::from("{\"username\": \"user1\", \"password\": \"password1\"}")),
            delay: None,
        },
    );
    let endpoints = Arc::new(Mutex::new(endpoints_map));
    let rate_limiter = new_rate_limit();

    let filter = warp::path::full()
        .and(warp::any().map(|| None))
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response)
        .recover(handle_rejection);

    let res = request()
        .method("GET")
        .path("/protected")
        .header("Authorization", "Basic dXNlcjpwd2Q=") 
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_valid_request_with_bearer_auth() {
    let mut endpoints_map = std::collections::HashMap::new();
    endpoints_map.insert(
        "/secure".to_string(),
        Endpoint {
            method: vec!["GET".to_string()],
            file: "uploads/file.json".to_string(),
            status_code: Some(200),
            rate_limit: None,
            authentication: Some(String::from("{\"tokenData\": \"SOME_LONG_TOKEN\"}")),
            delay: None,
        },
    );
    let endpoints = Arc::new(Mutex::new(endpoints_map));
    let rate_limiter = new_rate_limit();

    let filter = warp::path::full()
        .and(warp::any().map(|| Some("Bearer SOME_LONG_TOKEN".to_string())))
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response);

    let res = request()
        .method("GET")
        .path("/secure")
        .header("Authorization", "Bearer SOME_LONG_TOKEN")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_rate_limit_exceeded() {
    let mut endpoints_map = std::collections::HashMap::new();
    endpoints_map.insert(
        "/rate-limited".to_string(),
        Endpoint {
            method: vec!["GET".to_string()],
            file: "test.json".to_string(),
            status_code: Some(200),
            rate_limit: Some(RateLimit {
                requests: 1,
                window_ms: 1,
            }), // Allow only one request
            authentication: None,
            delay: None,
        },
    );
    let endpoints = Arc::new(Mutex::new(endpoints_map));
    let rate_limiter = new_rate_limit();

    let filter = warp::path::full()
        .and(warp::any().map(|| None))
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response)
        .recover(handle_rejection);
    // First request should pass
    let _ = request()
        .method("GET")
        .path("/rate-limited")
        .reply(&filter)
        .await;

    // Second request should be rate-limited
    let res = request()
        .method("GET")
        .path("/rate-limited")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
}


#[tokio::test]
async fn test_request_with_delay() {
    let mut endpoints_map = std::collections::HashMap::new();
    endpoints_map.insert(
        "/delayed".to_string(),
        Endpoint {
            method: vec!["GET".to_string()],
            file: "uploads/file.json".to_string(),
            status_code: Some(200),
            rate_limit: None,
            authentication: None,
            delay: Some(2000), // 2 seconds delay
        },
    );
    let endpoints = Arc::new(Mutex::new(endpoints_map));
    let rate_limiter = new_rate_limit();

    let filter = warp::path::full()
        .and(warp::any().map(|| None))
        .and(warp::any().map(move || endpoints.clone()))
        .and(warp::any().map(move || rate_limiter.clone()))
        .and_then(serve_dynamic_response);

    let start_time = Instant::now();
    let res = request()
        .method("GET")
        .path("/delayed")
        .reply(&filter)
        .await;
    let elapsed = start_time.elapsed();

    assert_eq!(res.status(), StatusCode::OK);
    assert!(elapsed >= Duration::from_millis(2000)); // Ensure delay was applied
}