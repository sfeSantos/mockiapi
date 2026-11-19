use std::path::Path;
use std::sync::Arc;
use warp::http::StatusCode;
use warp::test::request;
use warp::{Filter, Rejection, Reply};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::Endpoints;
use mockiapi::routes::admin_routes;

const BOUNDARY: &str = "mockiapi-test-boundary";

fn new_endpoints() -> Endpoints {
    Endpoints::default()
}

fn admin(endpoints: Endpoints) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    admin_routes(endpoints, Arc::new(GrpcRegistry::new()))
}

async fn delete(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    path: &str,
) -> StatusCode {
    request()
        .method("DELETE")
        .path(&format!("/delete/{}", urlencoding::encode(path)))
        .reply(filter)
        .await
        .status()
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
    assert!(endpoints.is_empty());
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
    assert!(endpoints.is_empty());
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
    assert!(endpoints.is_empty());
}

#[tokio::test]
async fn test_delete_removes_the_endpoint_and_its_uploaded_file() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let registered = register(
        &filter,
        &[("path", "/api/temporary"), ("methods", "GET"), ("file", "{\"ok\":true}")],
    )
    .await;
    let uploaded_file = endpoints.get("/api/temporary").unwrap().endpoint.file.clone();
    let existed_before_delete = Path::new(&uploaded_file).exists();
    let deleted = delete(&filter, "/api/temporary").await;

    assert_eq!(registered, StatusCode::OK);
    assert!(existed_before_delete);
    assert_eq!(deleted, StatusCode::OK);
    assert!(endpoints.is_empty());
    assert!(!Path::new(&uploaded_file).exists());
}

#[tokio::test]
async fn test_delete_removes_the_grpc_mocks_of_the_endpoint() {
    let registry = Arc::new(GrpcRegistry::new());
    let filter = admin_routes(new_endpoints(), Arc::clone(&registry));

    register(
        &filter,
        &[
            ("path", "/grpc"),
            ("methods", "POST"),
            ("grpcService", "com.example.BookService"),
            ("grpcRPC", "GetBook"),
            ("file", "{\"id\":\"b123\"}"),
        ],
    )
    .await;
    let mock_before_delete = registry.get_mock("com.example.BookService", "GetBook");
    let deleted = delete(&filter, "/grpc").await;

    assert!(mock_before_delete.is_some());
    assert_eq!(deleted, StatusCode::OK);
    assert!(registry.get_mock("com.example.BookService", "GetBook").is_none());
}

#[tokio::test]
async fn test_registering_a_path_again_removes_the_previous_upload() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());
    let fields = [("path", "/api/replaced"), ("methods", "GET"), ("file", "{}")];

    register(&filter, &fields).await;
    let first_file = endpoints.get("/api/replaced").unwrap().endpoint.file.clone();
    register(&filter, &fields).await;
    let second_file = endpoints.get("/api/replaced").unwrap().endpoint.file.clone();
    delete(&filter, "/api/replaced").await;

    assert_ne!(first_file, second_file);
    assert!(!Path::new(&first_file).exists());
    assert!(!Path::new(&second_file).exists());
}

#[tokio::test]
async fn test_registering_a_path_again_without_a_file_keeps_the_upload() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    register(&filter, &[("path", "/api/kept"), ("methods", "GET"), ("file", "{\"ok\":true}")]).await;
    let first_file = endpoints.get("/api/kept").unwrap().endpoint.file.clone();
    let updated = register(
        &filter,
        &[("path", "/api/kept"), ("methods", "GET,POST"), ("status_code", "503")],
    )
    .await;
    let route = endpoints.get("/api/kept").unwrap();
    let kept_on_disk = std::fs::read_to_string(&first_file).ok();
    delete(&filter, "/api/kept").await;

    assert_eq!(updated, StatusCode::OK);
    assert_eq!(route.endpoint.file, first_file);
    assert_eq!(route.endpoint.method, ["GET", "POST"]);
    assert_eq!(route.endpoint.status_code, Some(503));
    assert_eq!(kept_on_disk.as_deref(), Some("{\"ok\":true}"));
    assert!(!Path::new(&first_file).exists());
}

#[tokio::test]
async fn test_registering_a_new_path_without_a_file_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(&filter, &[("path", "/api/missing-file"), ("methods", "GET")]).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.is_empty());
}

async fn content(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    path: &str,
) -> (StatusCode, String) {
    let response = request()
        .path(&format!("/content/{}", urlencoding::encode(path)))
        .reply(filter)
        .await;

    (response.status(), String::from_utf8_lossy(response.body()).into_owned())
}

#[tokio::test]
async fn test_content_answers_the_response_as_it_was_uploaded() {
    let filter = admin(new_endpoints());
    let template = "{\n  \"id\": {{id}}\n}";

    register(
        &filter,
        &[("path", "/usr/{id}"), ("methods", "GET"), ("with_dynamic_vars", "true"), ("file", template)],
    )
    .await;
    let (status, body) = content(&filter, "/usr/{id}").await;
    delete(&filter, "/usr/{id}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, template);
}

#[tokio::test]
async fn test_content_follows_a_new_upload_and_survives_an_update_without_one() {
    let filter = admin(new_endpoints());

    register(&filter, &[("path", "/api/content"), ("methods", "GET"), ("file", "{\"version\":1}")]).await;
    register(&filter, &[("path", "/api/content"), ("methods", "GET"), ("file", "{\"version\":2}")]).await;
    let (_, replaced) = content(&filter, "/api/content").await;
    register(&filter, &[("path", "/api/content"), ("methods", "GET"), ("status_code", "503")]).await;
    let (_, kept) = content(&filter, "/api/content").await;
    delete(&filter, "/api/content").await;

    assert_eq!(replaced, "{\"version\":2}");
    assert_eq!(kept, "{\"version\":2}");
}

#[tokio::test]
async fn test_content_of_an_unknown_path_is_not_found() {
    let filter = admin(new_endpoints());

    let (status, _) = content(&filter, "/api/nobody").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
