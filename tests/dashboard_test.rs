use std::sync::Arc;
use warp::http::header::{CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use warp::http::StatusCode;
use warp::test::request;
use warp::Filter;
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::Endpoints;
use mockiapi::routes::dashboard::Dashboard;
use mockiapi::routes::{dashboard_routes, mock_routes};

/// Vite puts a content hash in the names under `assets/`, so the name is looked up.
fn bundled_asset(extension: &str) -> String {
    Dashboard::iter()
        .find(|name| name.starts_with("assets/") && name.ends_with(extension))
        .map(|name| name.into_owned())
        .expect("the dashboard bundle has no such asset")
}

fn bundled_bytes(name: &str) -> Vec<u8> {
    Dashboard::get(name)
        .expect("the file is missing from the dashboard bundle")
        .data
        .into_owned()
}

#[tokio::test]
async fn test_root_serves_the_dashboard_index() {
    let response = request()
        .path("/")
        .reply(&dashboard_routes())
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/html");
    assert_eq!(response.body().as_ref(), bundled_bytes("index.html"));
}

#[tokio::test]
async fn test_bundled_asset_is_served_with_its_content_type() {
    let script = bundled_asset(".js");

    let response = request()
        .path(&format!("/{script}"))
        .reply(&dashboard_routes())
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/javascript");
    assert_eq!(response.body().as_ref(), bundled_bytes(&script));
}

#[tokio::test]
async fn test_known_etag_gets_not_modified_without_a_body() {
    let filter = dashboard_routes();
    let first = request()
        .path("/")
        .reply(&filter)
        .await;
    let etag = first.headers()[ETAG].clone();

    let second = request()
        .path("/")
        .header(IF_NONE_MATCH, &etag)
        .reply(&filter)
        .await;

    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(second.headers()[ETAG], etag);
    assert!(second.body().is_empty());
}

#[tokio::test]
async fn test_stale_etag_gets_the_file_again() {
    let response = request()
        .path("/")
        .header(IF_NONE_MATCH, "\"not-the-current-hash\"")
        .reply(&dashboard_routes())
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.body().is_empty());
}

#[tokio::test]
async fn test_path_outside_the_bundle_is_rejected() {
    let matched = request()
        .path("/api/users")
        .matches(&dashboard_routes())
        .await;

    assert!(!matched);
}

#[tokio::test]
async fn test_only_reads_are_served() {
    let matched = request()
        .method("POST")
        .path("/")
        .matches(&dashboard_routes())
        .await;

    assert!(!matched);
}

#[tokio::test]
async fn test_path_outside_the_bundle_reaches_the_mocks() {
    let routes = dashboard_routes()
        .or(mock_routes(Endpoints::default(), Arc::new(GrpcRegistry::new())));

    let response = request()
        .path("/api/users")
        .reply(&routes)
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.body().as_ref(), b"Resource not found\n");
}
