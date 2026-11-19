use std::sync::Arc;
use bytes::Bytes;
use warp::http::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE, ETAG, IF_NONE_MATCH, VARY,
};
use warp::http::{Response, StatusCode};
use warp::test::request;
use warp::Filter;
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::Endpoints;
use mockiapi::routes::dashboard::Dashboard;
use mockiapi::routes::{dashboard_routes, mock_routes};

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Vite puts a content hash in the names under `assets/`, so the name is looked up.
fn bundled_asset(extension: &str) -> String {
    Dashboard::iter()
        .find(|name| name.starts_with("assets/") && name.ends_with(extension))
        .map(|name| name.into_owned())
        .expect("the dashboard bundle has no such asset")
}

fn bundled_bytes(name: &str) -> Vec<u8> {
    Dashboard::get(name)
        .expect("the file is missing from the dashboard bundle: run `npm run build` in frontend/")
        .data
        .into_owned()
}

async fn get(path: &str, accept_encoding: Option<&str>) -> Response<Bytes> {
    let request = request().path(path);
    let request = match accept_encoding {
        Some(accepted) => request.header(ACCEPT_ENCODING, accepted),
        None => request,
    };

    request.reply(&dashboard_routes()).await
}

#[tokio::test]
async fn test_root_serves_the_dashboard_index() {
    let response = get("/", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/html");
    assert_eq!(response.body().as_ref(), bundled_bytes("index.html"));
}

#[tokio::test]
async fn test_bundled_asset_is_served_with_its_content_type() {
    let script = bundled_asset(".js");

    let response = get(&format!("/{script}"), None).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/javascript");
    assert_eq!(response.body().as_ref(), bundled_bytes(&script));
}

#[tokio::test]
async fn test_client_without_accept_encoding_gets_the_file_as_it_is() {
    let response = get("/", None).await;

    assert!(!response.headers().contains_key(CONTENT_ENCODING));
    assert_eq!(response.headers()[VARY], "accept-encoding");
}

#[tokio::test]
async fn test_client_accepting_gzip_gets_the_precompressed_index() {
    let response = get("/", Some("gzip, deflate")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_ENCODING], "gzip");
    assert_eq!(response.headers()[CONTENT_TYPE], "text/html");
    assert_eq!(response.headers()[VARY], "accept-encoding");
    assert!(response.body().starts_with(&GZIP_MAGIC));
    assert_eq!(response.body().as_ref(), bundled_bytes("index.html.gz"));
}

#[tokio::test]
async fn test_client_accepting_gzip_gets_the_precompressed_asset() {
    let script = bundled_asset(".js");

    let response = get(&format!("/{script}"), Some("deflate, GZIP;q=0.8")).await;

    assert_eq!(response.headers()[CONTENT_ENCODING], "gzip");
    assert_eq!(response.headers()[CONTENT_TYPE], "text/javascript");
    assert_eq!(response.body().as_ref(), bundled_bytes(&format!("{script}.gz")));
}

#[tokio::test]
async fn test_client_refusing_gzip_gets_the_file_as_it_is() {
    for accepted in ["gzip;q=0", "br"] {
        let response = get("/", Some(accepted)).await;

        assert!(!response.headers().contains_key(CONTENT_ENCODING), "{accepted}");
        assert_eq!(response.body().as_ref(), bundled_bytes("index.html"), "{accepted}");
    }
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
async fn test_known_etag_of_the_gzip_copy_gets_not_modified() {
    let filter = dashboard_routes();
    let first = get("/", Some("gzip")).await;
    let etag = first.headers()[ETAG].clone();

    let second = request()
        .path("/")
        .header(ACCEPT_ENCODING, "gzip")
        .header(IF_NONE_MATCH, &etag)
        .reply(&filter)
        .await;

    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(second.headers()[CONTENT_ENCODING], "gzip");
    assert!(second.body().is_empty());
}

#[tokio::test]
async fn test_gzip_copy_has_its_own_etag() {
    let plain = get("/", None).await;
    let gzipped = get("/", Some("gzip")).await;

    assert_ne!(plain.headers()[ETAG], gzipped.headers()[ETAG]);

    // The tag of one variant must not pass for the other.
    let mismatched = request()
        .path("/")
        .header(IF_NONE_MATCH, &plain.headers()[ETAG])
        .header(ACCEPT_ENCODING, "gzip")
        .reply(&dashboard_routes())
        .await;

    assert_eq!(mismatched.status(), StatusCode::OK);
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
