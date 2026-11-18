use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::http::StatusCode;
use warp::test::request;
use warp::{Filter, Rejection, Reply};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::Endpoints;
use mockiapi::routes::admin_routes;

const BOUNDARY: &str = "mockiapi-test-boundary";

fn new_endpoints() -> Endpoints {
    Arc::new(Mutex::new(HashMap::new()))
}

fn admin(endpoints: Endpoints) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    admin_routes(endpoints, Arc::new(GrpcRegistry::new()))
}

/// Encodes the fields the dashboard form sends, with `file` going out as the uploaded JSON.
fn multipart_body(fields: &[(&str, &str)]) -> String {
    let mut body = String::new();

    for (name, value) in fields {
        let disposition = match *name {
            "file" => "name=\"file\"; filename=\"mock.json\"\r\nContent-Type: application/json".to_string(),
            _ => format!("name=\"{name}\""),
        };
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; {disposition}\r\n\r\n{value}\r\n"
        ));
    }

    body.push_str(&format!("--{BOUNDARY}--\r\n"));

    body
}

async fn register(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    fields: &[(&str, &str)],
) -> StatusCode {
    request()
        .method("POST")
        .path("/register")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(multipart_body(fields))
        .reply(filter)
        .await
        .status()
}

#[tokio::test]
async fn test_rate_limit_without_window_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("path", "/api/limited"), ("methods", "GET"), ("rate_limit", "10"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.lock().await.is_empty());
}

#[tokio::test]
async fn test_rate_limit_that_is_not_a_number_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("path", "/api/limited"), ("methods", "GET"), ("rate_limit", "ten/1000"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.lock().await.is_empty());
}

#[tokio::test]
async fn test_grpc_service_without_rpc_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("path", "/grpc"), ("methods", "POST"), ("grpcService", "com.example.BookService"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.lock().await.is_empty());
}
